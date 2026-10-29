use crate::display::OutputInfo;
use image::RgbaImage;
use std::{
    path::Path,
    process::{Command, Stdio},
};

/// 用 ScreenCaptureKit 截取一块屏幕。像素直接留在内存里，不经过子进程和临时文件；
/// Retina 屏按原始像素输出，颜色保持显示器色彩空间的原值。
#[cfg(target_os = "macos")]
pub fn capture(output: &OutputInfo) -> Result<RgbaImage, String> {
    screen_capture_kit::capture(output)
}

#[cfg(not(target_os = "macos"))]
pub fn capture(_output: &OutputInfo) -> Result<RgbaImage, String> {
    Err("babry-mac 只支持 macOS".into())
}

/// ScreenCaptureKit 的屏幕矩形与 OutputInfo 一样是左上角原点的全局点坐标，
/// 两块屏幕不会重叠，所以按左上角位置就能认出同一块屏幕。
fn is_same_screen(x: f64, y: f64, output: &OutputInfo) -> bool {
    x.round() as i32 == output.x && y.round() as i32 == output.y
}

#[cfg(target_os = "macos")]
mod screen_capture_kit {
    use super::is_same_screen;
    use crate::display::OutputInfo;
    use block2::RcBlock;
    use image::RgbaImage;
    use objc2::{AllocAnyThread, available, rc::Retained};
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGContext, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo,
    };
    use objc2_foundation::{NSArray, NSError};
    use objc2_screen_capture_kit::{
        SCCaptureResolutionType, SCContentFilter, SCDisplay, SCScreenshotManager,
        SCShareableContent, SCStreamConfiguration, SCStreamErrorCode,
    };
    use std::{sync::mpsc, time::Duration};

    // 回调通常在几十毫秒内返回；系统服务卡住时报错退出，而不是一直挂着。
    const TIMEOUT: Duration = Duration::from_secs(10);

    /// 回调在 ScreenCaptureKit 的后台队列执行，屏幕对象要交回等待中的主线程。
    struct SharedDisplay(Retained<SCDisplay>);

    // SAFETY: SCDisplay 是只读的屏幕快照，交出后回调线程不再访问它。
    unsafe impl Send for SharedDisplay {}

    pub fn capture(output: &OutputInfo) -> Result<RgbaImage, String> {
        if available!(macos = 15.2) {
            capture_rect(output)
        } else if available!(macos = 14.0) {
            capture_display(output)
        } else {
            Err("截屏需要 macOS 14 或更新的系统".into())
        }
    }

    /// macOS 15.2 起可以直接按全局矩形截图，省掉枚举屏幕和窗口，冷启动快一半以上。
    fn capture_rect(output: &OutputInfo) -> Result<RgbaImage, String> {
        let rect = CGRect::new(
            CGPoint::new(f64::from(output.x), f64::from(output.y)),
            CGSize::new(f64::from(output.width), f64::from(output.height)),
        );
        wait(|sender| {
            let handler = image_handler(sender);
            // SAFETY: 系统会复制回调 block，之后在后台队列调用一次。
            unsafe {
                SCScreenshotManager::captureImageInRect_completionHandler(rect, Some(&handler))
            };
        })
    }

    /// macOS 14 只能先查到这块屏幕，再按屏幕截图。
    fn capture_display(output: &OutputInfo) -> Result<RgbaImage, String> {
        let display = find_display(output)?;
        // SAFETY: 过滤器和配置在调用期间保持存活，系统会复制回调 block。
        unsafe {
            let filter = SCContentFilter::initWithDisplay_excludingWindows(
                SCContentFilter::alloc(),
                &display,
                &NSArray::new(),
            );
            let config = SCStreamConfiguration::new();
            let scale = f64::from(filter.pointPixelScale());
            config.setWidth((f64::from(output.width) * scale).round() as usize);
            config.setHeight((f64::from(output.height) * scale).round() as usize);
            config.setCaptureResolution(SCCaptureResolutionType::Best);
            config.setShowsCursor(false);
            wait(|sender| {
                let handler = image_handler(sender);
                SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
                    &filter,
                    &config,
                    Some(&handler),
                );
            })
        }
    }

    fn find_display(output: &OutputInfo) -> Result<Retained<SCDisplay>, String> {
        let output = output.clone();
        let shared = wait(move |sender| {
            let handler = RcBlock::new(
                move |content: *mut SCShareableContent, error: *mut NSError| {
                    // SAFETY: 系统传入有效对象或空指针；显示器列表在回调期间有效。
                    let result = match unsafe { content.as_ref() } {
                        Some(content) => unsafe { content.displays() }
                            .iter()
                            .find(|display| {
                                let frame = unsafe { display.frame() };
                                is_same_screen(frame.origin.x, frame.origin.y, &output)
                            })
                            .map(SharedDisplay)
                            .ok_or_else(|| format!("ScreenCaptureKit 找不到屏幕 {}", output.name)),
                        None => Err(describe(error)),
                    };
                    let _ = sender.send(result);
                },
            );
            // SAFETY: 系统会复制回调 block，之后在后台队列调用一次。
            unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&handler) };
        })?;
        Ok(shared.0)
    }

    /// CGImage 只在回调期间有效，所以在回调里转换成 RGBA 再送回。
    fn image_handler(
        sender: mpsc::Sender<Result<RgbaImage, String>>,
    ) -> RcBlock<dyn Fn(*mut CGImage, *mut NSError)> {
        RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
            // SAFETY: 系统传入有效的 CGImage 或空指针。
            let result = match unsafe { image.as_ref() } {
                Some(image) => to_rgba(image),
                None => Err(describe(error)),
            };
            let _ = sender.send(result);
        })
    }

    /// 发起带完成回调的系统调用，阻塞等待回调送回结果。
    fn wait<T: Send + 'static>(
        start: impl FnOnce(mpsc::Sender<Result<T, String>>),
    ) -> Result<T, String> {
        let (sender, receiver) = mpsc::channel();
        start(sender);
        receiver
            .recv_timeout(TIMEOUT)
            .map_err(|_| "截屏超时".to_string())?
    }

    fn describe(error: *mut NSError) -> String {
        // SAFETY: 失败时系统传入有效的 NSError，也可能为空。
        let Some(error) = (unsafe { error.as_ref() }) else {
            return "截屏失败".into();
        };
        let mut message = format!("截屏失败: {}", error.localizedDescription());
        if error.code() == SCStreamErrorCode::UserDeclined.0 {
            message.push_str(
                "\n请在“系统设置 → 隐私与安全性 → 录屏与系统录音”中允许 Babry 或启动它的终端，然后重试。",
            );
        }
        message
    }

    fn to_rgba(image: &CGImage) -> Result<RgbaImage, String> {
        let width = CGImage::width(Some(image));
        let height = CGImage::height(Some(image));
        let mut pixels = vec![0u8; width * height * 4];
        // 编辑窗口不做色彩管理，原样显示像素值。沿用截图自己的色彩空间、不做转换，
        // 冻结的画面才和真实屏幕一致；转换到 sRGB 会让彩色区域在打开编辑器时变色。
        let space = CGImage::color_space(Some(image));
        // 屏幕内容不透明，预乘不改变像素值；32 位大端序保证字节顺序是 RGBA。
        let info = CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0;
        // SAFETY: pixels 正好容纳 width × height 个 4 字节像素，并且比 context 活得久。
        let context = unsafe {
            CGBitmapContextCreate(
                pixels.as_mut_ptr().cast(),
                width,
                height,
                8,
                width * 4,
                space.as_deref(),
                info,
            )
        }
        .ok_or("无法创建截图位图")?;
        let rect = CGRect::new(CGPoint::ZERO, CGSize::new(width as f64, height as f64));
        CGContext::draw_image(Some(&context), rect, Some(image));
        drop(context);
        RgbaImage::from_raw(width as u32, height as u32, pixels)
            .ok_or_else(|| "截图尺寸无效".into())
    }
}

