# Experiment 5: complete Linux comparison

Executed 2026-11-07 on ten annotated cases, with five repeated measurements per
case and engine variant. Exact environment, transcripts, per-case metrics and
timing samples are committed in `05-linux-results.json`. Full raw TSV/JSON and
self-contained HTML are generated locally under `benchmark-results-linux-controlled`.

| Variant | Character error incl. whitespace | Error ignoring whitespace | Joint text + box recall (IoU >= .5) |
| --- | ---: | ---: | ---: |
| Tesseract original raw | 20.13% | 3.13% | 72.65% |
| Tesseract preprocessed raw | 19.49% | 2.92% | 77.78% |
| DeskLens current pipeline | 7.18% | 5.85% | 70.51% |
| RapidOCR original raw | 24.82% | 0.94% | 85.04% |

Errors are weighted by reference character counts; joint recall is weighted by
reference word counts. Whitespace-free error removes all whitespace from both
transcripts before edit distance. It does not prove correct line layout.

RapidOCR reads the glyph sequence better in this dataset and improves aggregate
word localization. However, its detector fragments many visual lines into short
segments, so literal text layout is substantially worse. On the user's attached
capture, whitespace-free error is 3.23% versus DeskLens 4.30%, while joint recall
is equal at 68.97%. Thus the aggregate localization gain is not a demonstrated
gain on that specific screenshot.

Median across the ten per-case warmed medians: DeskLens 1369 ms, RapidOCR 1218 ms.
These are different execution configurations: fresh native processes with one
OpenMP thread versus a persistent RapidOCR engine with four CPU threads.
RapidOCR initialization was 3756 ms; first inference includes additional lazy
initialization. Do not interpret these figures as equal-thread engine throughput.

## Decision

Do not replace the production engine yet. The agreed literal-CER-plus-box-recall
promotion criterion is not met. A useful next experiment is geometric line/order
and indentation reconstruction from RapidOCR's measured boxes, preserving its
literal recognized characters and avoiding example-specific spelling/symbol
repairs. It must be evaluated on fresh clean screenshots before integration.

The current reconstruction also deserves scrutiny: preprocessed Tesseract raw
has lower whitespace-free error and higher joint box recall than the final app,
although the app restores substantially more of the intended whitespace layout.

## Validation

- Frontend: 239 passed using portable Node discovery.
- Benchmark Python: 18 passed.
- Minimal native diagnostic: 20 passed.
- Full application Rust suite with `--no-fail-fast`: 235 passed, 6 failed,
  1 ignored. All six failures reproduced from untouched `c6c5737` sources in a
  temporary baseline harness; no unrelated fixes are included here.
- Native Windows workflow has successfully completed its benchmark step and
  proceeded to the full application Rust suite. Quantitative Windows results
  will be recorded after its artifact is available.

The six failures are `test_empirical_inspect_asimov_crop_row_ink`,
`test_t4_1_tc_real_1_asimov_loop_crop_verbatim`,
`test_t4_6_tc_box_7_bounding_box_fidelity_and_non_overlap`,
`test_code_ocr_loop_conditional_constructs`,
`test_code_ocr_native_pipeline_end_to_end`, and
`test_reference_newspaper_image_ocr`.

Most real-looking fixtures repeat the same content and some contain previous
selection overlays. Manual word ink bounds are approximate and tokenization
differs among engines. These limits apply to every aggregate above.
