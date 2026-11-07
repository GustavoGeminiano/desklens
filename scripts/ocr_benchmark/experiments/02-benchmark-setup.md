# Experiment 2: reproducible cross-platform comparison

Prepared on Linux, 2026-11-07. RapidOCR 3.10.0 / ONNX Runtime 1.31.1 CPU;
PP-OCRv5 mobile detector and Latin recognizer are selected explicitly, verified
against pinned SHA-256 digests, and loaded locally after explicit setup.

Offline smoke recognition of `scripts/code_loop_dark.png` succeeded with five
lines and 17 native word boxes. The first inference took approximately 7.95 s,
including lazy initialization; this is not a warmed latency claim or proof of
accuracy. Full comparative results are recorded separately after completion.

The dataset contains ten cases: six annotated existing/attached images and four
controlled synthetic PT/EN dark/light cases. Several screenshots repeat the
same code and include preexisting blue overlays. They are diagnostic evidence,
not an independent generalization benchmark.

Validation: 14 Python tests passed, covering edit distance, Unicode, one-to-one
box matching, missing symbols, malformed/missing fixtures, HTML escaping,
portable fixture generation and model integrity checks. The native harness
passed its 20 tests. Commands and measurement caveats are in the benchmark README.

Added native Linux/Windows GitHub Actions execution with artifacts. A workflow
definition is not an executed Windows test: remote results must be checked and
documented separately. No application engine or frontend contract was changed.
