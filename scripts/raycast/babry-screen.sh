#!/bin/bash

# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Babry 全屏截图
# @raycast.mode silent

# Optional parameters:
# @raycast.icon 🖥️
# @raycast.packageName Babry
# @raycast.description 截取整个屏幕并直接进入标注

# Raycast 不读取 ~/.zshrc；需要调试记录时取消下一行的注释。
# export BABRY_DEBUG_DIR="$HOME/babry-debug"

# 放到后台运行，Raycast 不必等待截图结束。
nohup "$HOME/.cargo/bin/babry" --screen >/dev/null 2>&1 &
