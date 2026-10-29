Babry 的 macOS 版。需要 macOS 14 或更高版本；通用版本，Apple 芯片和 Intel Mac 都能运行。

## 安装

1. 下载下方的 `Babry-版本号.dmg` 并打开。
2. 把 Babry 拖到“应用程序”文件夹。
3. 第一次打开时，macOS 会提示无法验证开发者，因为这个 App 没有经过 Apple 公证。请到“系统设置 → 隐私与安全性”，点“仍要打开”。也可以在终端运行：

   ```bash
   xattr -dr com.apple.quarantine /Applications/Babry.app
   ```

4. 第一次截图时，在“系统设置 → 隐私与安全性 → 录屏与系统录音”中允许 Babry，然后重新打开 Babry。

## 使用

- 打开 Babry（双击、Spotlight 或 Launchpad）就会立刻进入智能选区截图。
- 用 Raycast 绑定快捷键的方法见 README。

每次更新版本后，可能需要重新允许录屏权限。
