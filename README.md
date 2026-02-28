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

```bash
npm run tauri build
```

This produces a self-contained, optimised application bundle in
`src-tauri/target/release/bundle/`:

| Platform | Output |
|----------|--------|
| macOS | `macos/Diffrant.app` and a `.dmg` installer |
| Linux | `.deb`, `.rpm`, and an AppImage |
| Windows | `.msi` and NSIS `.exe` installer |

The built app embeds the compiled frontend and Rust binary. The only
runtime dependency that is **not** bundled is the HDF5 C library and any
filter plugins — these must be present on the target machine (e.g. via
conda-forge).

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
