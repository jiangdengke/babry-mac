//! 屏幕贴图。上游用 Wayland layer-shell；macOS 版用浮在普通窗口之上的无边框窗口。
//! 界面在 `ui/pin.slint`，事件转发在 `src/pin_window.rs`，这里只放进程启动和几何计算。

use crate::{display::OutputInfo, overlay::Frame};
use std::{
    path::Path,
    process::{Command, Stdio},
};

pub const MIN_PIN_SIZE: f64 = 64.0;
const RESIZE_EDGE_DISTANCE: f64 = 14.0;
const MIN_ZOOM: f64 = 0.2;

/// 每张贴图是独立进程，关闭截图窗口后贴图仍然保留，也可以同时钉多张。
pub fn launch(path: &Path, output: Option<String>) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("贴图文件不存在: {}", path.display()));
    }
    let current = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut command = Command::new(&current);
    command.arg("--pin-surface").arg(path);
    if let Some(output) = output {
        command.args(["--output", &output]);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法启动贴图窗口 {}: {error}", current.display()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    fn moves_left(self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }
    fn moves_right(self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }
    fn moves_top(self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }
    fn moves_bottom(self) -> bool {
        matches!(self, Self::Bottom | Self::BottomLeft | Self::BottomRight)
    }

    /// 与 `ui/pin.slint` 约定的指针形状：1 左右、2 上下、3 左上到右下、4 右上到左下。
    pub fn cursor(self) -> i32 {
        match self {
            Self::Left | Self::Right => 1,
            Self::Top | Self::Bottom => 2,
            Self::TopLeft | Self::BottomRight => 3,
            Self::TopRight | Self::BottomLeft => 4,
        }
    }
}

