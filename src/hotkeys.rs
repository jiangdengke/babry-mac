//! 全局快捷键配置，存放在 `hotkeys.json`，格式如 `"ctrl+alt+s"`，空字符串表示不绑定。
//! 平时用设置窗口录制（`src/settings_window.rs`），这里负责解析、校验和读写。

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use serde_json::Value;
use std::path::PathBuf;

// 避开 ⌘⇧3/4/5（系统截图）、⌘⇧S（另存为）、⌥⇧F（编辑器里的格式化文档）
// 和 ⌃⇧ 系列（kitty 等终端的默认前缀）。
const DEFAULT_REGION: &str = "ctrl+alt+s";
const DEFAULT_SCREEN: &str = "ctrl+alt+f";

#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    pub region: Option<HotKey>,
    pub screen: Option<HotKey>,
}

impl Bindings {
    pub fn defaults() -> Self {
        // 默认值是上面的常量，单元测试保证它们能解析。
        parse_config("{}").unwrap_or(Self {
            region: None,
            screen: None,
        })
    }
}

/// 缺少的项使用默认值。两个操作不能绑定同一组按键，否则只有一个会生效。
pub fn parse_config(text: &str) -> Result<Bindings, String> {
    let config: Value =
        serde_json::from_str(text).map_err(|error| format!("配置不是有效的 JSON: {error}"))?;
    let region = binding(&config, "region", DEFAULT_REGION)?;
    let screen = binding(&config, "screen", DEFAULT_SCREEN)?;
    if region.is_some() && region == screen {
        return Err("region 和 screen 不能使用同一个快捷键".into());
    }
    Ok(Bindings { region, screen })
}

fn binding(config: &Value, name: &str, default: &str) -> Result<Option<HotKey>, String> {
    let text = match config.get(name) {
        None => default,
        Some(Value::String(text)) => text,
        Some(_) => return Err(format!("{name} 必须是字符串")),
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    text.parse()
        .map(Some)
        .map_err(|error| format!("{name} 的快捷键 \"{text}\" 无效: {error}"))
}

/// 配置目录，同时存放快捷键配置和单实例锁。
pub fn config_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("找不到用户目录")?;
    Ok(PathBuf::from(home).join("Library/Application Support/Babry"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("hotkeys.json"))
}

/// 读取配置；还没有配置文件时使用默认值。
pub fn load() -> Result<Bindings, String> {
    match std::fs::read_to_string(config_path()?) {
        Ok(text) => parse_config(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Bindings::defaults()),
        Err(error) => Err(format!("无法读取快捷键配置: {error}")),
    }
}

/// 确保配置文件存在，返回路径和这次是否新建了它。
pub fn ensure_config_file() -> Result<(PathBuf, bool), String> {
    let path = config_path()?;
    if path.exists() {
        return Ok((path, false));
    }
    save(&Bindings::defaults())?;
    Ok((path, true))
}

pub fn save(bindings: &Bindings) -> Result<(), String> {
    let path = config_path()?;
    std::fs::create_dir_all(config_dir()?)
        .and_then(|()| std::fs::write(&path, config_text(bindings)))
        .map_err(|error| format!("无法保存快捷键配置: {error}"))
}

fn config_text(bindings: &Bindings) -> String {
    let text = |hotkey: &Option<HotKey>| hotkey.as_ref().map(config_string).unwrap_or_default();
    format!(
        "{{\n  \"region\": \"{}\",\n  \"screen\": \"{}\"\n}}\n",
        text(&bindings.region),
        text(&bindings.screen)
    )
}

/// 配置文件里的写法，比如 `ctrl+alt+s`。修饰键的顺序与 `label` 相同。
fn config_string(hotkey: &HotKey) -> String {
    let names = [
        (Modifiers::CONTROL, "ctrl+"),
        (Modifiers::ALT, "alt+"),
        (Modifiers::SHIFT, "shift+"),
        (Modifiers::META, "cmd+"),
    ];
    let mut text: String = names
        .iter()
        .filter(|(modifier, _)| hotkey.mods.contains(*modifier))
        .map(|(_, name)| *name)
        .collect();
    text.push_str(&key_name(hotkey.key).to_lowercase());
    text
}

/// 修饰键本身不能单独绑定，录制时按下它们只更新提示。
pub fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "ShiftLeft"
            | "ShiftRight"
            | "ControlLeft"
            | "ControlRight"
            | "AltLeft"
            | "AltRight"
            | "SuperLeft"
            | "SuperRight"
            | "MetaLeft"
            | "MetaRight"
            | "Meta"
            | "Super"
            | "Hyper"
            | "Fn"
            | "FnLock"
            | "CapsLock"
    )
}