// 用 AppleScript 发通知，不需要 .app 包；打包后再换成 UNUserNotificationCenter。
pub fn notify(message: &str) {
    let script = format!(
        "display notification \"{}\" with title \"Babry\"",
        applescript_escape(message)
    );
    let _ = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn applescript_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

#[cfg(target_os = "macos")]
pub fn copy_png(path: &Path) -> Result<(), String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG};
    use objc2_foundation::NSData;

    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let data = NSData::with_bytes(&bytes);
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    // SAFETY: NSPasteboardTypePNG 是 AppKit 导出的常量字符串，进程内始终有效。
    let png_type = unsafe { NSPasteboardTypePNG };
    pasteboard
        .setData_forType(Some(&data), png_type)
        .then_some(())
        .ok_or_else(|| "写入剪贴板失败".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn copy_png(_path: &Path) -> Result<(), String> {
    Err("babry-mac 只支持 macOS".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screens_are_matched_by_their_top_left_corner() {
        let output = OutputInfo {
            name: "DELL".into(),
            x: -400,
            y: -1080,
            width: 1920,
            height: 1080,
            scale: 1.0,
            focused: true,
        };
        assert!(is_same_screen(-400.0, -1080.0, &output));
        assert!(is_same_screen(-400.2, -1079.8, &output));
        assert!(!is_same_screen(0.0, 0.0, &output));
    }

    #[test]
    fn notification_text_cannot_break_out_of_the_applescript_string() {
        assert_eq!(applescript_escape("a\"b\\c\nd"), "a\\\"b\\\\c d");
    }
}