/// 坐标相对贴图左上角。边框内 14 点算抓到边，角落同时满足两条边。
pub fn edge_at(x: f64, y: f64, width: f64, height: f64) -> Option<ResizeEdge> {
    let left = x <= RESIZE_EDGE_DISTANCE;
    let right = x >= width - RESIZE_EDGE_DISTANCE;
    let top = y <= RESIZE_EDGE_DISTANCE;
    let bottom = y >= height - RESIZE_EDGE_DISTANCE;
    match (left, right, top, bottom) {
        (true, false, true, false) => Some(ResizeEdge::TopLeft),
        (false, true, true, false) => Some(ResizeEdge::TopRight),
        (true, false, false, true) => Some(ResizeEdge::BottomLeft),
        (false, true, false, true) => Some(ResizeEdge::BottomRight),
        (true, false, false, false) => Some(ResizeEdge::Left),
        (false, true, false, false) => Some(ResizeEdge::Right),
        (false, false, true, false) => Some(ResizeEdge::Top),
        (false, false, false, true) => Some(ResizeEdge::Bottom),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct Gesture {
    edge: Option<ResizeEdge>,
    start: Frame,
    pointer: (f64, f64), // 按下时指针的屏幕坐标。
}

/// 贴图窗口的位置和尺寸，始终限制在目标屏幕内。
pub struct PinGeometry {
    screen: Frame,
    base_width: f64,
    base_height: f64,
    zoom: f64,
    frame: Frame,
    gesture: Option<Gesture>,
}

impl PinGeometry {
    /// 初始尺寸等于截图在该屏幕上的逻辑尺寸，超出屏幕时等比缩小，并居中显示。
    pub fn new(image_width: u32, image_height: u32, output: &OutputInfo) -> Self {
        let screen = Frame::from(output);
        let scale = f64::from(output.scale).max(0.1);
        let natural_width = (f64::from(image_width) / scale).max(1.0);
        let natural_height = (f64::from(image_height) / scale).max(1.0);
        let fit = (screen.width / natural_width)
            .min(screen.height / natural_height)
            .min(1.0);
        let width = (natural_width * fit).round().max(1.0);
        let height = (natural_height * fit).round().max(1.0);
        let frame = Frame {
            x: screen.x + ((screen.width - width) / 2.0).floor(),
            y: screen.y + ((screen.height - height) / 2.0).floor(),
            width,
            height,
        };
        Self {
            screen,
            base_width: width,
            base_height: height,
            zoom: 1.0,
            frame,
            gesture: None,
        }
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn edge_at(&self, x: f64, y: f64) -> Option<ResizeEdge> {
        edge_at(x, y, self.frame.width, self.frame.height)
    }

    /// 左键按下，坐标相对贴图左上角：边框附近开始调整大小，其余位置开始移动。
    pub fn press(&mut self, x: f64, y: f64) {
        self.gesture = Some(Gesture {
            edge: self.edge_at(x, y),
            start: self.frame,
            pointer: (self.frame.x + x, self.frame.y + y),
        });
    }

    /// 指针移动到屏幕坐标 (x, y)。按相对按下位置的总位移计算，不会累积误差。
    /// 返回 None 表示没有进行中的拖动，或位置和尺寸都没有变化。
    pub fn drag_to(&mut self, x: f64, y: f64) -> Option<Frame> {
        let gesture = self.gesture?;
        let dx = x - gesture.pointer.0;
        let dy = y - gesture.pointer.1;
        let start = gesture.start;
        let frame = match gesture.edge {
            None => self.clamp_position(Frame {
                x: start.x + dx,
                y: start.y + dy,
                ..start
            }),
            Some(edge) => self.resized(start, edge, dx, dy),
        };
        self.update(frame)
    }

    pub fn release(&mut self) {
        self.gesture = None;
    }

    pub fn is_dragging(&self) -> bool {
        self.gesture.is_some()
    }

    /// 滚轮缩放并保持中心不动；delta 为正表示向上滚动，放大。
    pub fn zoom(&mut self, delta: f64) -> Option<Frame> {
        if delta == 0.0 || self.gesture.is_some() {
            return None;
        }
        // 鼠标滚轮一格按 10% 缩放；触控板的小位移按比例缩放，避免一下滑到头。
        let factor = 1.1_f64.powf((delta / 40.0).clamp(-1.0, 1.0));
        let max_zoom = (self.screen.width / self.base_width)
            .min(self.screen.height / self.base_height)
            .max(MIN_ZOOM);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, max_zoom);
        let width = (self.base_width * self.zoom).round().max(1.0);
        let height = (self.base_height * self.zoom).round().max(1.0);
        let center_x = self.frame.x + self.frame.width / 2.0;
        let center_y = self.frame.y + self.frame.height / 2.0;
        let frame = self.clamp_position(Frame {
            x: center_x - width / 2.0,
            y: center_y - height / 2.0,
            width,
            height,
        });
        self.update(frame)
    }

    fn update(&mut self, frame: Frame) -> Option<Frame> {
        (frame != self.frame).then(|| {
            self.frame = frame;
            frame
        })
    }

    fn clamp_position(&self, frame: Frame) -> Frame {
        let max_x = self.screen.x + (self.screen.width - frame.width).max(0.0);
        let max_y = self.screen.y + (self.screen.height - frame.height).max(0.0);
        Frame {
            x: frame.x.round().clamp(self.screen.x, max_x),
            y: frame.y.round().clamp(self.screen.y, max_y),
            ..frame
        }
    }

    // 只移动抓住的边，对边保持不动；被拖动的边不越过屏幕，也不让贴图小于最小尺寸。
    fn resized(&self, start: Frame, edge: ResizeEdge, dx: f64, dy: f64) -> Frame {
        let mut left = start.x;
        let mut top = start.y;
        let mut right = start.x + start.width;
        let mut bottom = start.y + start.height;
        if edge.moves_left() {
            left = (left + dx).max(self.screen.x).min(right - MIN_PIN_SIZE);
        }
        if edge.moves_right() {
            right = (right + dx)
                .min(self.screen.x + self.screen.width)
                .max(left + MIN_PIN_SIZE);
        }
        if edge.moves_top() {
            top = (top + dy).max(self.screen.y).min(bottom - MIN_PIN_SIZE);
        }
        if edge.moves_bottom() {
            bottom = (bottom + dy)
                .min(self.screen.y + self.screen.height)
                .max(top + MIN_PIN_SIZE);
        }
        let (left, top) = (left.round(), top.round());
        Frame {
            x: left,
            y: top,
            width: right.round() - left,
            height: bottom.round() - top,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retina() -> OutputInfo {
        OutputInfo {
            name: "Built-in".into(),
            x: 0,
            y: 0,
            width: 1710,
            height: 1107,
            scale: 2.0,
            focused: true,
        }
    }

    fn frame(x: f64, y: f64, width: f64, height: f64) -> Frame {
        Frame {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn starts_at_the_logical_size_centered_on_the_screen() {
        // Retina 截图 800x600 像素对应 400x300 点。
        let pin = PinGeometry::new(800, 600, &retina());
        assert_eq!(pin.frame(), frame(655.0, 403.0, 400.0, 300.0));
    }

    #[test]
    fn oversized_images_shrink_to_fit_a_secondary_screen() {
        let output = OutputInfo {
            name: "DELL".into(),
            x: -1920,
            y: -1080,
            width: 1920,
            height: 1080,
            scale: 1.0,
            focused: false,
        };
        let pin = PinGeometry::new(4000, 3000, &output);
        assert_eq!(pin.frame(), frame(-1680.0, -1080.0, 1440.0, 1080.0));
    }

    #[test]
    fn edge_detection_distinguishes_corners_edges_and_the_interior() {
        assert_eq!(edge_at(2.0, 2.0, 400.0, 300.0), Some(ResizeEdge::TopLeft));
        assert_eq!(edge_at(399.0, 150.0, 400.0, 300.0), Some(ResizeEdge::Right));
        assert_eq!(
            edge_at(200.0, 299.0, 400.0, 300.0),
            Some(ResizeEdge::Bottom)
        );
        assert_eq!(edge_at(200.0, 150.0, 400.0, 300.0), None);
        assert_eq!(
            ResizeEdge::TopLeft.cursor(),
            ResizeEdge::BottomRight.cursor()
        );
    }

    #[test]
    fn dragging_follows_the_pointer_and_stays_on_screen() {
        let mut pin = PinGeometry::new(800, 600, &retina());
        // 按在贴图内 (100, 100)，即屏幕坐标 (755, 503)。
        pin.press(100.0, 100.0);
        assert!(pin.is_dragging());
        assert_eq!(
            pin.drag_to(805.0, 533.0),
            Some(frame(705.0, 433.0, 400.0, 300.0))
        );
        assert_eq!(
            pin.drag_to(815.0, 533.0),
            Some(frame(715.0, 433.0, 400.0, 300.0))
        );
        assert_eq!(pin.drag_to(815.0, 533.0), None);
        pin.drag_to(-5000.0, -5000.0);
        assert_eq!((pin.frame().x, pin.frame().y), (0.0, 0.0));
        pin.release();
        assert!(!pin.is_dragging());
        assert_eq!(pin.drag_to(900.0, 900.0), None);
    }

    #[test]
    fn edges_resize_only_their_side_and_keep_the_minimum_size() {
        let mut pin = PinGeometry::new(800, 600, &retina());
        // 左边框，屏幕坐标 (657, 553)，向左拖 50 点。
        pin.press(2.0, 150.0);
        assert_eq!(
            pin.drag_to(607.0, 553.0),
            Some(frame(605.0, 403.0, 450.0, 300.0))
        );
        pin.release();
        // 右下角一直拖过左上角，停在 64 点的最小尺寸。
        pin.press(449.0, 299.0);
        pin.drag_to(-1000.0, -1000.0);
        assert_eq!(pin.frame(), frame(605.0, 403.0, 64.0, 64.0));
    }

    #[test]
    fn zoom_keeps_the_center_and_stays_within_limits() {
        let mut pin = PinGeometry::new(800, 600, &retina());
        assert_eq!(pin.zoom(120.0), Some(frame(635.0, 388.0, 440.0, 330.0)));
        for _ in 0..40 {
            pin.zoom(-120.0);
        }
        assert_eq!((pin.frame().width, pin.frame().height), (80.0, 60.0));
        for _ in 0..80 {
            pin.zoom(120.0);
        }
        // 放大到刚好放进屏幕为止。
        assert_eq!(pin.frame().height, 1107.0);
        assert!(pin.frame().width <= 1710.0);
        // 触控板的小位移只缩放一点点。
        let before = pin.frame().width;
        pin.zoom(-4.0);
        assert!(before - pin.frame().width < before * 0.02);
    }
}
