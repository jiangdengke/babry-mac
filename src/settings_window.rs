//! 快捷键设置窗口。常驻进程打开它之前会注销全局快捷键，窗口关闭后重新载入配置，
//! 否则系统会先抢走已绑定的组合键，这里就录不到了。

#[cfg(target_os = "macos")]
pub use macos::run;

#[cfg(not(target_os = "macos"))]
pub fn run() -> Result<(), String> {
    Err("babry-mac 只支持 macOS".into())
}

#[cfg(target_os = "macos")]
mod macos {
    use super::super::SettingsWindow;
    use babry::{
        display,
        hotkeys::{self, Bindings},
        overlay::{self, Frame, Layer},
    };
    use global_hotkey::hotkey::{HotKey, Modifiers};
    use slint::{
        ComponentHandle,
        winit_030::{
            EventResult, WinitWindowAccessor,
            winit::{
                event::{ElementState, WindowEvent},
                keyboard::{ModifiersState, PhysicalKey},
            },
        },
    };
    use std::{cell::RefCell, rc::Rc};

    // 与 ui/settings.slint 的窗口尺寸一致。
    const WIDTH: f64 = 340.0;
    const HEIGHT: f64 = 160.0;

    const IDLE_MESSAGE: &str = "点击右侧的框，按下新的组合键。";
    const RECORDING_MESSAGE: &str = "按下组合键，Esc 取消。";
    const SAVED_MESSAGE: &str = "已保存，关闭窗口后生效。";

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Action {
        Region,
        Screen,
    }

    impl Action {
        fn from_index(index: i32) -> Option<Self> {
            match index {
                1 => Some(Self::Region),
                2 => Some(Self::Screen),
                _ => None,
            }
        }

        fn index(self) -> i32 {
            match self {
                Self::Region => 1,
                Self::Screen => 2,
            }
        }
    }

    struct Recorder {
        bindings: Bindings,
        recording: Option<Action>,
        modifiers: Modifiers,
    }

    impl Recorder {
        fn slot(&mut self, action: Action) -> &mut Option<HotKey> {
            match action {
                Action::Region => &mut self.bindings.region,
                Action::Screen => &mut self.bindings.screen,
            }
        }

        fn other(&self, action: Action) -> Option<HotKey> {
            match action {
                Action::Region => self.bindings.screen,
                Action::Screen => self.bindings.region,
            }
        }
    }

