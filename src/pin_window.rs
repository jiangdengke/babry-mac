//! 贴图窗口的事件处理。界面只负责显示和转发事件，几何计算在 `babry::pin`。

use super::{PinWindow, image_to_slint};
use babry::{
    display,
    overlay::{self, Frame, Layer},
    pin::PinGeometry,
};
use slint::{
    ComponentHandle, LogicalPosition, LogicalSize, Timer, TimerMode,
    winit_030::{WinitWindowAccessor, winit},
};
use std::{cell::RefCell, path::Path, rc::Rc, sync::Arc, time::Duration};

type NativeWindow = Rc<RefCell<Option<Arc<winit::window::Window>>>>;

// 拖动时读取指针位置的间隔，约 120 次每秒。
const DRAG_POLL_INTERVAL: Duration = Duration::from_millis(8);

pub fn run(path: &Path, output_name: Option<String>) -> Result<(), String> {
    let source = image::open(path)
        .map_err(|error| format!("无法读取贴图 {}: {error}", path.display()))?
        .to_rgba8();
    if source.width() == 0 || source.height() == 0 {
        return Err("贴图尺寸为空".to_string());
    }
    let outputs = display::outputs()?;
    let output = display::select_output(&outputs, output_name.as_deref())?;
    let geometry = Rc::new(RefCell::new(PinGeometry::new(
        source.width(),
        source.height(),
        &output,
    )));
    let frame = geometry.borrow().frame();

    overlay::select_backend(frame, Layer::Pin)?;
    let app = PinWindow::new().map_err(|error| error.to_string())?;
    app.set_picture(image_to_slint(&source));
    let native: NativeWindow = Rc::default();
    apply_frame(&app, &native, frame);
    {
        // winit 窗口在事件循环启动后才创建，之后调整位置都直接写 NSWindow。
        let weak = app.as_weak();
        let native = native.clone();
        slint::spawn_local(async move {
            let Some(app) = weak.upgrade() else { return };
            if let Ok(window) = app.window().winit_window().await {
                overlay::raise(&window, frame, Layer::Pin);
                *native.borrow_mut() = Some(window);
            }
        })
        .map_err(|error| error.to_string())?;
    }

    let weak = app.as_weak();
    let hover_geometry = geometry.clone();
    app.on_hover(move |x, y| {
        let Some(app) = weak.upgrade() else { return };
        let geometry = hover_geometry.borrow();
        if !geometry.is_dragging() {
            let edge = geometry.edge_at(f64::from(x), f64::from(y));
            app.set_cursor_shape(edge.map_or(0, |edge| edge.cursor()));
        }
    });

    // 按下后轮询系统的指针位置和左键状态，松开时停止，不依赖可能丢失的松开事件。
    let drag_timer = Rc::new(Timer::default());
    let weak = app.as_weak();
    let press_geometry = geometry.clone();
    let press_native = native.clone();
    let press_timer = drag_timer.clone();
    app.on_press(move |x, y| {
        press_geometry
            .borrow_mut()
            .press(f64::from(x), f64::from(y));
        let weak = weak.clone();
        let geometry = press_geometry.clone();
        let native = press_native.clone();
        let timer = Rc::downgrade(&press_timer);
        press_timer.start(TimerMode::Repeated, DRAG_POLL_INTERVAL, move || {
            let Some(app) = weak.upgrade() else { return };
            if !display::left_button_pressed() {
                geometry.borrow_mut().release();
                if let Some(timer) = timer.upgrade() {
                    timer.stop();
                }
                return;
            }
            let Some((x, y)) = display::pointer_location() else {
                return;
            };
            let frame = geometry.borrow_mut().drag_to(x, y);
            if let Some(frame) = frame {
                apply_frame(&app, &native, frame);
            }
        });
    });

    let weak = app.as_weak();
    let zoom_native = native.clone();
    app.on_zoom(move |delta| {
        let frame = geometry.borrow_mut().zoom(f64::from(delta));
        if let (Some(app), Some(frame)) = (weak.upgrade(), frame) {
            apply_frame(&app, &zoom_native, frame);
        }
    });

    let weak = app.as_weak();
    app.on_close_pin(move || {
        drag_timer.stop();
        // 先释放持有的原生窗口引用，否则 Slint 无法隐藏窗口并结束事件循环。
        native.borrow_mut().take();
        if let Some(app) = weak.upgrade() {
            app.hide().ok();
        }
    });

    let weak = app.as_weak();
    let _right_click_monitor = close_on_right_click(move || {
        if let Some(app) = weak.upgrade() {
            app.invoke_close_pin();
        }
    });

    app.run().map_err(|error| error.to_string())
}

/// 贴图不在前台时，AppKit 收到右键却不交给窗口内容，Slint 收不到。
/// 用本进程的事件监视器处理右键，前台时也走这条路径。返回的句柄要在事件循环期间保持存活。
#[cfg(target_os = "macos")]
fn close_on_right_click(
    close: impl Fn() + 'static,
) -> Option<objc2::rc::Retained<objc2::runtime::AnyObject>> {
    use objc2_app_kit::{NSEvent, NSEventMask};
    use std::ptr::{NonNull, null_mut};

    let handler = block2::RcBlock::new(move |_event: NonNull<NSEvent>| -> *mut NSEvent {
        close();
        // 事件已经处理，不再交给窗口，避免重复关闭。
        null_mut()
    });
    // SAFETY: 返回空指针表示吞掉事件，这是 AppKit 允许的返回值；AppKit 会复制这个 block。
    unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::RightMouseDown, &handler)
    }
}

#[cfg(not(target_os = "macos"))]
fn close_on_right_click(_close: impl Fn() + 'static) {}

// 先更新界面的尺寸约束，再一次性写入窗口位置和尺寸，避免 Slint 把窗口改回旧尺寸。
fn apply_frame(app: &PinWindow, native: &NativeWindow, frame: Frame) {
    app.set_frame_width(frame.width as f32);
    app.set_frame_height(frame.height as f32);
    if let Some(window) = native.borrow().as_ref() {
        overlay::set_frame(window, frame);
    } else {
        let window = app.window();
        window.set_size(LogicalSize::new(frame.width as f32, frame.height as f32));
        window.set_position(LogicalPosition::new(frame.x as f32, frame.y as f32));
    }
}
