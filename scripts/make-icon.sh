#!/bin/bash
# 从 docs/images/babry-mark.svg 生成 packaging/AppIcon.icns。只在 logo 变化时运行，结果提交到仓库。
set -euo pipefail

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Quick Look 能把 SVG 渲染成带透明通道的 PNG。
qlmanage -t -s 1024 -o "$work" docs/images/babry-mark.svg >/dev/null
source="$work/babry-mark.svg.png"
iconset="$work/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$source" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$source" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o packaging/AppIcon.icns
echo packaging/AppIcon.icns
