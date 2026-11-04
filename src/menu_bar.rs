//! 菜单栏常驻进程：状态栏图标、全局快捷键和开机启动。
//!
//! 截图和设置窗口都在独立的子进程里，退出后内存全部释放；常驻进程不初始化 Slint，占用很小。
//! 子进程用同一个可执行文件启动，录屏权限算在 Babry 身上。

#[cfg(target_os = "macos")]
pub use macos::run;

#[cfg(not(target_os = "macos"))]
pub fn run() -> Result<(), String> {
    Err("babry-mac 只支持 macOS".into())
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::{
        capture,
        hotkeys::{self, Bindings},
    };
    use global_hotkey::{
        GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
        hotkey::{HotKey, Modifiers},
    };
    use objc2::{
        AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
        rc::Retained,
        runtime::{AnyObject, ProtocolObject},
        sel,
    };
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
        NSControlStateValueOff, NSControlStateValueOn, NSEventModifierFlags, NSImage, NSMenu,
        NSMenuDelegate, NSMenuItem, NSStatusBar, NSVariableStatusItemLength,
    };
    use objc2_foundation::{
        NSBundle, NSData, NSObject, NSObjectNSThreadPerformAdditions, NSObjectProtocol, NSSize,
        NSString, ns_string,
    };
    use objc2_service_management::{SMAppService, SMAppServiceStatus};
    use std::{
        cell::RefCell,
        fs::{File, OpenOptions, TryLockError},
        process::{Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };

    // 菜单收起有一段淡出动画，等它结束再截图，否则菜单会出现在截图里。
    const MENU_CLOSE_DELAY: Duration = Duration::from_millis(200);
    // 由 scripts/make-icon.sh 从 docs/images/babry-menubar.svg 生成。
    const ICON: &[u8] = include_bytes!("../packaging/MenuBarIcon.png");
    const ICON_SIZE: f64 = 18.0;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Mode {
        Region,
        Screen,
    }

    impl Mode {
        fn argument(self) -> &'static str {
            match self {
                Self::Region => "--region",
                Self::Screen => "--screen",
            }
        }
    }

    /// 等待子进程的线程把结果交回主线程时用它带上代理对象。
    struct MainThreadTarget(*const MenuBar);

    // SAFETY: 只用来调用线程安全的 performSelectorOnMainThread；代理在进程退出前一直存活。
    unsafe impl Send for MainThreadTarget {}

    /// 启动截图子进程，同一时间只允许一个。
    #[derive(Clone, Default)]
    struct Launcher {
        busy: Arc<AtomicBool>,
    }

    impl Launcher {
        /// 编辑窗口开着时忽略新的请求：再截一张只会截到编辑窗口自己。
        fn launch(&self, mode: Mode, delay: Duration) {
            if self.busy.swap(true, Ordering::SeqCst) {
                return;
            }
            let busy = self.busy.clone();
            thread::spawn(move || {
                thread::sleep(delay);
                let result = std::env::current_exe().and_then(|exe| {
                    Command::new(exe)
                        .arg(mode.argument())
                        .stdin(Stdio::null())
                        .status()
                });
                // 子进程自己会报告截图失败，这里只处理启动不了的情况。
                if let Err(error) = result {
                    capture::notify(&format!("无法启动截图: {error}"));
                }
                busy.store(false, Ordering::SeqCst);
            });
        }
    }

    struct State {
        launcher: Launcher,
        manager: GlobalHotKeyManager,
        registered: RefCell<Vec<HotKey>>,
        // 快捷键事件的回调要求 Send + Sync，所以按键和操作的对应关系放在锁里。
        actions: Arc<Mutex<Vec<(u32, Mode)>>>,
        settings_open: Arc<AtomicBool>,
        region_item: Retained<NSMenuItem>,
        screen_item: Retained<NSMenuItem>,
        login_item: Retained<NSMenuItem>,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "BabryMenuBar"]
        #[ivars = State]
        struct MenuBar;

        unsafe impl NSObjectProtocol for MenuBar {}

        unsafe impl NSApplicationDelegate for MenuBar {
            // 已经在运行时再次打开 App（双击、Spotlight、open -a），直接开始智能截图。
            #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
            fn should_handle_reopen(&self, _app: &NSApplication, _has_windows: bool) -> bool {
                self.ivars().launcher.launch(Mode::Region, Duration::ZERO);
                false
            }
        }

        unsafe impl NSMenuDelegate for MenuBar {
            // 开机启动也可能在系统设置里被改掉，所以每次打开菜单都重新读取。
            #[unsafe(method(menuWillOpen:))]
            fn menu_will_open(&self, _menu: &NSMenu) {
                self.refresh_login_item();
            }
        }

        impl MenuBar {
            #[unsafe(method(captureRegion:))]
            fn capture_region(&self, _sender: Option<&AnyObject>) {
                self.ivars().launcher.launch(Mode::Region, MENU_CLOSE_DELAY);
            }

            #[unsafe(method(captureScreen:))]
            fn capture_screen(&self, _sender: Option<&AnyObject>) {
                self.ivars().launcher.launch(Mode::Screen, MENU_CLOSE_DELAY);
            }

            #[unsafe(method(toggleLogin:))]
            fn toggle_login(&self, _sender: Option<&AnyObject>) {
                self.switch_login_item();
                self.refresh_login_item();
            }

            #[unsafe(method(openSettings:))]
            fn open_settings(&self, _sender: Option<&AnyObject>) {
                let state = self.ivars();
                if state.settings_open.swap(true, Ordering::SeqCst) {
                    return;
                }
                // 已绑定的组合键会被系统先拿走，设置窗口就录不到了，所以窗口开着时先注销。
                self.suspend();
                let target = MainThreadTarget(self);
                let settings_open = state.settings_open.clone();
                thread::spawn(move || {
                    let target = target;
                    let result = std::env::current_exe().and_then(|exe| {
                        Command::new(exe)
                            .arg("--settings")
                            .stdin(Stdio::null())
                            .status()
                    });
                    if let Err(error) = result {
                        capture::notify(&format!("无法打开设置窗口: {error}"));
                    }
                    settings_open.store(false, Ordering::SeqCst);
                    // SAFETY: 见 MainThreadTarget；selector 由 MenuBar 定义，不带参数对象。
                    unsafe {
                        (*target.0).performSelectorOnMainThread_withObject_waitUntilDone(
                            sel!(settingsClosed:),
                            None,
                            false,
                        );
                    }
                });
            }

            #[unsafe(method(settingsClosed:))]
            fn settings_closed(&self, _argument: Option<&AnyObject>) {
                self.apply(&hotkeys::load().unwrap_or_else(|error| {
                    capture::notify(&error);
                    Bindings::defaults()
                }));
            }

            #[unsafe(method(quit:))]
            fn quit(&self, _sender: Option<&AnyObject>) {
                NSApplication::sharedApplication(self.mtm()).terminate(None);
            }
        }
    );

    impl MenuBar {
        fn new(mtm: MainThreadMarker, state: State) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(state);
            // SAFETY: NSObject 的 init 没有额外要求。
            unsafe { msg_send![super(this), init] }
        }

        /// 注销全部快捷键。
        fn suspend(&self) {
            let state = self.ivars();
            let old = state.registered.take();
            if let Err(error) = state.manager.unregister_all(&old) {
                eprintln!("注销快捷键失败: {error}");
            }
        }

        /// 换上一组新的快捷键，并同步菜单上显示的按键。注册失败的不显示。
        fn apply(&self, bindings: &Bindings) {
            self.suspend();
            let state = self.ivars();
            let mut actions = Vec::new();
            let mut registered = Vec::new();
            let entries = [
                (bindings.region, Mode::Region, &state.region_item),
                (bindings.screen, Mode::Screen, &state.screen_item),
            ];
            for (hotkey, mode, item) in entries {
                let active = hotkey.filter(|hotkey| match state.manager.register(*hotkey) {
                    Ok(()) => true,
                    Err(error) => {
                        capture::notify(&format!(
                            "快捷键 {} 注册失败: {error}",
                            hotkeys::label(hotkey)
                        ));
                        false
                    }
                });
                if let Some(hotkey) = active {
                    actions.push((hotkey.id(), mode));
                    registered.push(hotkey);
                }
                show_shortcut(item, active.as_ref());
            }
            *state
                .actions
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = actions;
            *state.registered.borrow_mut() = registered;
        }

        fn switch_login_item(&self) {
            // SAFETY: 主线程调用，返回的对象由 Retained 管理。
            let service = unsafe { SMAppService::mainAppService() };
            // SAFETY: 这些方法没有额外的调用要求。
            let status = unsafe { service.status() };
            let result = if status == SMAppServiceStatus::Enabled {
                unsafe { service.unregisterAndReturnError() }
            } else if status == SMAppServiceStatus::RequiresApproval {
                // 用户在系统设置里关掉过，只能由用户在那里重新打开。
                unsafe { SMAppService::openSystemSettingsLoginItems() };
                Ok(())
            } else {
                unsafe { service.registerAndReturnError() }
            };
            if let Err(error) = result {
                capture::notify(&format!(
                    "无法设置开机启动: {}",
                    error.localizedDescription()
                ));
            }
        }

        fn refresh_login_item(&self) {
            // SAFETY: 主线程调用，没有额外要求。
            let enabled =
                unsafe { SMAppService::mainAppService().status() } == SMAppServiceStatus::Enabled;
            let state = if enabled {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            };
            self.ivars().login_item.setState(state);
        }
    }

    pub fn run() -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("菜单栏必须在主线程运行")?;
        // 同时存在两个常驻进程时，一次快捷键会截两张图。锁随进程退出自动释放。
        let Some(_lock) = acquire_lock()? else {
            capture::notify("Babry 已经在菜单栏运行");
            return Ok(());
        };

        let app = NSApplication::sharedApplication(mtm);
        // 不显示 Dock 图标。打包后 Info.plist 的 LSUIElement 有同样效果，这里照顾终端启动。
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        let launcher = Launcher::default();
        let actions: Arc<Mutex<Vec<(u32, Mode)>>> = Arc::default();
        {
            let launcher = launcher.clone();
            let actions = actions.clone();
            GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
                if event.state() != HotKeyState::Pressed {
                    return;
                }
                let mode = actions
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .iter()
                    .find(|(id, _)| *id == event.id())
                    .map(|(_, mode)| *mode);
                if let Some(mode) = mode {
                    launcher.launch(mode, Duration::ZERO);
                }
            }));
        }

        let menu = NSMenu::new(mtm);
        let region_item = menu_item(mtm, "智能截图", sel!(captureRegion:));
        let screen_item = menu_item(mtm, "全屏截图", sel!(captureScreen:));
        let login_item = menu_item(mtm, "开机时启动", sel!(toggleLogin:));
        let settings_item = menu_item(mtm, "设置快捷键…", sel!(openSettings:));
        let quit_item = menu_item(mtm, "退出 Babry", sel!(quit:));
        // 开机启动只对打包后的 App 有意义，从终端运行时没有 App 可以注册。
        login_item.setHidden(NSBundle::mainBundle().bundleIdentifier().is_none());

        let delegate = MenuBar::new(
            mtm,
            State {
                launcher,
                manager: GlobalHotKeyManager::new()
                    .map_err(|error| format!("无法初始化全局快捷键: {error}"))?,
                registered: RefCell::default(),
                actions,
                settings_open: Arc::default(),
                region_item: region_item.clone(),
                screen_item: screen_item.clone(),
                login_item: login_item.clone(),
            },
        );
        let target: &AnyObject = delegate.as_ref();
        for item in [
            &region_item,
            &screen_item,
            &login_item,
            &settings_item,
            &quit_item,
        ] {
            // SAFETY: delegate 在 run 期间一直存活，菜单项不会悬空指向它。
            unsafe { item.setTarget(Some(target)) };
        }
        menu.addItem(&region_item);
        menu.addItem(&screen_item);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&login_item);
        menu.addItem(&settings_item);
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&quit_item);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        delegate.refresh_login_item();

        let status_item =
            NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = status_item.button(mtm) {
            let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(ICON));
            if let Some(image) = &image {
                image.setSize(NSSize::new(ICON_SIZE, ICON_SIZE));
                // 模板图只用透明度，系统按菜单栏的深浅色着色，和其他图标保持一致。
                image.setTemplate(true);
            }
            button.setImage(image.as_deref());
            // 图片加载不了时还有文字，菜单不至于点不到。
            if image.is_none() {
                button.setTitle(ns_string!("Babry"));
            }
        }
        status_item.setMenu(Some(&menu));
        app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        let bindings = hotkeys::load().unwrap_or_else(|error| {
            capture::notify(&error);
            Bindings::defaults()
        });
        delegate.apply(&bindings);

        // 第一次运行时提示一次：菜单栏图标在刘海屏上可能被挡住，光靠图标不容易发现。
        match hotkeys::ensure_config_file() {
            Ok((_, true)) => capture::notify(&format!(
                "Babry 已在菜单栏运行：{}",
                shortcut_summary(&bindings)
            )),
            Ok((_, false)) => {}
            Err(error) => capture::notify(&error),
        }

        app.run();
        Ok(())
    }

    fn acquire_lock() -> Result<Option<File>, String> {
        let directory = hotkeys::config_dir()?;
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(directory.join("menu-bar.lock"))
            .map_err(|error| error.to_string())?;
        match file.try_lock() {
            Ok(()) => Ok(Some(file)),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error.to_string()),
        }
    }

    fn menu_item(
        mtm: MainThreadMarker,
        title: &str,
        action: objc2::runtime::Sel,
    ) -> Retained<NSMenuItem> {
        // SAFETY: action 对应 MenuBar 上定义的方法，调用前会把 target 设为 MenuBar。
        unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                Some(action),
                ns_string!(""),
            )
        }
    }

    fn show_shortcut(item: &NSMenuItem, hotkey: Option<&HotKey>) {
        let shown = hotkey.and_then(|hotkey| {
            hotkeys::key_equivalent(hotkey.key).map(|key| (key, modifier_flags(hotkey.mods)))
        });
        match shown {
            Some((key, flags)) => {
                item.setKeyEquivalent(&NSString::from_str(&key));
                item.setKeyEquivalentModifierMask(flags);
            }
            None => {
                item.setKeyEquivalent(ns_string!(""));
                item.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
            }
        }
    }

    fn modifier_flags(modifiers: Modifiers) -> NSEventModifierFlags {
        let mut flags = NSEventModifierFlags::empty();
        for (modifier, flag) in [
            (Modifiers::CONTROL, NSEventModifierFlags::Control),
            (Modifiers::ALT, NSEventModifierFlags::Option),
            (Modifiers::SHIFT, NSEventModifierFlags::Shift),
            (Modifiers::META, NSEventModifierFlags::Command),
        ] {
            if modifiers.contains(modifier) {
                flags |= flag;
            }
        }
        flags
    }

    fn shortcut_summary(bindings: &Bindings) -> String {
        let describe = |name: &str, hotkey: Option<HotKey>| match hotkey {
            Some(hotkey) => format!("{name} {}", hotkeys::label(&hotkey)),
            None => format!("{name}未绑定"),
        };
        format!(
            "{}，{}",
            describe("智能截图", bindings.region),
            describe("全屏截图", bindings.screen)
        )
    }
}
