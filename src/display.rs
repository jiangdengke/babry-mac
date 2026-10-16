//! 显示器信息。替代上游通过 niri / Hyprland IPC 查询输出的 `compositor.rs`。

/// 位置与尺寸均为全局逻辑坐标（点），原点在主屏左上角，向下为正，可包含负坐标。
#[derive(Clone, Debug, PartialEq)]
pub struct OutputInfo {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub focused: bool,
}

/// AppKit 屏幕坐标中的矩形：原点在主屏左下角，向上为正。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CocoaFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl CocoaFrame {
    // 鼠标停在屏幕最上沿时 y 等于上边界，所以上界和右界也算在内。
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.width && y <= self.y + self.height
    }
}

/// 列出所有显示器；鼠标所在的屏幕标记为焦点屏。
#[cfg(target_os = "macos")]
pub fn outputs() -> Result<Vec<OutputInfo>, String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen};

    let mtm = MainThreadMarker::new().ok_or("显示器查询必须在主线程执行")?;
    let screens: Vec<(String, CocoaFrame, f64)> = NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let frame = screen.frame();
            (
                screen.localizedName().to_string(),
                CocoaFrame {
                    x: frame.origin.x,
                    y: frame.origin.y,
                    width: frame.size.width,
                    height: frame.size.height,
                },
                screen.backingScaleFactor(),
            )
        })
        .collect();
    let mouse = NSEvent::mouseLocation();
    let result = from_cocoa(&screens, (mouse.x, mouse.y));
    if result.is_empty() {
        Err("没有可用的显示器".into())
    } else {
        Ok(result)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn outputs() -> Result<Vec<OutputInfo>, String> {
    Err("babry-mac 只支持 macOS".into())
}

/// 把 AppKit 坐标转换为左上角原点。第一块屏幕是主屏，它的原点就是全局原点。
fn from_cocoa(screens: &[(String, CocoaFrame, f64)], mouse: (f64, f64)) -> Vec<OutputInfo> {
    let Some((_, primary, _)) = screens.first() else {
        return Vec::new();
    };
    let primary_top = primary.y + primary.height;
    let mut names: Vec<String> = Vec::new();
    screens
        .iter()
        .map(|(name, frame, scale)| {
            // 同型号显示器名称相同，追加序号保证 --output 能区分。
            let mut unique = name.clone();
            let mut index = 2;
            while names.contains(&unique) {
                unique = format!("{name} {index}");
                index += 1;
            }
            names.push(unique.clone());
            OutputInfo {
                name: unique,
                x: frame.x.round() as i32,
                y: (primary_top - (frame.y + frame.height)).round() as i32,
                width: frame.width.round().max(1.0) as u32,
                height: frame.height.round().max(1.0) as u32,
                scale: *scale as f32,
                focused: frame.contains(mouse.0, mouse.1),
            }
        })
        .collect()
}

pub fn select_output(outputs: &[OutputInfo], name: Option<&str>) -> Result<OutputInfo, String> {
    if let Some(name) = name {
        outputs
            .iter()
            .find(|output| output.name == name)
            .cloned()
            .ok_or_else(|| format!("显示器 {name} 不存在"))
    } else {
        outputs
            .iter()
            .find(|output| output.focused)
            .or_else(|| outputs.first())
            .cloned()
            .ok_or_else(|| "没有可用的显示器".into())
    }
}

impl OutputInfo {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && y >= f64::from(self.y)
            && x < f64::from(self.x) + f64::from(self.width)
            && y < f64::from(self.y) + f64::from(self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: f64, y: f64, width: f64, height: f64) -> CocoaFrame {
        CocoaFrame {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn cocoa_frames_flip_to_top_left_origin() {
        // 主屏 1440x900；外接屏在主屏上方偏左，AppKit 中 y 为正。
        let screens = vec![
            ("Built-in".to_string(), frame(0.0, 0.0, 1440.0, 900.0), 2.0),
            (
                "DELL".to_string(),
                frame(-400.0, 900.0, 1920.0, 1080.0),
                1.0,
            ),
        ];
        let outputs = from_cocoa(&screens, (100.0, 1000.0));
        assert_eq!((outputs[0].x, outputs[0].y), (0, 0));
        assert_eq!((outputs[1].x, outputs[1].y), (-400, -1080));
        assert_eq!((outputs[1].width, outputs[1].height), (1920, 1080));
        assert_eq!(outputs[0].scale, 2.0);
        assert!(!outputs[0].focused);
        assert!(outputs[1].focused);
        assert!(outputs[1].contains(-100.0, -500.0));
    }

    #[test]
    fn duplicate_names_get_a_suffix() {
        let screens = vec![
            ("LG".to_string(), frame(0.0, 0.0, 1920.0, 1080.0), 1.0),
            ("LG".to_string(), frame(1920.0, 0.0, 1920.0, 1080.0), 1.0),
        ];
        let outputs = from_cocoa(&screens, (0.0, 0.0));
        assert_eq!(outputs[0].name, "LG");
        assert_eq!(outputs[1].name, "LG 2");
        assert_eq!(select_output(&outputs, Some("LG 2")).unwrap().x, 1920);
    }

    #[test]
    fn falls_back_to_first_screen_and_rejects_unknown_names() {
        let screens = vec![("A".to_string(), frame(0.0, 0.0, 800.0, 600.0), 1.0)];
        let outputs = from_cocoa(&screens, (-50.0, -50.0));
        assert_eq!(select_output(&outputs, None).unwrap().name, "A");
        assert!(select_output(&outputs, Some("B")).is_err());
        assert!(from_cocoa(&[], (0.0, 0.0)).is_empty());
    }
}
