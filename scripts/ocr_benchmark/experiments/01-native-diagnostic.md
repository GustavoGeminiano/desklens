# Experiment 1: recognition versus post-processing

Executed on Linux, 2026-11-07, with Tesseract 5.5.0 (`por+eng`, PSM 11).
Input: existing `tests/fixtures/real_editor_asimov_crop.png`, including its prior
selection overlay. This is a diagnostic reproduction, not clean held-out data.

The isolated diagnostic produced:

| Variant | Words |
| --- | ---: |
| Original image, raw Tesseract | 32 |
| Current preprocessing, raw Tesseract | 42 |
| Complete DeskLens pipeline | 35 |

The preprocessed raw output contains `if 1% 2 == 0:`; the complete pipeline
contains `if 1% 2 ==`. Recognition is imperfect, and post-processing additionally
loses `0:`. Word counts alone are not an accuracy measure. No conclusion about
RapidOCR or Windows follows from this experiment.

Reproduce:

```sh
cargo build --locked --manifest-path tools/ocr-diagnostic/Cargo.toml
tools/ocr-diagnostic/target/debug/ocr_diagnostic --input tests/fixtures/real_editor_asimov_crop.png --output native-diagnostic.json
cargo test --locked --manifest-path tools/ocr-diagnostic/Cargo.toml
```

Validation: isolated harness 20/20 tests passed; existing frontend suite 239/239
passed; application library suite 22/22 passed. The initial attempt at the full
Rust suite failed building the static library due to disk space, before tests
executed. A smaller-profile full-suite rerun is tracked separately.

Changes add diagnostic helpers only; the application's engine and its existing
recognition/reconstruction paths remain unchanged.