    pub fn run() -> Result<(), String> {
        let outputs = display::outputs()?;
        let output = display::select_output(&outputs, None)?;
        let frame = Frame {
            x: f64::from(output.x) + (f64::from(output.width) - WIDTH) / 2.0,
            y: f64::from(output.y) + (f64::from(output.height) - HEIGHT) / 2.0,
            width: WIDTH,
            height: HEIGHT,
        };
        overlay::select_backend(frame, Layer::Settings)?;
        let app = SettingsWindow::new().map_err(|error| error.to_string())?;
        {
            let weak = app.as_weak();
            slint::spawn_local(async move {
                let Some(app) = weak.upgrade() else { return };
                if let Ok(window) = app.window().winit_window().await {
                    overlay::raise(&window, frame, Layer::Settings);
                }
            })
            .map_err(|error| error.to_string())?;
        }

        // 配置写坏时从默认值开始，第一次保存就会修好它。
        let (bindings, warning) = match hotkeys::load() {
            Ok(bindings) => (bindings, None),
            Err(error) => (Bindings::defaults(), Some(error)),
        };
        let recorder = Rc::new(RefCell::new(Recorder {
            bindings,
            recording: None,
            modifiers: Modifiers::empty(),
        }));
        show_bindings(&app, &recorder.borrow().bindings);
        match warning {
            Some(warning) => show_message(&app, &warning, true),
            None => show_message(&app, IDLE_MESSAGE, false),
        }

        {
            let weak = app.as_weak();
            let recorder = recorder.clone();
            app.on_record(move |index| {
                let (Some(app), Some(action)) = (weak.upgrade(), Action::from_index(index)) else {
                    return;
                };
                recorder.borrow_mut().recording = Some(action);
                app.set_recording(action.index());
                app.set_preview("".into());
                show_message(&app, RECORDING_MESSAGE, false);
            });
        }
        {
            let weak = app.as_weak();
            let recorder = recorder.clone();
            app.on_clear(move |index| {
                let (Some(app), Some(action)) = (weak.upgrade(), Action::from_index(index)) else {
                    return;
                };
                let mut recorder = recorder.borrow_mut();
                *recorder.slot(action) = None;
                save(&app, &recorder.bindings);
            });
        }
        {
            let weak = app.as_weak();
            let recorder = recorder.clone();
            app.on_restore_defaults(move || {
                let Some(app) = weak.upgrade() else { return };
                let mut recorder = recorder.borrow_mut();
                recorder.recording = None;
                recorder.bindings = Bindings::defaults();
                app.set_recording(0);
                save(&app, &recorder.bindings);
            });
        }
        {
            let weak = app.as_weak();
            app.window().on_winit_window_event(move |_window, event| {
                let Some(app) = weak.upgrade() else {
                    return EventResult::Propagate;
                };
                let mut recorder = recorder.borrow_mut();
                match event {
                    WindowEvent::ModifiersChanged(modifiers) => {
                        recorder.modifiers = hotkey_modifiers(modifiers.state());
                        if recorder.recording.is_some() {
                            app.set_preview(hotkeys::modifiers_label(recorder.modifiers).into());
                        }
                        EventResult::Propagate
                    }
                    // 录制期间所有按键都归录制框，不交给界面。
                    WindowEvent::KeyboardInput { event, .. } if recorder.recording.is_some() => {
                        if event.state == ElementState::Pressed
                            && !event.repeat
                            && let PhysicalKey::Code(code) = event.physical_key
                        {
                            record_key(&app, &mut recorder, &format!("{code:?}"));
                        }
                        EventResult::PreventDefault
                    }
                    _ => EventResult::Propagate,
                }
            });
        }

        app.run().map_err(|error| error.to_string())
    }

    fn record_key(app: &SettingsWindow, recorder: &mut Recorder, key: &str) {
        let Some(action) = recorder.recording else {
            return;
        };
        if hotkeys::is_modifier_key(key) {
            return;
        }
        if key == "Escape" && recorder.modifiers.is_empty() {
            recorder.recording = None;
            app.set_recording(0);
            show_message(app, IDLE_MESSAGE, false);
            return;
        }
        let recorded = hotkeys::recorded(key, recorder.modifiers)
            .ok_or_else(|| "不支持这个按键，请换一个。".to_string())
            .and_then(|hotkey| {
                hotkeys::validate(&hotkey, recorder.other(action).as_ref()).map(|()| hotkey)
            });
        match recorded {
            Ok(hotkey) => {
                *recorder.slot(action) = Some(hotkey);
                recorder.recording = None;
                app.set_recording(0);
                save(app, &recorder.bindings);
            }
            // 保持录制状态，让用户直接再按一次。
            Err(error) => {
                app.set_preview("".into());
                show_message(app, &error, true);
            }
        }
    }

    fn save(app: &SettingsWindow, bindings: &Bindings) {
        show_bindings(app, bindings);
        match hotkeys::save(bindings) {
            Ok(()) => show_message(app, SAVED_MESSAGE, false),
            Err(error) => show_message(app, &error, true),
        }
    }

    fn show_bindings(app: &SettingsWindow, bindings: &Bindings) {
        let label =
            |hotkey: &Option<HotKey>| hotkey.as_ref().map(hotkeys::label).unwrap_or_default();
        app.set_region_text(label(&bindings.region).into());
        app.set_screen_text(label(&bindings.screen).into());
    }

    fn show_message(app: &SettingsWindow, message: &str, is_error: bool) {
        app.set_message(message.into());
        app.set_message_is_error(is_error);
    }

    fn hotkey_modifiers(state: ModifiersState) -> Modifiers {
        let mut modifiers = Modifiers::empty();
        for (pressed, modifier) in [
            (state.control_key(), Modifiers::CONTROL),
            (state.alt_key(), Modifiers::ALT),
            (state.shift_key(), Modifiers::SHIFT),
            (state.super_key(), Modifiers::META),
        ] {
            if pressed {
                modifiers |= modifier;
            }
        }
        modifiers
    }
}
