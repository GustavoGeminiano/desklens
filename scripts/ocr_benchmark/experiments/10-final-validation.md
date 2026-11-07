# Final validation: Linux and Windows

[Completed native CI run and visual reports](https://github.com/GustavoGeminiano/desklens/actions/runs/42425684950).
Source commit: `1a37de7`. Exact environment versions, models, aggregate metrics
and check results are in `10-final-validation.json`. Earlier experiment reports
retain their original environment and are historical measurements.

Ubuntu 24 with `libgbm-dev` successfully compiled and executed the complete
application Rust suite. Windows executed the same full suite natively. All ten
annotated benchmark cases ran with five repeated measurements per variant on
both platforms. No production engine was replaced.

| Final checks | Linux | Windows |
| --- | ---: | ---: |
| Frontend | 239 passed | 239 passed |
| Benchmark Python | 18 passed | 17 passed, 1 Linux-only assertion skipped |
| Native diagnostic | 20 passed | 20 passed |
| Native benchmark | Passed | Passed |
| Existing full application Rust suite | 236 passed, 5 failed, 1 ignored | 236 passed, 5 failed, 1 ignored |

The five failing tests differ by platform and are listed in the JSON. They are
existing OCR assertions, with production recognition paths unchanged. Linux
failures are a subset of those independently reproduced using original sources
in experiment 5. An independent Windows baseline re-execution is not claimed.
The full workflows correctly remain red; new diagnostic/benchmark success does
not mean the existing application suite is green. No compile/link blockers
remain in this final workflow run.

## Results and decision

| Pipeline | Literal character error | Error ignoring whitespace | Joint word + box recall |
| --- | ---: | ---: | ---: |
| Linux current DeskLens | 7.18% | 5.85% | 70.51% |
| Linux RapidOCR | 24.82% | 0.94% | 85.04% |
| Windows current DeskLens | 15.36% | 11.38% | 64.53% |
| Windows RapidOCR | 24.68% | 1.04% | 85.47% |

RapidOCR improves aggregate character preservation and localization on this
small dataset, but fragments lines and fails the agreed literal-error promotion
criterion. **Keep the current app engines.** The next useful experiment is
geometry-based line ordering/grouping and indentation reconstruction from
RapidOCR's measured boxes, preserving recognized characters without handcrafted
spelling or operator repairs. This follow-up is not implemented by this benchmark.

The exact user screenshot does not demonstrate a universal box improvement:
Windows joint recall is 72.41% for the app versus 68.97% for RapidOCR. Several
fixtures repeat code and include prior selection overlays; manual bounds are
approximate and word segmentation differs among engines. Obtain clean held-out
captures before making an integration decision.

Artifacts `ocr-benchmark-Linux` and `ocr-benchmark-Windows` contain self-contained
`report.html`, complete results, native JSON/TSV and the exact benchmark inputs.
Models, virtual environments and temporary outputs remain excluded from Git.
All implementation and experiment commits use Gustavo's configured identity;
GitHub confirmed attribution to `GustavoGeminiano`.
