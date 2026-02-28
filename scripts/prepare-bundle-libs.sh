#!/usr/bin/env bash
# Collect all non-system dylib dependencies of libhdf5 and liblz4 (transitively)
# into bundle-libs/lib/, ready for Tauri to bundle into Contents/Frameworks/.
#
# How it works
# ------------
# Conda-forge builds already set each dylib's install name to
# @rpath/SHORT_NAME and add @loader_path/ as LC_RPATH. This means:
#
#   - When the main binary runs, its @executable_path/../Frameworks rpath
#     makes all deps findable in Contents/Frameworks/.
#   - When a bundled dylib (e.g. libhdf5) loads one of its own deps
#     (e.g. @rpath/libcrypto.3.dylib), @loader_path/ also resolves to
#     Frameworks/ — so siblings find each other without any path surgery.
#
# We therefore just need to copy each dylib under its SHORT_NAME (the part
# after @rpath/ in its install name). No install_name_tool edits required.
#
# Usage:
#   ./scripts/prepare-bundle-libs.sh [path-to-env]   (default: ./ENV)
#
# After running this, build the release bundle with:
#   HDF5_DIR=$(pwd)/ENV npm run tauri build

set -euo pipefail
cd "$(dirname "$0")/.."

ENV_DIR="${1:-./ENV}"
ENV_DIR="$(realpath "$ENV_DIR")"
OUT="./bundle-libs"
LIB_OUT="$OUT/lib"
PL_OUT="$OUT/plugins"

echo "==> Collecting dylib deps from $ENV_DIR"
mkdir -p "$LIB_OUT" "$PL_OUT"

# ── Recursive dep collection (Python for reliability) ────────────────────────
python3 - "$ENV_DIR" "$LIB_OUT" << 'PYEOF'
import sys, subprocess, os, shutil

env_dir, lib_out = sys.argv[1], sys.argv[2]
env_lib = os.path.join(env_dir, 'lib')

def otool_L(path):
    try:
        out = subprocess.check_output(['otool', '-L', path], stderr=subprocess.DEVNULL).decode()
        return [line.strip().split()[0] for line in out.strip().split('\n')[1:] if line.strip()]
    except Exception:
        return []

def otool_D(path):
    try:
        out = subprocess.check_output(['otool', '-D', path], stderr=subprocess.DEVNULL).decode()
        lines = [l.strip() for l in out.strip().split('\n') if l.strip()]
        return lines[1] if len(lines) > 1 else None
    except Exception:
        return None

def is_system(dep):
    return (dep.startswith('/usr/lib') or dep.startswith('/System/') or
            '/CoreFoundation' in dep or '/SystemConfiguration' in dep)

def resolve(dep, env_lib_dir):
    """Resolve a dep path (absolute or @rpath/@loader_path) to a real file."""
    if dep.startswith('/'):
        r = os.path.realpath(dep)
        return r if os.path.exists(r) else None
    name = dep.split('/')[-1]
    p = os.path.join(env_lib_dir, name)
    if os.path.exists(p):
        return os.path.realpath(p)
    return None

plugin_dir = os.path.join(env_lib, 'hdf5', 'plugin')
seeds = [
    os.path.realpath(os.path.join(env_lib, 'libhdf5.dylib')),
    os.path.realpath(os.path.join(env_lib, 'liblz4.dylib')),
] + [
    os.path.realpath(os.path.join(plugin_dir, f))
    for f in os.listdir(plugin_dir)
    if f.endswith('.so')
]

seen = set()
queue = list(seeds)
to_bundle = []  # list of (real_path, dest_name)

while queue:
    lib = queue.pop(0)
    if lib in seen:
        continue
    seen.add(lib)

    # Only bundle proper dylibs (not the .so plugins — those go to PL_OUT)
    if lib.endswith('.dylib'):
        install_name = otool_D(lib)
        if install_name and install_name.startswith('@rpath/'):
            dest_name = install_name[len('@rpath/'):]
        else:
            dest_name = os.path.basename(lib)
        to_bundle.append((lib, dest_name))

    for dep in otool_L(lib):
        if is_system(dep):
            continue
        resolved = resolve(dep, env_lib)
        if resolved and resolved not in seen:
            queue.append(resolved)

print(f"  Bundling {len(to_bundle)} dylibs:")
for real, dest in sorted(to_bundle, key=lambda x: x[1]):
    dest_path = os.path.join(lib_out, dest)
    print(f"    {dest}")
    shutil.copy2(real, dest_path)

PYEOF

# ── HDF5 include headers (for cargo build with HDF5_DIR=bundle-libs) ─────────
ln -sfn "$ENV_DIR/include" "$OUT/include"

# ── Filter plugins ────────────────────────────────────────────────────────────
echo "  plugins:"
for plugin in "$ENV_DIR/lib/hdf5/plugin/"*.so; do
    name=$(basename "$plugin")
    echo "    $name"
    cp "$plugin" "$PL_OUT/$name"
done

echo ""
echo "==> bundle-libs/ ready."
echo ""
echo "Build the release app with:"
echo "    HDF5_DIR=$(pwd)/ENV npm run tauri build"
