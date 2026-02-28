#!/usr/bin/env bash
# Prepare bundle-libs/ for a self-contained macOS release build.
#
# What it does:
#   1. Copies libhdf5 and liblz4 dylibs out of ENV/ and fixes their install
#      names to @rpath/… so Tauri can wire them into Contents/Frameworks/.
#   2. Copies the HDF5 filter plugins (bshuf, lz4, bz2) into bundle-libs/plugins/.
#   3. Symlinks ENV/include into bundle-libs/include so that cargo build can
#      find the HDF5 headers when HDF5_DIR is set to bundle-libs/.
#
# Usage:
#   ./scripts/prepare-bundle-libs.sh [path-to-env]   (default: ./ENV)
#
# After running, build with:
#   HDF5_DIR=$(pwd)/bundle-libs npm run tauri build

set -euo pipefail
cd "$(dirname "$0")/.."

ENV_DIR="${1:-./ENV}"
ENV_DIR="$(realpath "$ENV_DIR")"
OUT="./bundle-libs"
LIB_OUT="$OUT/lib"
PL_OUT="$OUT/plugins"

echo "==> Preparing bundle libs from $ENV_DIR"
mkdir -p "$LIB_OUT" "$PL_OUT"

# ── Helper: resolve symlink to the real versioned file ───────────────────────
real_lib() { realpath "$1"; }

# ── libhdf5 ──────────────────────────────────────────────────────────────────
HDF5_REAL=$(real_lib "$ENV_DIR/lib/libhdf5.dylib")
HDF5_NAME=$(basename "$HDF5_REAL")            # e.g. libhdf5.310.dylib

echo "  libhdf5: $HDF5_NAME"
cp "$HDF5_REAL" "$LIB_OUT/$HDF5_NAME"
install_name_tool -id "@rpath/$HDF5_NAME" "$LIB_OUT/$HDF5_NAME"
ln -sf "$HDF5_NAME" "$LIB_OUT/libhdf5.dylib"

# ── liblz4 ───────────────────────────────────────────────────────────────────
LZ4_REAL=$(real_lib "$ENV_DIR/lib/liblz4.dylib")
LZ4_NAME=$(basename "$LZ4_REAL")              # e.g. liblz4.1.10.0.dylib

echo "  liblz4: $LZ4_NAME"
cp "$LZ4_REAL" "$LIB_OUT/$LZ4_NAME"
install_name_tool -id "@rpath/$LZ4_NAME" "$LIB_OUT/$LZ4_NAME"
ln -sf "$LZ4_NAME" "$LIB_OUT/liblz4.dylib"

# Create the liblz4.1.dylib compatibility symlink that plugins reference
LZ4_COMPAT="liblz4.1.dylib"
ln -sf "$LZ4_NAME" "$LIB_OUT/$LZ4_COMPAT"

# ── HDF5 include headers (for cargo build) ───────────────────────────────────
ln -sfn "$ENV_DIR/include" "$OUT/include"

# ── Filter plugins ───────────────────────────────────────────────────────────
echo "  plugins:"
for plugin in "$ENV_DIR/lib/hdf5/plugin/"*.so; do
    name=$(basename "$plugin")
    echo "    $name"
    cp "$plugin" "$PL_OUT/$name"
done

echo ""
echo "==> bundle-libs/ ready. To build a release app:"
echo "    HDF5_DIR=$(pwd)/bundle-libs npm run tauri build"
