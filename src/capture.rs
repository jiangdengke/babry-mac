use crate::display::OutputInfo;
use std::{
    path::Path,
    process::{Command, Stdio},
};

// 第一版用系统自带的 screencapture，按屏幕的全局矩形截取，Retina 屏输出原始像素。
// 之后换成 ScreenCaptureKit，可以省掉子进程并拿到更多控制。
pub fn capture(path: &Path, output: &OutputInfo) -> Result<(), String> {
    let result = capture_command(path, output)
        .output()
        .map_err(|e| format!("无法启动 screencapture: {e}"))?;
    if result.status.success() && path.metadata().is_ok_and(|meta| meta.len() > 0) {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    Err(format!(
        "截屏失败: {}\n请在“系统设置 → 隐私与安全性 → 录屏与系统录音”中允许启动 babry 的终端或应用，然后重试。",
        stderr.trim()
    ))
}

fn capture_command(path: &Path, output: &OutputInfo) -> Command {
    let rect = format!(
        "{},{},{},{}",
        output.x, output.y, output.width, output.height
    );
    let mut command = Command::new("/usr/sbin/screencapture");
    command.args(["-x", "-t", "png", "-R", &rect]).arg(path);
    command
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
    fn capture_limits_screencapture_to_one_screen_rect() {
        let output = OutputInfo {
            name: "DELL".into(),
            x: -400,
            y: -1080,
            width: 1920,
            height: 1080,
            scale: 1.0,
            focused: true,
        };
        let command = capture_command(Path::new("capture.png"), &output);
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "-x",
                "-t",
                "png",
                "-R",
                "-400,-1080,1920,1080",
                "capture.png"
            ]
        );
    }

    #[test]
    fn notification_text_cannot_break_out_of_the_applescript_string() {
        assert_eq!(applescript_escape("a\"b\\c\nd"), "a\\\"b\\\\c d");
    }
}
