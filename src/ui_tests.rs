use super::*;
use imageproc::{drawing::draw_filled_rect_mut, rect::Rect};
use slint::{
    LogicalPosition, PhysicalSize, Rgb8Pixel,
    platform::{
        Platform, PointerEventButton, WindowAdapter, WindowEvent,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};

struct TestPlatform(Rc<MinimalSoftwareWindow>);
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn wheel_selected_parent_is_committed_by_click_and_drag_still_works() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(TestPlatform(window.clone()))).unwrap();
    let app = AppWindow::new().unwrap();
    let mut image = RgbaImage::from_pixel(640, 480, Rgba([20, 20, 20, 255]));
    draw_filled_rect_mut(
        &mut image,
        Rect::at(40, 30).of_size(560, 300),
        Rgba([45, 45, 45, 255]),
    );
    draw_filled_rect_mut(
        &mut image,
        Rect::at(120, 90).of_size(140, 50),
        Rgba([75, 75, 75, 255]),
    );
    let detector = RegionDetector::new(&image);
    app.set_selection_loading(true);
    app.set_screenshot(image_to_slint(&image));
    app.show().unwrap();
    window.set_size(PhysicalSize::new(640, 480));
    let mut pixels = vec![Rgb8Pixel::default(); 640 * 480];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 640);
    });
    let position = LogicalPosition::new(180.0, 110.0);
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    assert_eq!(app.get_sw(), 0.0);
    app.set_selection_loading(false);
    install_selection_detector(&app, detector, None);
    // 后台结果到达时不需要再次移动鼠标就能显示候选。
    assert!((app.get_sw() - 140.0 / 640.0).abs() < 0.005);
    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: 120.0,
    });
    assert!((app.get_sw() - 560.0 / 640.0).abs() < 0.005);
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    assert_eq!(app.get_phase(), 2);
    assert!(
        (app.get_sw() - 560.0 / 640.0).abs() < 0.005,
        "click reset the wheel choice"
    );

    app.set_phase(0);
    let start = LogicalPosition::new(60.0, 60.0);
    let end = LogicalPosition::new(290.0, 210.0);
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: start,
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: end });
    install_selection_detector(&app, RegionDetector::new(&image), None);
    // 结果刷新不会覆盖鼠标按住期间已经拖出的手动选区。
    assert!((app.get_sw() - 230.0 / 640.0).abs() < 0.001);
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: end,
        button: PointerEventButton::Left,
    });
    assert_eq!(app.get_phase(), 2);
    assert!((app.get_sx() - 60.0 / 640.0).abs() < 0.001);
    assert!((app.get_sy() - 60.0 / 480.0).abs() < 0.001);
    assert!((app.get_sw() - 230.0 / 640.0).abs() < 0.001);
    assert!((app.get_sh() - 150.0 / 480.0).abs() < 0.001);

    // 编辑阶段拖动右边框，只改变宽度。
    drag(&app, (290.0, 135.0), (350.0, 135.0));
    assert_selection(&app, (60.0, 60.0, 290.0, 150.0));
    // 拖动左上角，同时移动左边和上边。
    drag(&app, (60.0, 60.0), (40.0, 30.0));
    assert_selection(&app, (40.0, 30.0, 310.0, 180.0));
    // 在选区内部拖动是在画标注，不改变选区。
    drag(&app, (200.0, 135.0), (220.0, 150.0));
    assert_selection(&app, (40.0, 30.0, 310.0, 180.0));
    // 左边拖过右边时停在最小宽度 8px，不会翻转。
    drag(&app, (40.0, 120.0), (500.0, 120.0));
    assert_selection(&app, (342.0, 30.0, 8.0, 180.0));
    app.hide().unwrap();
}

fn drag(app: &AppWindow, from: (f32, f32), to: (f32, f32)) {
    let from = LogicalPosition::new(from.0, from.1);
    let to = LogicalPosition::new(to.0, to.1);
    let window = app.window();
    window.dispatch_event(WindowEvent::PointerMoved { position: from });
    window.dispatch_event(WindowEvent::PointerPressed {
        position: from,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerMoved { position: to });
    window.dispatch_event(WindowEvent::PointerReleased {
        position: to,
        button: PointerEventButton::Left,
    });
}

// 期望值是 640x480 窗口中的逻辑像素 (x, y, 宽, 高)。
fn assert_selection(app: &AppWindow, expected: (f32, f32, f32, f32)) {
    let actual = (
        app.get_sx() * 640.0,
        app.get_sy() * 480.0,
        app.get_sw() * 640.0,
        app.get_sh() * 480.0,
    );
    let close = |a: f32, b: f32| (a - b).abs() < 0.5;
    assert!(
        close(actual.0, expected.0)
            && close(actual.1, expected.1)
            && close(actual.2, expected.2)
            && close(actual.3, expected.3),
        "selection {actual:?}, expected {expected:?}"
    );
}
