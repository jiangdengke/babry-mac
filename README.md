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
| 拖动边框调整选区 | ✅ | 进入标注后拖动四条边或四个角点 |
| 保存 PNG | ✅ | 保存到 `~/Pictures/babry-时间.png` |
| 复制到剪贴板 | ✅ | 写入 `NSPasteboard`，类型为 PNG |
| 全屏覆盖窗口 | ✅ | 无边框窗口盖住菜单栏和 Dock，已实机验证 |
| Babry.app 与 DMG | ✅ | 通用版本；未经 Apple 公证，首次打开需手动允许 |
| 多显示器 | 🧪 | 截取鼠标所在的屏幕，或用 `--output` 指定 |
| 贴图 | ✅ | 浮在普通窗口之上，可移动、缩放、调整大小，可同时钉多张 |
| 全局快捷键、菜单栏常驻 | ❌ | 计划中；目前用 Raycast 绑定，见下文 |
| ScreenCaptureKit | ❌ | 目前调用系统的 `screencapture` |

## 安装

1. 从 [Releases](https://github.com/jiangdengke/babry-mac/releases/latest) 下载 `Babry-版本号.dmg` 并打开。Apple 芯片和 Intel Mac 都能运行。
2. 把 Babry 拖到“应用程序”文件夹。
3. 第一次打开时，macOS 会提示无法验证开发者，因为这个 App 没有经过 Apple 公证。到“系统设置 → 隐私与安全性”点“仍要打开”，或在终端运行 `xattr -dr com.apple.quarantine /Applications/Babry.app`。
4. 第一次截图时，在“系统设置 → 隐私与安全性 → 录屏与系统录音”中允许 Babry，然后重新打开 Babry。每次更新版本后可能需要重新允许。

### 从源码构建

需要 Rust 1.88 或更高版本（代码使用了 let chains）。

```bash
git clone https://github.com/jiangdengke/babry-mac.git
cd babry-mac
scripts/package-macos.sh        # 生成 dist/Babry-版本号.dmg
cargo run --release -- --screen # 开发时直接运行
```

## 运行

打开 Babry（双击、Spotlight、Launchpad 或 Raycast）就会立刻进入智能选区截图。需要参数时，用 `open` 传给 App：

```bash
open -a Babry                                 # CV 智能区域截图：悬停吸附，点击确认
open -a Babry --args --screen                 # 截取整个屏幕并直接进入编辑
open -a Babry --args --output NAME            # 在指定显示器截图，可与 --screen 组合
/Applications/Babry.app/Contents/MacOS/babry --list-outputs   # 列出显示器名称、位置和缩放
/Applications/Babry.app/Contents/MacOS/babry --help
```

直接在终端运行 App 里的 `babry` 也可以，但这时录屏权限算在终端身上。

## 绑定快捷键

Babry 还没有内置全局快捷键。上游在 niri 里用 `binds` 绑定，macOS 上可以用 [Raycast](https://www.raycast.com/) 的 Script Command 代替：

1. 按上文从 DMG 安装 Babry。
2. 打开 Raycast 设置 → Extensions → `+` → Add Script Directory，选择本仓库的 `scripts/raycast`。
3. 在列表里找到“Babry 智能截图”和“Babry 全屏截图”，分别点 Hotkey 设置快捷键。

脚本通过 `open -a Babry` 启动 App，所以录屏权限只需要给 Babry，不需要给 Raycast。

| 上游 niri 绑定 | 脚本 | 建议的 Mac 快捷键 |
| --- | --- | --- |
| `Mod+Shift+S` → `babry` | `babry-region.sh` | `⌥⇧S` |
| `Mod+Shift+F` → `babry --screen` | `babry-screen.sh` | `⌥⇧F` |

避开 `⌘⇧3/4/5`（系统截图）和 `⌘⇧S`（很多应用的“另存为”）。

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

### 贴图

点击工具栏的图钉，选区会保存到 `~/Pictures` 并钉在屏幕上；截图窗口关闭后贴图仍然保留。也可以把已有图片钉上去：`open -a Babry --args --pin /图片的/绝对路径.png`。

| 操作 | 效果 |
| --- | --- |
| 左键拖动 | 移动贴图 |
| 拖动边或角 | 调整大小，最小 64 点 |
| 滚轮 | 以中心缩放 |
| 右键或 `Esc` | 关闭这张贴图 |

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
| layer-shell 贴图 | 浮动层级的无边框窗口，每张贴图一个进程 | `src/pin.rs`、`src/pin_window.rs`、`ui/pin.slint` |
| `/usr/share/fonts`、`fc-match` | macOS 系统字体 | `src/editor.rs` |

## 路线图

- [x] 覆盖窗口、Retina、贴图、DMG 打包
- [ ] 外接显示器实测
- [ ] 通知改用 `UNUserNotificationCenter`，来源显示为 Babry
- [ ] 换成 ScreenCaptureKit；签名和公证；用窗口边框辅助 CV 吸附

## 开发

发布新版本：把 `Cargo.toml` 的版本改成新版本号并提交，再推送同名 tag，例如 `git tag v0.2.0 && git push origin v0.2.0`。`release` 工作流会编译通用版本的 Babry.app，打成 DMG 并创建 Release；本地可以用 `scripts/package-macos.sh` 试打包。App 图标由 `scripts/make-icon.sh` 从 logo 生成。

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

### 复现吸附问题

设置 `BABRY_DEBUG_DIR` 后，每次截图都会在该目录保存原始全屏图（`babry-debug-时间.png`）和选区记录（`babry-debug-时间.log`）。记录包括悬停、滚轮切换和最终选区。

```bash
open -a Babry --env BABRY_DEBUG_DIR=$HOME/babry-debug
/Applications/Babry.app/Contents/MacOS/babry --debug-selection ~/babry-debug/babry-debug-时间.png /tmp/cv   # 离线查看全部候选
```

## License

[Apache License 2.0](LICENSE)。上游版权归 Babry 的作者所有，详见 [NOTICE](NOTICE)。
