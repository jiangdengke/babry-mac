#!/bin/bash

# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Babry 全屏截图
# @raycast.mode silent

# Optional parameters:
# @raycast.icon 🖥️
# @raycast.packageName Babry
# @raycast.description 截取整个屏幕并直接进入标注

# 通过 open 启动 Babry.app，录屏权限算在 Babry 自己身上，而不是 Raycast。
# Babry 正在截图时再次触发只会切回当前窗口，不会叠加第二个。
if ! open -a Babry --args --screen 2>/dev/null; then
  echo "找不到 Babry.app，请先从 DMG 安装"
  exit 1
fi
