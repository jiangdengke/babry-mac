#!/bin/bash
# 从 docs/images 里的 SVG 生成 App 图标和菜单栏图标。只在 logo 变化时运行，结果提交到仓库。
set -euo pipefail

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Quick Look 把 SVG 渲染在不透明的白底上，所以按 logo 外框的圆角矩形抠出透明背景。
# 外框在 160 的画布上是 (8,8) 到 (152,152)、圆角 38，换算到 1024 后内缩一点，避开边缘的白色过渡。
qlmanage -t -s 1024 -o "$work" docs/images/babry-mark.svg >/dev/null
source="$work/babry-mark-transparent.png"
magick "$work/babry-mark.svg.png" \
  \( -size 1024x1024 xc:black -fill white -draw "roundrectangle 53,53 971,971 243,243" \) \
  -alpha off -compose CopyOpacity -composite "$source"
iconset="$work/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$source" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$source" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o packaging/AppIcon.icns
echo packaging/AppIcon.icns
# 菜单栏用单色模板图，单独一个 SVG。显示 18 点，Retina 屏用两倍像素。
# Quick Look 输出的是白底黑图，模板图只看透明度，所以把亮度转成透明度。需要 ImageMagick。
qlmanage -t -s 1024 -o "$work" docs/images/babry-menubar.svg >/dev/null
magick "$work/babry-menubar.svg.png" -colorspace Gray -negate -alpha copy \
  -fill black -colorize 100 -resize 36x36 packaging/MenuBarIcon.png
echo packaging/MenuBarIcon.png
