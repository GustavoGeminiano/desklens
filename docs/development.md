# Development and contributions

Run commands from the repository root unless stated otherwise.

## Requirements

- Node.js 20 and npm; install locked dependencies with `npm ci`.
- Current stable Rust; the root `Cargo.lock` owns the Tauri workspace resolution.
- Windows: Microsoft C++ build tools and WebView2 for Tauri.
- Ubuntu/Debian: GTK/WebKit and capture dependencies below. CI builds on Ubuntu 24.04.

```sh
sudo apt update
sudo apt install -y build-essential curl wget file pkg-config libssl-dev \
  libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libpipewire-0.3-dev libspa-0.2-dev libgbm-dev \
  tesseract-ocr tesseract-ocr-eng tesseract-ocr-por wl-clipboard xclip
npm ci
npm run tauri dev
```

On Linux, `./run-linux.sh --dev` also configures the compatibility environment
and GNOME shortcut. The launcher can change desktop shortcuts; direct Tauri dev
is useful when you only want to run the app. Wayland may restrict global hotkeys;
use the tray capture action or configure a desktop shortcut to
`/path/to/desklens/run-linux.sh --trigger`.

## Build

```sh
npm run tauri build
```

Workspace binaries live in `target/release/`, and installers in
`target/release/bundle/`. `run-linux.sh` prefers an existing release binary;
rebuild and restart it after changes, or use `--dev` for development.

The `ocr-v2` feature is experimental. Follow the
[worker setup and packaging guide](../tools/ocr_worker/README.md) to prepare a
Python 3.12 build environment and verified models, then run:

```sh
npm run ocr:build -- --bundles deb   # Linux
npm run ocr:build -- --bundles nsis  # Windows
```

Python is a developer/build requirement for this worker, not a requirement on
the end user's computer. Package on the target OS and retain the complete
`ocr-runtime` resource directory. Do not commit downloaded models or the frozen
runtime. The native diagnostic under `tools/ocr-diagnostic/` is intentionally an
independent Cargo workspace with its own lockfile.

## Changes

Keep changes focused, explain the user-visible behavior, and run the relevant
checks in [testing](testing.md). Preserve literal OCR evidence when adding a
regression; do not turn an incorrect transcription into passing ground truth.
Use existing UI patterns and keep platform behavior explicit. Never include
credentials, local environment files, generated installers or caches in commits.

## Generated files

`target/`, `build/`, `node_modules/`, `.venv-ocr/`, `.ocr-models/`, and
`benchmark-results*/` are local generated directories. The many benchmark folders
in an existing checkout are not tracked repository contents. Benchmark commands
can place new reports under `build/reports/` to keep the checkout root uncluttered.
Delete local outputs only when you no longer need their results; no cache or
report deletion is required to contribute.
