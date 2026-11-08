# Architecture

DeskLens uses Tauri v2 with a Rust backend and a browser-based frontend. Source
folders retain Tauri's standard conventions so its CLI and packaging configuration
remain straightforward.

## Frontend (`src/`)

`main.js`, `index.html` and `styles.css` implement capture selection, OCR review,
copying, translation overlays and language controls. `results.*` implements the
visual-search panel. Frontend regression tests live in `tests/`; the browser
sizing regression uses Chrome when available and explicitly skips otherwise.

## Backend (`src-tauri/`)

- `lib.rs`: Tauri commands, window lifecycle, capture, clipboard, visual search
  and desktop integrations. `main.rs` is the executable entry point.
- `ocr.rs`: native platform OCR and its existing preprocessing pipeline.
- `ocr_service.rs`: immutable crop sessions, cached canonical results and the
  optional persistent OCR worker process. It rejects obsolete crop responses.
- `translation.rs`: translation providers and paragraph grouping.
- `tauri.conf.json`: standard application configuration.
- `tauri.ocr.conf.json`: additional frozen runtime resources for experimental builds.

## OCR tools

`tools/ocr_worker/` contains model inference, geometric reconstruction, bounded
pixel rereads, protocol, packaging and a fixed synthetic corpus. Its model pins
and verification come from `scripts/ocr_benchmark/rapidocr_engine.py`.
`tools/ocr-diagnostic/` builds the native diagnostic example without the desktop
application's GUI dependencies. `scripts/ocr_benchmark/` compares engine stages
and creates HTML reports.

## Test data and research

Shared screenshots live in `tests/fixtures/`; generated source-code examples
live in `tests/fixtures/synthetic/`. Benchmark-specific model/font assets and
annotations stay beside the benchmark that owns them. Font licenses remain
with those assets.

`docs/research/ocr/` retains dated experiments, negative results and platform
validation evidence. `docs/archive/` contains superseded readiness reports.
Neither directory is runtime input or a claim about the current test status.
