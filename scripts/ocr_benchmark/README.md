# OCR benchmark: Linux and Windows

This experiment does not select or replace the desktop application's OCR engine.
Linux uses Tesseract; Windows uses Windows.Media.Ocr. RapidOCR runs separately
with ONNX Runtime CPU, a pinned PP-OCRv5 detector and Latin recognizer.

## Run locally

Use Python 3.12 and Rust stable. On Linux install Tesseract with `eng` and `por`
language data. On Windows enable English OCR language capability; Windows OCR
availability depends on installed language capabilities. Missing engines or
required fixtures are errors, not successful skipped tests.

```sh
python -m venv .venv-ocr
# Activate: Linux: source .venv-ocr/bin/activate
# PowerShell: .venv-ocr\Scripts\Activate.ps1
python -m pip install -r scripts/ocr_benchmark/requirements.txt
python scripts/ocr_benchmark/rapidocr_engine.py prepare --model-dir .ocr-models
cargo build --locked --manifest-path tools/ocr-diagnostic/Cargo.toml
cargo test --locked --manifest-path tools/ocr-diagnostic/Cargo.toml
python -m unittest discover -s scripts/ocr_benchmark -p "test_*.py"
python scripts/ocr_benchmark/run.py --output benchmark-results --repeats 5 --diagnostic tools/ocr-diagnostic/target/debug/ocr_diagnostic
```

On Windows append `.exe` to the diagnostic executable path. `prepare` is the
explicit network-enabled step. Subsequent inference verifies local model hashes;
it must not fetch models. The isolated Rust manifest compiles the same OCR source
as the application without pulling in Tauri, GTK or an application window.

Open `benchmark-results/report.html`; `results.json` and per-invocation native
JSON preserve measurements and raw Tesseract TSV. Generated artifacts, models
and the virtual environment are ignored by Git. Commit concise experiment
summaries under `experiments/`, not dependencies or temporary files.

## What is compared

The native diagnostic exports original-image raw recognition, preprocessed raw
recognition and the complete current DeskLens pipeline. RapidOCR receives the
original RGB image. Raw native output bypasses spelling repairs, token splitting,
confidence thresholds and invented code symbols. Application behavior and public
frontend contracts are unchanged.

Coordinates are relative to each benchmark input image (after a documented crop,
if present). Tesseract TSV retains its native coordinate space; the diagnostic
JSON maps boxes back to input-image pixels. Windows OCR exposes no confidence:
diagnostic values are null. Confidences from different engines are not calibrated
and must not be ranked against one another.

The runner separates first-call timing, RapidOCR initialization, and five repeated
measurements. Native timings include executable/engine startup; these are repeated
invocations, not a persistent warm native OCR service. The current pipeline's
preprocessing is included in total time. Model download time is excluded.

## Interpreting quality

Ground truth is literal, including visibly malformed code. Existing screenshot
fixtures may contain earlier blue selection overlays; provenance is recorded and
they do not establish accuracy on clean independent screenshots. Synthetic PT/EN
cases provide controlled accents, numbers and symbols in dark/light themes.

Report CER (including whitespace and separately excluding it), exact ordered
lines, and one-to-one box matching at IoU >= 0.5. Joint text-and-box metrics
require exact text. Whitespace-word annotation differs from some engines' token
segmentation; box scores are sensitive to that difference. Missing boxes remain
missing: this benchmark never estimates boxes by uniform character widths.

An integration recommendation requires lower character error and higher joint
text/box recall against the current app, with per-image regressions and latency
reported independently on both platforms. This small dataset is exploratory;
add fresh clean screenshots before changing the global default.

## CI

The `OCR benchmark` GitHub Actions workflow runs native Linux and Windows jobs,
existing frontend/Rust suites and this benchmark. Each job uploads its visual
report and machine-readable artifacts even if the existing Rust suite fails.
A failed native test remains a failed job. A Linux run or a Windows cross-build
is never reported as executed Windows OCR validation.

Upstream: [RapidOCR](https://github.com/RapidAI/RapidOCR),
[usage](https://github.com/RapidAI/RapidOCRDocs/blob/main/docs/install_usage/rapidocr/usage.en.md).
