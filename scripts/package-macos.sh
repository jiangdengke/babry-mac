#!/bin/bash
# 编译 Apple 芯片和 Intel 两种架构，合成通用二进制，打成 Babry.app 并制作 DMG。
# Release 工作流和本地共用。用法：scripts/package-macos.sh [输出目录]，默认输出到 dist。
set -euo pipefail

out="${1:-dist}"
target_dir="${CARGO_TARGET_DIR:-target}"
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
dmg="$out/Babry-$version.dmg"

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --locked --release --target "$target"
done

stage="$out/dmg"
app="$stage/Babry.app"
rm -rf "$stage" "$dmg" "$dmg.sha256"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

lipo -create -output "$app/Contents/MacOS/babry" \
  "$target_dir/aarch64-apple-darwin/release/babry" \
  "$target_dir/x86_64-apple-darwin/release/babry"
lipo -info "$app/Contents/MacOS/babry"
"$app/Contents/MacOS/babry" --help >/dev/null

sed "s/__VERSION__/$version/g" packaging/Info.plist >"$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
cp packaging/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
printf 'APPL????' >"$app/Contents/PkgInfo"

# 整个 App 做 ad-hoc 签名：Apple 芯片不运行没有签名的程序，签名也覆盖 Info.plist 和图标。
codesign --force --sign - "$app"
codesign --verify --strict --verbose=2 "$app"

# DMG 里放一个指向“应用程序”的快捷方式，打开后把 Babry 拖进去即可安装。
ln -s /Applications "$stage/Applications"
hdiutil create -volname "Babry $version" -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$dmg" >/dev/null
(cd "$out" && shasum -a 256 "Babry-$version.dmg" >"Babry-$version.dmg.sha256")
rm -rf "$stage"
echo "$dmg"
