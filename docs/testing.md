# Testing

## Frontend

```sh
npm ci
npm test
```

Uses Node's built-in test runner for selection, stale replies, copy, translation
and geometry regressions. The real-browser sizing test uses a local Chrome or
Chromium installation on Linux and reports a skip if none is found. DOM mocks
alone do not establish that text fits in a rendered box.

## Rust

```sh
cargo check --locked -p desklens
cargo test --locked -p desklens --features ocr-v2 ocr_service::tests --lib
cargo test --locked --no-fail-fast -p desklens
```

On Linux, set `OMP_THREAD_LIMIT=1` for reproducible Tesseract test execution.
Native OCR integration tests require the platform engine and fixture files.
Some older tests skip absent images; keep the versioned fixtures present when
interpreting a successful run.

The existing native suite has known OCR failures, documented with platform
results in [experiment 12](research/ocr/12-native-installers-and-scale-recovery.md).
Do not report that suite as green merely because the focused session tests pass.

## OCR tooling

After installing the pinned Python dependencies described in the
[worker guide](../tools/ocr_worker/README.md):

```sh
python -m unittest discover -s scripts/ocr_benchmark -p "test_*.py"
python -m unittest discover -s tools/ocr_worker -p "test_*.py"
cargo test --locked --manifest-path tools/ocr-diagnostic/Cargo.toml
```

The [benchmark guide](../scripts/ocr_benchmark/README.md) covers annotated images,
CER, symbol recovery, box IoU and timings. Run held-out evaluation explicitly and
do not tune against its results. The clean rendered corpus is synthetic; it is
not evidence from 30 independent real-world captures.

## Fixtures

- `tests/fixtures/*.png`: existing screenshot regressions.
- `tests/fixtures/synthetic/*.png`: five generated code examples used by Rust tests.
- `scripts/generate_test_image.ps1`: Windows/System.Drawing fixture generator;
  writes into `tests/fixtures/synthetic/`.
- `scripts/ocr_benchmark/assets/`: benchmark-specific fonts, licenses and reported images.

Moving a fixture requires updating its Rust paths, scripts and documentation.
Retain the original image when changing a regression's expected text.

## CI

`ocr-benchmark.yml` runs the native diagnostic, benchmark and existing tests on
Linux and Windows. `ocr-v2.yml` validates experimental runtime packaging and
installed backend smoke tests with an empty child PATH and blocked network.
`release.yml` handles tagged installer releases. Cross-compilation does not
count as Windows execution, and benchmark execution success does not imply that
accuracy promotion gates passed.
