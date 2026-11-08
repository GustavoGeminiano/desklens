# OCR research records

These dated records preserve experiments and negative results; they are not
current setup instructions. Start with [testing](../../testing.md) for commands.

- [Initial native validation](10-final-validation.md): benchmark stages and existing failures.
- [Canonical offline runtime](11-offline-canonical-runtime.md): experimental architecture and limits.
- [Native installers and scale recovery](12-native-installers-and-scale-recovery.md): Linux/Windows results and regressions.
- [Validation race](13-validation-race.md): test cleanup and stale-session corrections.

Earlier numbered notes explain intermediate decisions. JSON summaries are kept
for reproducibility; large generated HTML reports and installers belong in CI
artifacts or ignored local output directories.

Paths recorded inside historical experiment prose and JSON identify the checkout
at the time of that run. For example, `scripts/code_loop_dark.png` now lives at
`tests/fixtures/synthetic/code_loop_dark.png`. Historical observations are retained
rather than rewritten as new measurements.
