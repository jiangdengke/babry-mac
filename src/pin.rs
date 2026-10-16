//! 屏幕贴图。上游用 Wayland layer-shell 实现；macOS 版计划改用置顶的无边框 NSPanel，尚未完成。

use std::path::Path;

/// 编辑器据此决定是否在导出前就提示“暂不支持”，避免为一次失败的贴图多存一张图。
pub const SUPPORTED: bool = false;

const UNSUPPORTED: &str = "macOS 版暂未实现贴图";

pub fn launch(path: &Path, _output: Option<String>) -> Result<(), String> {
    run_surface(path, None)
}

pub fn run_surface(path: &Path, _output: Option<String>) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("贴图文件不存在: {}", path.display()));
    }
    Err(UNSUPPORTED.into())
}