/// 由录制到的物理按键名（如 `KeyS`）和修饰键生成快捷键。
pub fn recorded(key: &str, modifiers: Modifiers) -> Option<HotKey> {
    let code: Code = key.parse().ok()?;
    Some(HotKey::new(Some(modifiers), code))
}

/// 录制结果能否使用：要带修饰键，不能和另一个操作或系统快捷键重复。
pub fn validate(hotkey: &HotKey, other: Option<&HotKey>) -> Result<(), String> {
    let is_function_key = key_equivalent(hotkey.key).is_some_and(|key| {
        key.chars()
            .next()
            .is_some_and(|character| ('\u{F704}'..='\u{F70F}').contains(&character))
    });
    let has_modifier = hotkey
        .mods
        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::META);
    if !has_modifier && !is_function_key {
        return Err("要带 ⌃、⌥ 或 ⌘，否则平时打字会被拦截。".into());
    }
    if other == Some(hotkey) {
        return Err(format!("{} 已经用于另一个操作。", label(hotkey)));
    }
    const RESERVED: [&str; 5] = [
        "cmd+shift+3",
        "cmd+shift+4",
        "cmd+shift+5",
        "cmd+space",
        "ctrl+space",
    ];
    let is_reserved = RESERVED.iter().any(|text| {
        text.parse::<HotKey>()
            .is_ok_and(|reserved| reserved == *hotkey)
    });
    if is_reserved {
        return Err(format!("{} 是系统快捷键，请换一个。", label(hotkey)));
    }
    Ok(())
}

/// 修饰键的符号，按系统菜单的顺序排列。录制时用来显示已按住的键。
pub fn modifiers_label(modifiers: Modifiers) -> String {
    [
        (Modifiers::CONTROL, "⌃"),
        (Modifiers::ALT, "⌥"),
        (Modifiers::SHIFT, "⇧"),
        (Modifiers::META, "⌘"),
    ]
    .iter()
    .filter(|(modifier, _)| modifiers.contains(*modifier))
    .map(|(_, symbol)| *symbol)
    .collect()
}

// 字母和数字去掉前缀，其他键保留名字。
fn key_name(key: Code) -> String {
    let name = key.to_string();
    name.strip_prefix("Key")
        .or_else(|| name.strip_prefix("Digit"))
        .unwrap_or(&name)
        .to_string()
}

/// 给人看的写法，比如 `⌃⌥S`。
pub fn label(hotkey: &HotKey) -> String {
    modifiers_label(hotkey.mods) + &key_name(hotkey.key)
}

