#!/bin/bash
# 编译 Apple 芯片和 Intel 两种架构，合成通用二进制并打包，Release 工作流和本地共用。
# 用法：scripts/package-macos.sh [输出目录]，默认输出到 dist。
set -euo pipefail

out="${1:-dist}"
name="babry-macos-universal"
target_dir="${CARGO_TARGET_DIR:-target}"

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --locked --release --target "$target"
done

stage="$out/babry"
rm -rf "$stage" "$out/$name.tar.gz" "$out/$name.tar.gz.sha256"
mkdir -p "$stage"
lipo -create -output "$stage/babry" \
  "$target_dir/aarch64-apple-darwin/release/babry" \
  "$target_dir/x86_64-apple-darwin/release/babry"
# 合并后重新做 ad-hoc 签名：Apple 芯片不运行没有签名的程序。
codesign --force --sign - "$stage/babry"
lipo -info "$stage/babry"
"$stage/babry" --help >/dev/null

cp LICENSE NOTICE README.md "$stage/"
cp -R scripts/raycast "$stage/raycast"
# 不把 macOS 扩展属性打成 ._ 文件。
COPYFILE_DISABLE=1 tar -czf "$out/$name.tar.gz" -C "$out" babry
(cd "$out" && shasum -a 256 "$name.tar.gz" >"$name.tar.gz.sha256")
echo "$out/$name.tar.gz"
