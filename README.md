<div align="center">
  <img src="docs/images/babry-mark.svg" width="112" alt="Babry logo">

  # Babry for macOS

  **[Babry](https://github.com/kregonia/babry) 的 macOS 移植：一款懂边界的截图工具。**

  悬停吸附 · 单击确认 · 拖动精修 · 原位标注 · 一键复制
</div>

<br>

<div align="center">
  <img src="docs/images/selection-editor.png" alt="Babry 编辑器与智能选区预览" width="92%">
  <br>
  <sub>界面与上游一致，截图来自上游 Linux 版。</sub>
</div>

## 来源

本项目移植自 [kregonia/babry](https://github.com/kregonia/babry)（Apache-2.0）。第一个提交原样导入上游 `497f87d`，后续改动见 git 历史和 [NOTICE](NOTICE)。

CV 智能选区、标注渲染、撤销历史、命令行解析和 Slint 界面直接沿用上游代码；本仓库只替换了和 Linux / Wayland 绑定的部分。

## 当前状态

| 功能 | 状态 | 说明 |
| --- | --- | --- |
| CV 智能选区 | ✅ | 沿用上游，单测在 macOS 上通过 |
| 标注、文字、马赛克/模糊 | ✅ | 文字默认使用系统自带的冬青黑体 |
| 保存 PNG | ✅ | 保存到 `~/Pictures/babry-时间.png` |
| 复制到剪贴板 | ✅ | 写入 `NSPasteboard`，类型为 PNG |
| 全屏覆盖窗口 | 🧪 | 无边框窗口盖住菜单栏和 Dock，需要实机验证 |
| 多显示器 | 🧪 | 截取鼠标所在的屏幕，或用 `--output` 指定 |
| 贴图 | ❌ | 计划用置顶的 `NSPanel` 实现 |
| 全局快捷键、菜单栏常驻 | ❌ | 计划中；目前用 Raycast 绑定，见下文 |
| ScreenCaptureKit、.app 打包 | ❌ | 目前调用系统的 `screencapture` |

## 安装

从 [Releases](https://github.com/jiangdengke/babry-mac/releases) 下载编译好的通用二进制，Apple 芯片和 Intel Mac 都能运行，不需要 Rust：

```bash
curl -fsSL https://github.com/jiangdengke/babry-mac/releases/latest/download/babry-macos-universal.tar.gz | tar -xz -C /tmp
sudo install -m 755 /tmp/babry/babry /usr/local/bin/babry
```

用浏览器下载时，macOS 会给程序加上隔离标记，导致无法运行；解压后先运行 `xattr -d com.apple.quarantine babry/babry`。

### 从源码构建

需要 Rust 1.88 或更高版本（代码使用了 let chains）。

```bash
git clone https://github.com/jiangdengke/babry-mac.git
cd babry-mac
cargo install --path . --locked   # 装到 ~/.cargo/bin/babry
```

## 运行

第一次截图时，macOS 会要求录屏权限。请在“系统设置 → 隐私与安全性 → 录屏与系统录音”中允许你使用的终端（Terminal、iTerm2、Ghostty 等），然后重启终端。

```bash
babry                  # CV 智能区域截图：悬停吸附，单击确认
babry --screen         # 截取整个屏幕并直接进入编辑
babry --output NAME    # 在指定显示器截图，可与 --screen 组合
babry --list-outputs   # 列出显示器名称、位置和缩放
babry --debug-selection screenshot.png /tmp/babry-cv-debug  # 离线检查 CV 选区
babry --help
```

## 绑定快捷键

babry 还没有内置全局快捷键。上游在 niri 里用 `binds` 绑定，macOS 上可以用 [Raycast](https://www.raycast.com/) 的 Script Command 代替：

1. 按上文安装 `babry`。脚本会在 `/usr/local/bin`、`~/.cargo/bin` 等常见位置查找它。
2. 打开 Raycast 设置 → Extensions → `+` → Add Script Directory，选择本仓库的 `scripts/raycast`，或 Release 压缩包里的 `raycast` 目录。
3. 在列表里找到“Babry 智能截图”和“Babry 全屏截图”，分别点 Hotkey 设置快捷键。
4. 第一次按快捷键时，在“录屏与系统录音”里允许 Raycast，然后退出并重新打开 Raycast。

| 上游 niri 绑定 | 脚本 | 建议的 Mac 快捷键 |
| --- | --- | --- |
| `Mod+Shift+S` → `babry` | `babry-region.sh` | `⌥⇧S` |
| `Mod+Shift+F` → `babry --screen` | `babry-screen.sh` | `⌥⇧F` |

避开 `⌘⇧3/4/5`（系统截图）和 `⌘⇧S`（很多应用的“另存为”）。脚本不读取 `~/.zshrc`，需要调试记录时，取消脚本里 `BABRY_DEBUG_DIR` 那一行的注释。

## 交互方式

### 选区阶段

1. 移动鼠标：吸附到窗口、面板或控件。
2. 滚轮向上：扩大到父区域；向下：缩小到子区域。
3. 单击：确认当前选区，进入编辑。
4. 按住拖动：切换为手动框选。
5. `Esc`：取消截图。

### 编辑阶段

| 操作 | 快捷键 |
| --- | --- |
| 撤销 / 重做 | `⌘Z` / `⌘⇧Z` 或 `⌘Y` |
| 保存 | `⌘S` |
| 复制 | `⌘C` |
| 确认文字 | `⌘↩`，或点击画布外 |
| 取消文字或截图 | `Esc` |

Slint 在 macOS 上把 `Control` 映射为 `⌘`，所以界面文件里的快捷键不用改。

## 字体

文字标注依次尝试：`BABRY_FONT` 环境变量、冬青黑体（Hiragino Sans GB）、华文黑体（STHeiti）、Arial Unicode、Helvetica。

```bash
export BABRY_FONT="/System/Library/Fonts/STHeiti Medium.ttc"
export BABRY_FONT_INDEX=0   # .ttc 字体集合中的索引
```

## 与上游的差异

| 上游（Linux） | 本仓库（macOS） | 文件 |
| --- | --- | --- |
| `grim` 截屏 | `screencapture -R` 按屏幕矩形截取 | `src/capture.rs` |
| `wl-copy` | `NSPasteboard` | `src/capture.rs` |
| `notify-send` | `osascript` 通知 | `src/capture.rs` |
| niri / Hyprland IPC | `NSScreen` | `src/display.rs` |
| niri `open-fullscreen` 窗口规则 | 无边框窗口，层级高于菜单栏 | `src/overlay.rs` |
| layer-shell 贴图 | 待实现 | `src/pin.rs` |
| `/usr/share/fonts`、`fc-match` | macOS 系统字体 | `src/editor.rs` |

## 路线图

- [ ] P1：实机验证覆盖窗口、多屏和 Retina；处理焦点和输入法问题
- [ ] P2：菜单栏常驻、全局快捷键、`NSPanel` 贴图窗口
- [ ] P3：换成 ScreenCaptureKit；打包 `.app`，签名和公证；用窗口边框辅助 CV 吸附

## 开发

发布新版本：把 `Cargo.toml` 的版本改成新版本号并提交，再推送同名 tag，例如 `git tag v0.2.0 && git push origin v0.2.0`。`release` 工作流会编译通用二进制并创建 Release；本地可以用 `scripts/package-macos.sh` 试打包。

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

### 复现吸附问题

设置 `BABRY_DEBUG_DIR` 后，每次截图都会在该目录保存原始全屏图（`babry-debug-时间.png`）和选区记录（`babry-debug-时间.log`）。记录包括悬停、滚轮切换和最终选区。

```bash
BABRY_DEBUG_DIR=~/babry-debug babry
babry --debug-selection ~/babry-debug/babry-debug-时间.png /tmp/cv   # 离线查看全部候选
```

## License

[Apache License 2.0](LICENSE)。上游版权归 Babry 的作者所有，详见 [NOTICE](NOTICE)。
