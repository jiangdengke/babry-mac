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
# Babry 常驻菜单栏时，不带 -n 的 open 会丢掉参数，所以每次启动一个新实例。
if ! open -n -a Babry --args --screen 2>/dev/null; then
  echo "找不到 Babry.app，请先从 DMG 安装"
  exit 1
fi
