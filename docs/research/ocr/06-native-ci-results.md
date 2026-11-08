# Experiment 6: executed native Linux and Windows comparison

[Completed workflow and downloadable artifacts](https://github.com/GustavoGeminiano/desklens/actions/runs/42423898417).
Both native benchmark steps succeeded: ten cases, five repeated measurements
per variant, actual Windows.Media.Ocr execution on Windows Server 2025 and
Tesseract on Ubuntu 22.04. This is not cross-compilation-only validation.

Exact environments, transcripts, quality metrics and timing samples are in
`06-native-ci-results.json`. Reports are in the workflow artifacts
`ocr-benchmark-Linux` and `ocr-benchmark-Windows`.

| Platform / pipeline | Error incl. whitespace | Error ignoring whitespace | Joint text + box recall |
| --- | ---: | ---: | ---: |
| Linux / current DeskLens | 7.54% | 5.53% | 68.80% |
| Linux / RapidOCR | 24.82% | 0.94% | 85.04% |
| Windows / current DeskLens | 15.36% | 11.38% | 64.53% |
| Windows / RapidOCR | 24.68% | 1.04% | 85.47% |

The same tradeoff holds on both platforms: RapidOCR better preserves recognized
characters and aggregate word locations, but fragments lines and loses intended
layout. Therefore it does not satisfy the agreed literal-CER-plus-box-recall
replacement criterion. Keep the existing production engines. A follow-up should
test geometric line grouping/order and indentation reconstruction from measured
RapidOCR boxes, without content-specific spelling/operator repairs.

On the user's exact screenshot, Windows current-pipeline joint recall is 72.41%
versus RapidOCR 68.97%, despite whitespace-free CER improving from 13.98% to
3.23%. This explicitly limits the aggregate improvement claim. The dataset has
repeated source content and overlay contamination; acquire clean held-out
screenshots before any global engine switch.

Median of the ten case warm medians: Linux DeskLens 533 ms / RapidOCR 248 ms;
Windows DeskLens 761 ms / RapidOCR 351 ms. Platforms have different hosts and
native engines; these figures must not be compared as OS throughput. Native
process startup is included, RapidOCR is persistent, and CPU thread limits differ.

## Checks and workflow status

- Portable frontend suite: 239 passed on both operating systems.
- Python benchmark tests: Linux 18 passed; Windows 17 passed, one Linux-specific
  process-state assertion skipped. Windows timeout propagation still executes.
- Native diagnostic harness: 20 passed on both operating systems.
- Both native benchmark steps completed, and both reports were uploaded.
- Windows full application Rust tests stopped at the first failing integration
  binary: `test_empirical_inspect_asimov_crop_row_ink`, the same guideline-mask
  assertion already reproduced from original sources. Linux CI could not compile
  the app's `libspa` dependency against Ubuntu 22's PipeWire headers; no full-app
  test execution is claimed for that CI job. Both benchmarks and the standalone
  harness passed. Consequently both jobs correctly remain red; benchmark success
  is not presented as a green full suite.
- A subsequent workflow uses `--no-fail-fast` to expose all existing suite
  failures; the local equivalent already completed with 235 passed / 6 failed /
  1 ignored, with all six failures independently reproduced from the baseline.

No OCR engine selection or desktop frontend interface was changed.
