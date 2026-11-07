# Experiment 3: bound native OCR measurements

Linux, 2026-11-07. The initial full benchmark was interrupted intentionally:
the first case's raw Tesseract invocation took 59.3 seconds while the full Rust
suite was compiling concurrently. Those partial timings are not used to rank
engines. A controlled rerun starts after compilation, with native OpenMP threads
fixed to one and the setting recorded in every result/report.

The runner now bounds each native invocation to 120 seconds and terminates its
process tree on timeout. Errors remain errors, never empty successful results.
RapidOCR CPU threads remain explicitly fixed at four; this is an application
configuration comparison, not an equal-thread throughput benchmark.

Validation: 18 Python tests passed, including environment propagation, nonzero
exit handling, timeout behavior and actual descendant termination on Linux.
Windows process cleanup is implemented but requires native Windows validation.

The compact-profile full application Rust suite now compiles successfully and
reaches an existing failing test: `test_empirical_inspect_asimov_crop_row_ink`,
in `challenger_m1_1_gen2_empirical_tests`, reports 310 instead of meeting its
guideline-masking assertion. This failure is kept visible; the benchmark workflow
still collects measurements before running the full application suite.
