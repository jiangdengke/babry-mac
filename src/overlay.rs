//! 截图编辑窗口的 macOS 覆盖层：无边框、铺满一块屏幕、盖住菜单栏和 Dock。
//! 上游在 niri 下用 `open-fullscreen` 窗口规则得到同样的效果。
//!
//! 不用原生全屏：它会新建一个桌面空间并播放切换动画。

use crate::display::OutputInfo;
use slint::winit_030::winit;

/// 选择 winit 后端，并预设窗口属性。必须在创建任何 Slint 组件之前调用。
#[cfg(target_os = "macos")]
pub fn select_backend(output: &OutputInfo) -> Result<(), String> {
    use winit::platform::macos::{
        ActivationPolicy, EventLoopBuilderExtMacOS, WindowAttributesExtMacOS,
    };

    let mut event_loop =
        winit::event_loop::EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
    // 不显示 Dock 图标，也不接管菜单栏；窗口仍然可以获得键盘焦点。
    event_loop.with_activation_policy(ActivationPolicy::Accessory);

    let position = winit::dpi::LogicalPosition::new(output.x, output.y);
    let size = winit::dpi::LogicalSize::new(output.width, output.height);
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_event_loop_builder(event_loop)
        .with_winit_window_attributes_hook(move |attributes| {
            attributes
                .with_decorations(false)
                .with_resizable(false)
                .with_position(position)
                .with_inner_size(size)
                .with_has_shadow(false)
                .with_accepts_first_mouse(true)
        })
        .select()
        .map_err(|error| format!("无法初始化窗口后端: {error}"))
}

/// winit 窗口创建后调用：提高窗口层级，并贴合目标屏幕的边框。
#[cfg(target_os = "macos")]
pub fn raise(window: &winit::window::Window, output: &OutputInfo) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSScreen, NSStatusWindowLevel, NSView, NSWindowAnimationBehavior,
        NSWindowCollectionBehavior,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: winit 保证 ns_view 在窗口存在期间指向有效的 NSView；这里在主线程同步使用。
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    let Some(ns_window) = view.window() else {
        return;
    };

    // 菜单栏是 24、状态栏是 25。再高一级就能盖住它们，
    // 同时低于弹出菜单（101），中文输入法的候选窗不会被挡住。
    ns_window.setLevel(NSStatusWindowLevel + 1);
    // 出现在当前桌面空间，包括其他应用的全屏空间；不参与 ⌘` 窗口切换。
    ns_window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    ns_window.setAnimationBehavior(NSWindowAnimationBehavior::None);
    ns_window.setHasShadow(false);

    // winit 的位置换算依赖主屏高度；这里按同样规则直接写入 AppKit 坐标，避免误差。
    if let Some(primary) = NSScreen::screens(mtm).firstObject() {
        let top = primary.frame().origin.y + primary.frame().size.height;
        let (x, y) = (f64::from(output.x), f64::from(output.y));
        let (width, height) = (f64::from(output.width), f64::from(output.height));
        let frame = NSRect::new(
            NSPoint::new(x, top - y - height),
            NSSize::new(width, height),
        );
        ns_window.setFrame_display(frame, true);
    }
    ns_window.makeKeyAndOrderFront(None);
}

#[cfg(not(target_os = "macos"))]
pub fn select_backend(_output: &OutputInfo) -> Result<(), String> {
    Err("babry-mac 只支持 macOS".into())
}

#[cfg(not(target_os = "macos"))]
pub fn raise(_window: &winit::window::Window, _output: &OutputInfo) {}
