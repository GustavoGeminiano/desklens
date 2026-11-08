# Experiment 11 — offline canonical OCR runtime (2026-11-08)

## Implementation

Three agents implemented persistent RapidOCR/PyInstaller packaging, geometric
line reconstruction/corpus, and canonical frontend sessions; the main agent
integrated the Rust process lifecycle, cache, packaging and validation.

The native engine remains the standard build. `ocr-v2` explicitly enables the
experimental runtime. No silent native fallback. This is necessary because
cross-platform numerical and real-capture promotion gates are not established.
The frontend/session improvements apply to both builds.

## Executed Linux evidence at implementation commit

- 249 frontend tests passed.
- 17 worker/layout/corpus/geometry tests passed.
- 5 Rust service tests passed with `ocr-v2`: concurrent requests share inference,
  independent mode cache, retry after failure and invalidation of stale crops.
- Actual model inference reproduced the user's five-line crop literally,
  including `if i % 2 == 0:` and three/six-space indentation. No Python syntax
  correction or lexical insertion.
- Initial experiment failed: line-fragment confidence rejected the recognizer's
  correct whole-line result, and short CTC intervals distorted indentation.
  Generic source-preserving candidate selection and long-span pitch estimation
  fixed this. The user's intentionally malformed source remains malformed.
- Before ink-bound refinement: 20 synthetic development cases, RapidOCR raw
  fragments literal CER 11.1905%, reconstructed CER 0.8730%, 14/20 exact docs.
  This compares stages of RapidOCR, not the native production engine.
- Frozen Linux runtime, empty child PATH: exact user crop in six requests;
  startup 1180.61 ms, first 1691.31 ms, warm median 1463.71 ms. This predates
  the final geometry refinement; final packaging must rerun the same smoke.
- CTC boxes covered full recognition-crop height. Observed-ink tightening inside
  character regions improved this user's text+box IoU>=0.5 recall from 7/17 to
  15/17, retaining all text. Original model polygons remain evidence. Synthetic
  code_01 rose to 17/17. Other families still require comparative evaluation.

## Reproduction and remaining gates

See tools/ocr_worker/README.md. CI `OCR V2 experimental installers` runs native
Linux and Windows, five warmed repetitions on fixed 20/10 development/held-out
synthetic splits, and installed backend smoke with empty PATH and network blocked.
Reports and experimental installers are artifacts, not a production release.

At this commit, Windows execution, final installed application smoke, full new
comparative results and held-out results are pending. Existing native-suite
failures are documented in experiment 10; they are not reclassified as passes.
The corpus contains synthetic clean renders and an overlay-contaminated real
user capture, not 30 real independent screenshots. No production promotion is
claimed. Visual desktop end-to-end validation and clean real held-out captures
remain required. Do not tune layout on the held-out split.
