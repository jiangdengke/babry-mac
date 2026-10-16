//! macOS 窗口设置：截图编辑窗口和贴图窗口都是无边框窗口，位置和层级由这里控制。
//!
//! 编辑窗口铺满一块屏幕并盖住菜单栏和 Dock，上游在 niri 下用 `open-fullscreen`
//! 窗口规则得到同样的效果。不用原生全屏：它会新建一个桌面空间并播放切换动画。
//! 贴图窗口浮在普通窗口之上，对应上游 layer-shell 的 Top 层。

use crate::display::OutputInfo;
use slint::winit_030::winit;

/// 屏幕上的矩形，单位是点，原点在主屏左上角，向下为正。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<&OutputInfo> for Frame {
    fn from(output: &OutputInfo) -> Self {
        Self {
            x: f64::from(output.x),
            y: f64::from(output.y),
            width: f64::from(output.width),
            height: f64::from(output.height),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// 截图编辑窗口：盖住菜单栏和 Dock。
    Editor,
    /// 贴图：浮在普通窗口之上，低于菜单栏。
    Pin,
}

/// 选择 winit 后端，并预设窗口属性。必须在创建任何 Slint 组件之前调用。
#[cfg(target_os = "macos")]
pub fn select_backend(frame: Frame, layer: Layer) -> Result<(), String> {
    use winit::platform::macos::{
        ActivationPolicy, EventLoopBuilderExtMacOS, WindowAttributesExtMacOS,
    };

    let mut event_loop =
        winit::event_loop::EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
    // 不显示 Dock 图标，也不接管菜单栏；窗口仍然可以获得键盘焦点。
    event_loop.with_activation_policy(ActivationPolicy::Accessory);

    let position = winit::dpi::LogicalPosition::new(frame.x, frame.y);
    let size = winit::dpi::LogicalSize::new(frame.width, frame.height);
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_event_loop_builder(event_loop)
        .with_winit_window_attributes_hook(move |attributes| {
            attributes
                .with_decorations(false)
                .with_resizable(false)
                .with_position(position)
                .with_inner_size(size)
                .with_has_shadow(layer == Layer::Pin)
                .with_accepts_first_mouse(true)
        })
        .select()
        .map_err(|error| format!("无法初始化窗口后端: {error}"))
}

/// winit 窗口创建后调用：设置层级和桌面空间行为，并贴合目标位置。
#[cfg(target_os = "macos")]
pub fn raise(window: &winit::window::Window, frame: Frame, layer: Layer) {
    use objc2_app_kit::{
        NSFloatingWindowLevel, NSStatusWindowLevel, NSWindowAnimationBehavior,
        NSWindowCollectionBehavior,
    };

    let Some(ns_window) = ns_window(window) else {
        return;
    };
    let level = match layer {
        // 菜单栏是 24、状态栏是 25。再高一级就能盖住它们，
        // 同时低于弹出菜单（101），中文输入法的候选窗不会被挡住。
        Layer::Editor => NSStatusWindowLevel + 1,
        Layer::Pin => NSFloatingWindowLevel,
    };
    ns_window.setLevel(level);
    // 出现在当前桌面空间，包括其他应用的全屏空间；不参与 ⌘` 窗口切换。
    ns_window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    ns_window.setAnimationBehavior(NSWindowAnimationBehavior::None);
    ns_window.setHasShadow(layer == Layer::Pin);
    set_frame(window, frame);
    ns_window.makeKeyAndOrderFront(None);
}

/// 一次写入位置和尺寸。分开设置时 AppKit 以左下角为原点改尺寸，窗口会先跳一下。
#[cfg(target_os = "macos")]
pub fn set_frame(window: &winit::window::Window, frame: Frame) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let (Some(mtm), Some(ns_window)) = (MainThreadMarker::new(), ns_window(window)) else {
        return;
    };
    // winit 的位置换算依赖主屏高度；这里按同样规则直接写入 AppKit 坐标，避免误差。
    if let Some(primary) = NSScreen::screens(mtm).firstObject() {
        let top = primary.frame().origin.y + primary.frame().size.height;
        let rect = NSRect::new(
            NSPoint::new(frame.x, top - frame.y - frame.height),
            NSSize::new(frame.width, frame.height),
        );
        ns_window.setFrame_display(rect, true);
    }
}

#[cfg(target_os = "macos")]
fn ns_window(
    window: &winit::window::Window,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    use objc2_app_kit::NSView;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: winit 保证 ns_view 在窗口存在期间指向有效的 NSView；这里在主线程同步使用。
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    view.window()
}

#[cfg(not(target_os = "macos"))]
pub fn select_backend(_frame: Frame, _layer: Layer) -> Result<(), String> {
    Err("babry-mac 只支持 macOS".into())
}

#[cfg(not(target_os = "macos"))]
pub fn raise(_window: &winit::window::Window, _frame: Frame, _layer: Layer) {}

#[cfg(not(target_os = "macos"))]
pub fn set_frame(_window: &winit::window::Window, _frame: Frame) {}
