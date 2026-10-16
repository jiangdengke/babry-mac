//! 吸附问题的复现记录。设置 `BABRY_DEBUG_DIR` 后，每次截图都保存原始全屏图，
//! 并记录悬停、滚轮切换和最终选区，之后可以用同一张图离线重放。

use crate::{core::RectPx, display::OutputInfo};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// 选区分析在后台线程完成后才交给界面，所以记录句柄需要能跨线程传递。
#[derive(Clone)]
pub struct DebugLog(Arc<Mutex<File>>);

impl DebugLog {
    /// 未设置环境变量时返回 None；写入失败只打印警告，不影响截图。
    pub fn from_env(source: &Path, output: &OutputInfo) -> Option<Self> {
        let directory = PathBuf::from(std::env::var_os("BABRY_DEBUG_DIR")?);
        match Self::create(&directory, source, output) {
            Ok(log) => {
                eprintln!("调试记录: {}", directory.display());
                Some(log)
            }
            Err(error) => {
                eprintln!("无法写入调试记录 {}: {error}", directory.display());
                None
            }
        }
    }

    pub fn create(directory: &Path, source: &Path, output: &OutputInfo) -> std::io::Result<Self> {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        std::fs::create_dir_all(directory)?;
        std::fs::copy(source, directory.join(format!("babry-debug-{stamp}.png")))?;
        let mut file = File::create(directory.join(format!("babry-debug-{stamp}.log")))?;
        writeln!(
            file,
            "output {:?} at {},{} size {}x{} scale {}",
            output.name, output.x, output.y, output.width, output.height, output.scale
        )?;
        Ok(Self(Arc::new(Mutex::new(file))))
    }

    pub fn hover(&self, x: f32, y: f32, rect: RectPx) {
        self.line(format_args!("hover {x:.4} {y:.4} -> {}", format_rect(rect)));
    }

    pub fn wheel(&self, x: f32, y: f32, direction: i32, rect: RectPx) {
        self.line(format_args!(
            "wheel {direction:+} {x:.4} {y:.4} -> {}",
            format_rect(rect)
        ));
    }

    pub fn line(&self, text: std::fmt::Arguments) {
        if let Ok(mut file) = self.0.lock() {
            let _ = writeln!(file, "{text}");
        }
    }
}

fn format_rect(rect: RectPx) -> String {
    format!("{},{} {}x{}", rect.x, rect.y, rect.width, rect.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_the_capture_and_appends_events() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.png");
        image::RgbaImage::new(4, 4).save(&source).unwrap();
        let output = OutputInfo {
            name: "Built-in".into(),
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            scale: 2.0,
            focused: true,
        };
        let target = directory.path().join("debug");
        let log = DebugLog::create(&target, &source, &output).unwrap();
        log.hover(0.25, 0.5, RectPx::new(1, 2, 3, 4));
        log.wheel(0.25, 0.5, -1, RectPx::new(0, 0, 4, 4));
        drop(log);

        let mut names: Vec<_> = std::fs::read_dir(&target)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names[1].ends_with(".png"));
        let text = std::fs::read_to_string(target.join(&names[0])).unwrap();
        assert!(text.contains("hover 0.2500 0.5000 -> 1,2 3x4"), "{text}");
        assert!(text.contains("wheel -1 0.2500 0.5000 -> 0,0 4x4"), "{text}");
    }
}
