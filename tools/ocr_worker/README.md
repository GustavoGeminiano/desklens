# Offline OCR runtime

The worker keeps one RapidOCR CPU model pair loaded and serves JSON-lines over
stdin/stdout. Recognition uses the shared benchmark adapter's exact PP-OCRv5
Latin recognition/detection models and SHA-256 checks. Whole visual lines are
recognized again using the same loaded recognizer, with CTC character polygons
mapped back to original crop pixels. Layout never repairs spelling or code.

## Development

Use Python 3.12 and an isolated environment. Linux:

```sh
uv venv --python 3.12 .venv-ocr
uv pip install --python .venv-ocr/bin/python -r tools/ocr_worker/requirements-build.txt
.venv-ocr/bin/python scripts/ocr_benchmark/rapidocr_engine.py prepare
.venv-ocr/bin/python tools/ocr_worker/worker.py --self-test scripts/code_loop_dark.png
```

On Windows use `.venv-ocr\Scripts\python.exe` instead. Model downloads happen only
in the explicit `prepare` build/setup step. Normal worker startup verifies local
models and fails if a model is missing or corrupted. ONNX telemetry is disabled.

## Build the installer resource

```sh
.venv-ocr/bin/python tools/ocr_worker/build.py
build/ocr-runtime/desklens-ocr --self-test scripts/code_loop_dark.png
```

Build on each target OS. `build.py --prepare-models` explicitly allows downloading
models before freezing. Default output is `build/ocr-runtime`; optional
`--output` and `--model-dir` paths are supported. The **entire** `ocr-runtime`
directory, including `_internal` and `models`, must ship as one Tauri resource.
Copying just the executable will not work. On Windows the executable ends in
`.exe`. The consumer does not install Python, pip, Tesseract, or model packages.

The PyInstaller `onedir` runtime is deliberately retained: no per-launch
extraction or executable files in a temporary directory. The Linux runtime still
requires the platform glibc; build on the oldest supported Linux distribution.
The current uncompressed Linux staging is approximately 499 MB.

## Protocol version 1

After verified model initialization, stdout emits:

```json
{"type":"ready","protocol_version":1,"engine":"rapidocr","models":{},"offline":true}
```

Each request is one JSON object terminated by a newline:

```json
{"id":"request-1","crop_id":"crop-1","mode":"auto","input_path":"/absolute/crop.png"}
```

Mode is `auto`, `text`, or `code`. Responses echo both IDs and return either
`{"ok":true,"result":{...}}` or `{"ok":false,"error":"..."}`. Canonical results
include full text, lines/words with source offsets in UTF-16, geometry, uncertainty
and recognition evidence, and raw detector character polygons. Logs use stderr.
Malformed requests return an error without terminating the persistent process.

## Experimental app integration and promotion status

The standard build keeps the native OCR engine. The `ocr-v2` Cargo feature opts
into the persistent packaged worker; it never silently falls back to native OCR.
After building the runtime above, run `npm run ocr:build -- --bundles deb` on Linux
or `npm run ocr:build -- --bundles nsis` on Windows. The separate Tauri config
preserves the entire runtime directory, not just the executable. A development
build can use `npm run tauri dev -- --features ocr-v2` with the documented venv.

Both engines use immutable crop sessions and shared in-flight/cached results.
The experimental canonical text and UTF-16 spans drive copy, selection and
highlights. Whole-line recognition recovers actual model glyphs; it does not
correct Python syntax. Conflicting reads expose a read-only preview with Retry
and Copy anyway. OCR modes are Auto, Text and Code.

Run `python -m unittest discover -s tools/ocr_worker -p "test_*.py"` and
`cargo test -p desklens --features ocr-v2 ocr_service::tests --lib`.
`evaluate_layout.py --diagnostic PATH --output OUTPUT` compares the 20 synthetic
development cases with native OCR using five warm repetitions. `--split heldout`
evaluates ten fixed content-disjoint cases; do not tune on them. Reports include
literal CER, glyph CER, box IoU, startup and latency. Synthetic clean renders and
the user's overlay-containing capture do not replace real clean held-out images.
The exact five-line user regression is mandatory, but alone cannot authorize
production promotion. Native engines remain the default until the full gates pass.

`smoke_packaged.py --runtime PATH --output OUTPUT [--app INSTALLED_EXE]` clears
PATH in child processes and tests the actual frozen engine and optionally the
installed application's Rust session/cache path. `--ocr-smoke INPUT OUTPUT`
on the app runs without opening its windows and writes a diagnostic JSON.
The experimental CI extracts the .deb or installs NSIS, blocks network for OCR,
and uploads the installers and reports. This is a backend packaging smoke;
frontend behavior is covered by the JavaScript regression suite, not a claim
of automated visual end-to-end desktop coverage.

Low-confidence full lines have a bounded second look: up to three lines per
image, two pixel transforms per line (cubic enlargement and Otsu binarization).
Only different glyph output with a >0.02 score increase within the same recognizer
can replace the original. Agreeing text retains its original geometry. Every
changed read remains visible as uncertainty and evidence. `diagnose_variants.py`
records successful and unsuccessful pixel alternatives without choosing text.
The packaged smoke also checks the mandatory crop at 125%, 150%, and 200% and
uses an accented filesystem path; stdout/stdin are explicitly UTF-8 on Windows.
