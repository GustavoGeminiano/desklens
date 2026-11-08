# DeskLens

Capture a region of your screen, copy its text, translate it in place, or search
for the image with Google Lens. DeskLens runs from the system tray on Windows
and Linux.

## Features

- Screen capture with an adjustable selection and `Ctrl+Shift+X` shortcut.
- OCR with selectable words, lines, and full-text copying.
- On-screen translation with language selection and paragraph overlays.
- Visual search in your default browser.

## Install and use

Check [GitHub Releases](https://github.com/GustavoGeminiano/desklens/releases) for
available Windows and Linux installers. Experimental builds and their reports
are published as artifacts in [GitHub Actions](https://github.com/GustavoGeminiano/desklens/actions).

Launch DeskLens, press `Ctrl+Shift+X`, and drag to select a region. Use **Copy
Text**, **Translate**, or **Search Image**. Press `Esc` to close the overlay.
The tray menu also provides capture and exit controls. Global shortcuts may
require desktop configuration on Wayland; see [development](docs/development.md).

Standard builds use Windows OCR on Windows and Tesseract on Linux. Linux users
need Tesseract and the desired language packs, for example on Ubuntu/Debian:

```sh
sudo apt install tesseract-ocr tesseract-ocr-eng tesseract-ocr-por wl-clipboard xclip
```

The experimental `ocr-v2` build bundles an offline RapidOCR runtime and models.
It does not require a separate Python or Tesseract installation. Its accuracy
and packaging evidence are documented in the [OCR worker guide](tools/ocr_worker/README.md)
and [research records](docs/research/ocr/README.md); it remains opt-in because
some evaluation categories still regress.

## Data and network use

OCR runs locally. Translation sends recognized text to the configured translation
provider. **Search Image** uploads the selected image to an image-hosting service
and opens Google Lens; it requires an internet connection. Avoid selecting content
you do not intend to send when using these online features.

## Develop

Use Node.js 20 and the current stable Rust toolchain, plus your platform's Tauri
build dependencies. From the repository root:

```sh
npm ci
npm run tauri dev
```

For setup, packaging and contributions, see [development](docs/development.md).
For verification commands and known failures, see [testing](docs/testing.md).

## Repository layout

| Path | Purpose |
| --- | --- |
| `src/` | HTML, CSS and JavaScript for the capture overlay and search panel |
| `src-tauri/` | Rust app, platform integrations, capabilities and installer configuration |
| `tests/` | Frontend regressions and shared image fixtures |
| `scripts/` | Fixture generation and OCR benchmark runner |
| `tools/` | Offline OCR worker and standalone native diagnostic |
| `docs/` | Architecture, development, testing and historical research |
| `.github/workflows/` | Native validation and packaging workflows |

Build products, downloaded models, virtual environments and local benchmark
reports are ignored by Git. They are not part of a clean clone.

## License

[MIT](LICENSE). Developed by Gustavo Geminiano.
DeskLens is an independent project and is not affiliated with Google.
