Babry 的 macOS 版，通用二进制，Apple 芯片和 Intel Mac 都能运行。

## 安装

```bash
curl -fsSL https://github.com/jiangdengke/babry-mac/releases/latest/download/babry-macos-universal.tar.gz | tar -xz -C /tmp
sudo install -m 755 /tmp/babry/babry /usr/local/bin/babry
babry --help
```

如果用浏览器下载压缩包，macOS 会给程序加上隔离标记，导致无法运行。解压后先运行：

```bash
xattr -d com.apple.quarantine babry/babry
```

第一次截图时，请在“系统设置 → 隐私与安全性 → 录屏与系统录音”中允许启动 babry 的终端，然后重启终端。

压缩包里的 `raycast` 目录是 Raycast 脚本，可以用来绑定截图快捷键，用法见 README。
