# Experiment 9: Ubuntu 24 full application link

Run [42425151567](https://github.com/GustavoGeminiano/desklens/actions/runs/42425151567)
confirmed Ubuntu 24 resolves the PipeWire/libspa header errors. Native benchmark,
frontend, Python and standalone diagnostic checks passed again on Linux.

The full application Rust build then reached linking and failed with:

```text
rust-lld: error: unable to find library -lgbm
```

Added `libgbm-dev` to this workflow's Linux development packages, required by the
existing capture dependencies. No application source/dependency version changed.
Do not count the failed link as executed full application tests. The local full
Linux suite and the native Windows full suite remain documented separately.

GitHub API verification also confirmed both author and committer of the published
experiment commits resolve to the account `GustavoGeminiano`, with configured Git
identity Gustavo <gustavongeminiano@gmail.com>.
