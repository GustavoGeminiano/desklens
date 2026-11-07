# Experiment 4: native Windows test discovery

The first Windows Actions job executed dependency installation and the 14 Python
tests, then failed before OCR at `npm test`:

```text
Could not find 'D:\a\desklens\desklens\tests\*.test.mjs'
```

Windows does not expand the shell glob used by the original package script.
Changed the script to Node's built-in `node --test` discovery. It discovers the
same 239 tests locally, all passing. This enables the existing suite on Windows
without a shell-specific wildcard. CI keeps frontend and full Rust failures
visible but allows the native benchmark to execute independently of those suites.

The existing Rust guideline-masking failure was independently reproduced from
the untouched `c6c5737` source and fixture in a temporary minimal harness. The
same assertion fails with `got: 310`; this is confirmed preexisting, not inferred
only from the additive nature of the diagnostic helpers.

Remote Windows OCR execution remains pending until the corrected workflow runs.
