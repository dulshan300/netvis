#!/usr/bin/env bash
# Cross-compile the Windows release from Linux/WSL and wrap it in the
# installer. Needs the x86_64-pc-windows-gnu target, mingw-w64 and NSIS
# (makensis on PATH, or MAKENSIS=/path/to/makensis).
#   scripts/build-windows.sh            -> target/installer/NetVis-<ver>-setup.exe
#   scripts/build-windows.sh --no-installer
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p netvis-app --target x86_64-pc-windows-gnu
exe=target/x86_64-pc-windows-gnu/release/netvis.exe
echo "built $exe ($(du -h "$exe" | cut -f1))"

[[ "${1:-}" == "--no-installer" ]] && exit 0

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
mkdir -p target/installer
"${MAKENSIS:-makensis}" -V2 -DVERSION="$version" installer/netvis.nsi
echo "built target/installer/NetVis-$version-setup.exe"
