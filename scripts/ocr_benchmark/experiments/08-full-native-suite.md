# Experiment 8: full suites without fail-fast

[Completed native run](https://github.com/GustavoGeminiano/desklens/actions/runs/42424347527).
Both benchmarks and standalone diagnostic suites passed again. Frontend tests
passed on both platforms; benchmark Python tests passed with the documented
Linux-only process-state test skipped on Windows.

Full Windows application suite: **236 passed, 5 failed, 1 ignored**. Failures:

- `test_empirical_inspect_asimov_crop_row_ink`
- `test_t4_1_tc_real_1_asimov_loop_crop_verbatim`
- `test_t4_2_tc_gutter_2_asimov_gutter_crop`
- `test_benchmark_crop_ocr`
- `test_code_ocr_loop_conditional_constructs`

These are existing OCR assertions. All production recognition paths are
unchanged. Independent baseline re-execution proved the six Linux failures in
experiment 5; an independent Windows baseline suite is not claimed here.

The Linux CI full app build failed in existing `libspa` 0.10.1 because Ubuntu 22
provides incompatible older PipeWire headers (missing fields/constants and
function signature mismatches). This does not affect the independent benchmark,
which completed successfully. The full suite executed locally on Linux with
**235 passed, 6 failed, 1 ignored**; those six failures were independently
reproduced from `c6c5737`.

Move this benchmark workflow's Linux runner to Ubuntu 24, leaving the application
dependencies and release workflow unchanged. The newer distribution supplies
newer PipeWire development headers. A follow-up run validates that environment
adjustment; historical Ubuntu 22 results remain identified as such.
