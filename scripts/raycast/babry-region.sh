#!/bin/bash

# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Babry 智能截图
# @raycast.mode silent

# Optional parameters:
# @raycast.icon 📸
# @raycast.packageName Babry
# @raycast.description 悬停吸附、点击确认的区域截图

# Raycast 不读取 ~/.zshrc；需要调试记录时取消下一行的注释。
# export BABRY_DEBUG_DIR="$HOME/babry-debug"

# Raycast 的 PATH 很短；补上 Release 安装和 cargo install 的常见位置。
export PATH="/usr/local/bin:/opt/homebrew/bin:$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
if ! command -v babry >/dev/null; then
  echo "找不到 babry，请先安装"
  exit 1
fi

# 放到后台运行，Raycast 不必等待截图结束。
nohup babry >/dev/null 2>&1 &