/// 菜单项右侧显示按键用的字符。没有对应字符的键返回 None，菜单上就不显示。
pub fn key_equivalent(key: Code) -> Option<String> {
    let name = key.to_string();
    if name == "Space" {
        return Some(" ".into());
    }
    if let Some(letter) = name.strip_prefix("Key") {
        return Some(letter.to_lowercase());
    }
    if let Some(digit) = name.strip_prefix("Digit") {
        return Some(digit.to_string());
    }
    // AppKit 把 F1 到 F12 映射到私有区 U+F704 开始的连续字符。
    let number: u32 = name.strip_prefix('F')?.parse().ok()?;
    (1..=12)
        .contains(&number)
        .then(|| char::from_u32(0xF704 + number - 1))
        .flatten()
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcuts_parse() {
        let bindings = Bindings::defaults();
        assert_eq!(label(&bindings.region.unwrap()), "⌃⌥S");
        assert_eq!(label(&bindings.screen.unwrap()), "⌃⌥F");
    }

    #[test]
    fn config_overrides_defaults_and_empty_string_disables() {
        let bindings = parse_config(r#"{"region": "alt+shift+s", "screen": ""}"#).unwrap();
        assert_eq!(label(&bindings.region.unwrap()), "⌥⇧S");
        assert_eq!(bindings.screen, None);
    }

    #[test]
    fn missing_entries_keep_their_default() {
        let bindings = parse_config(r#"{"screen": "cmd+f1"}"#).unwrap();
        assert_eq!(label(&bindings.region.unwrap()), "⌃⌥S");
        assert_eq!(label(&bindings.screen.unwrap()), "⌘F1");
    }

    #[test]
    fn invalid_config_is_reported() {
        assert!(parse_config("not json").is_err());
        assert!(parse_config(r#"{"region": 1}"#).is_err());
        let error = parse_config(r#"{"region": "ctrl+nope"}"#).unwrap_err();
        assert!(error.contains("ctrl+nope"), "{error}");
    }

    #[test]
    fn the_same_shortcut_cannot_be_bound_twice() {
        let error = parse_config(r#"{"region": "ctrl+a", "screen": "ctrl+a"}"#).unwrap_err();
        assert!(error.contains("同一个"), "{error}");
        assert!(parse_config(r#"{"region": "", "screen": ""}"#).is_ok());
    }

    #[test]
    fn saved_config_parses_back_to_the_same_shortcuts() {
        let keys = [
            "KeyS",
            "Digit7",
            "F5",
            "Space",
            "ArrowLeft",
            "Backquote",
            "Minus",
            "Slash",
        ];
        for key in keys {
            let mods = Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::META;
            let bindings = Bindings {
                region: recorded(key, mods),
                screen: None,
            };
            assert!(bindings.region.is_some(), "{key}");
            assert_eq!(parse_config(&config_text(&bindings)), Ok(bindings), "{key}");
        }
    }

    #[test]
    fn saved_config_is_readable() {
        assert_eq!(
            config_text(&Bindings::defaults()),
            "{\n  \"region\": \"ctrl+alt+s\",\n  \"screen\": \"ctrl+alt+f\"\n}\n"
        );
    }

    #[test]
    fn recording_needs_a_modifier_except_for_function_keys() {
        let key = |name, mods| recorded(name, mods).unwrap();
        assert!(validate(&key("KeyA", Modifiers::empty()), None).is_err());
        assert!(validate(&key("KeyA", Modifiers::SHIFT), None).is_err());
        assert!(validate(&key("KeyA", Modifiers::ALT), None).is_ok());
        assert!(validate(&key("F6", Modifiers::empty()), None).is_ok());
    }

    #[test]
    fn recording_rejects_duplicates_and_system_shortcuts() {
        let cmd_shift = Modifiers::META | Modifiers::SHIFT;
        let hotkey = recorded("KeyK", Modifiers::CONTROL).unwrap();
        assert!(validate(&hotkey, Some(&hotkey)).is_err());
        assert!(validate(&recorded("Digit4", cmd_shift).unwrap(), None).is_err());
        assert!(validate(&recorded("Space", Modifiers::META).unwrap(), None).is_err());
        assert!(validate(&recorded("Digit7", cmd_shift).unwrap(), None).is_ok());
    }

    #[test]
    fn modifier_keys_cannot_be_recorded_alone() {
        assert!(is_modifier_key("ShiftLeft"));
        assert!(is_modifier_key("SuperRight"));
        assert!(!is_modifier_key("KeyS"));
        assert_eq!(recorded("NotAKey", Modifiers::ALT), None);
    }

    #[test]
    fn menu_shows_letters_digits_and_function_keys() {
        assert_eq!(key_equivalent(Code::KeyS).as_deref(), Some("s"));
        assert_eq!(key_equivalent(Code::Digit3).as_deref(), Some("3"));
        assert_eq!(key_equivalent(Code::F1), Some("\u{F704}".to_string()));
        assert_eq!(key_equivalent(Code::F12), Some("\u{F70F}".to_string()));
        assert_eq!(key_equivalent(Code::Space).as_deref(), Some(" "));
        assert_eq!(key_equivalent(Code::ArrowLeft), None);
    }
}
