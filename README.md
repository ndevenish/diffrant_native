# diffrant-native

A native desktop application wrapping the [diffrant](../diffrant) diffraction
image viewer. Opens NeXus (`.nxs`) and HDF5 (`.h5`, `.hdf5`) detector files
and displays frames with the same controls available in the web viewer.

## How it works

The Rust backend starts a small HTTP server on a random localhost port. When
you open a file it reads frames and detector geometry out of the HDF5 file and
serves them to the frontend over that local connection. The diffrant React
component renders the image exactly as it would in a browser, using the same
16-bit pipeline.

---

## Prerequisites

### All platforms

| Tool | Version | Install |
|------|---------|---------|
| Node.js | ≥ 18 | https://nodejs.org or `conda install nodejs` |
| Rust | ≥ 1.77 | https://rustup.rs |
| Tauri CLI | v2 | installed via npm (see below) |

### HDF5 C library

The Rust HDF5 reader links against the system HDF5 C library.
The easiest source is **conda-forge**:

```bash
conda install -c conda-forge hdf5
```

If you use conda, activate the environment before running any commands so
the compiler can find `libhdf5`. On macOS you can also use Homebrew:

```bash
brew install hdf5
```

> **Compressed files (bitshuffle + LZ4):** detector files from most modern
> instruments use these HDF5 filter plugins.  Install them the same way:
>
> ```bash
> conda install -c conda-forge hdf5-external-filter-plugins
> ```
>
> The plugins are loaded at runtime, not at build time, so the app will still
> build without them — it will just error when it tries to read a compressed
> file.

### macOS — WebKit / Xcode

Tauri uses the system WebKit on macOS. Xcode Command Line Tools must be
present:

```bash
xcode-select --install
```

### Linux — system libraries

```bash
# Debian / Ubuntu
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

# Fedora
sudo dnf install webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel
```

---

## Running in development

The project ships a `.env` file that sets `HDF5_PLUGIN_PATH` to
`./ENV/lib/hdf5/plugin`.  Create a conda environment at `./ENV` with the
required packages and this will be picked up automatically:

```bash
conda create -p ./ENV -c conda-forge hdf5 hdf5-external-filter-plugins
```

Then:

```bash
# 1. Install JavaScript dependencies (only needed once, or after package changes)
npm install

# 2. Start the dev server (Vite frontend + Tauri backend together)
npm run tauri dev
```

The first run will compile all Rust dependencies, which takes a few minutes.
Subsequent runs are fast. The app opens in a native window; the frontend hot-
reloads when you edit files under `src/`.

Set `RUST_LOG=diffrant_native=debug` to see verbose logging from the backend:

```bash
RUST_LOG=diffrant_native=debug npm run tauri dev
```

---

## Building a release app bundle

The Rust binary dynamically links against `libhdf5`, and the HDF5 filter
plugins (`libh5bshuf`, `libh5lz4`, `libh5bz2`) are loaded at runtime via
`dlopen`. All of these — plus their transitive dependencies (libcrypto,
libssl, libcurl, and so on) — must be bundled inside the `.app`.

conda-forge builds already set each dylib's install name to
`@rpath/SHORT_NAME` and embed `@loader_path/` in their LC_RPATH, so once
everything is in `Contents/Frameworks/` they resolve each other without any
`install_name_tool` surgery.

### Step 1 — prepare the bundle libs (once per ENV update)

```bash
./scripts/prepare-bundle-libs.sh
```

This script:
- Walks the full dependency tree of `libhdf5` and `liblz4` (and the three
  filter plugins) using `otool -L`.
- Copies each non-system dylib into `bundle-libs/lib/` under its install
  name (the short `@rpath/NAME` form) so Tauri bundles it with the right
  filename.
- Copies the filter plugins into `bundle-libs/plugins/`.
- Symlinks `ENV/include/` into `bundle-libs/include/` (used during
  compilation; see Step 2).

The `bundle-libs/` directory is gitignored; regenerate it whenever the conda
environment is updated.

> **Signing note:** For distribution via the Mac App Store or to users on
> Macs with Gatekeeper enabled you will need a Developer ID certificate so
> Tauri can re-sign and notarize the bundle. For internal / lab use,
> ad-hoc or unsigned builds work fine.

### Step 2 — build

```bash
HDF5_DIR=$(pwd)/ENV npm run tauri build
```

`HDF5_DIR` points to the conda environment so the `hdf5-metno-sys` build
script finds the original (unmodified) headers and library to link against.
Tauri's bundler then:

1. Copies all 16 dylibs listed in `bundle.macOS.frameworks` (from
   `bundle-libs/lib/`) into `Contents/Frameworks/`.
2. Rewires the binary's `libhdf5` dependency to `@rpath/libhdf5.310.dylib`
   and adds `@executable_path/../Frameworks` to the binary's LC_RPATH.
3. Copies the filter plugins into `Contents/Resources/hdf5-plugins/`.
4. At runtime, `lib.rs` sets `HDF5_PLUGIN_PATH` to the bundled plugin
   directory before any HDF5 reads occur.

The finished bundle is in `src-tauri/target/release/bundle/`:

| Platform | Output |
|----------|--------|
| macOS | `macos/Diffrant.app` and a `.dmg` installer |
| Linux | `.deb`, `.rpm`, and an AppImage |
| Windows | `.msi` and NSIS `.exe` installer |

### Targeting a single format

```bash
# macOS disk image only
npm run tauri build -- --bundles dmg

# AppImage only
npm run tauri build -- --bundles appimage
```

### Cross-compilation

Tauri does not support cross-compilation out of the box. Build on the
platform you are targeting.

---

## Adding support for new file formats

1. Create `src-tauri/src/readers/myformat.rs` and implement the `Reader`
   trait:

   ```rust
   pub trait Reader: Send + Sync {
       fn metadata(&self) -> anyhow::Result<ImageMetadata>;
       fn frame_count(&self) -> anyhow::Result<usize>;
       fn read_frame(&self, frame: usize) -> anyhow::Result<(Vec<u16>, usize, usize)>;
   }
   ```

2. Add `pub mod myformat;` to `src-tauri/src/readers/mod.rs` and match on
   the file extension in the `open()` function there.

3. Add the extension to the file-picker filter in `src/App.tsx`.

---

## Project layout

```
diffrant_native/
├── src/                    # React / TypeScript frontend
│   ├── App.tsx             # Main app: file open, frame navigation, viewer
│   └── main.tsx
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs          # App setup, embedded HTTP server startup
│   │   ├── commands.rs     # Tauri IPC: open_file, get_server_port
│   │   ├── server.rs       # axum HTTP server (/metadata, /image/:frame)
│   │   └── readers/
│   │       ├── mod.rs      # Reader trait + format dispatcher
│   │       └── nxs.rs      # NXmx NeXus / HDF5 reader
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
└── index.html
```
