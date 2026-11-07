# Experiment 7: Windows conditional dependency audit

Audited installed distribution requirements after successful native Windows and
Linux benchmark execution. `colorlog` and `tqdm` require `colorama` on Windows;
Linux's dependency freeze did not include this conditional package.

Added `colorama==0.4.6; sys_platform == "win32"`, exactly matching the version
already installed in the successful Windows job (recorded in its installation
logs). All other runtime dependencies were already pinned; remaining conditional
requirements are optional extras. This does not change the tested Windows package
set or Linux execution.

Validation: `uv pip install --python .venv-ocr/bin/python -r
scripts/ocr_benchmark/requirements.txt --dry-run` checked 21 package constraints
and reported `Would make no changes`. Native Windows execution with colorama
0.4.6 is documented in experiment 6. A regular CI run is automatically scheduled
by this requirements-only change; no OCR source, model or annotation changed.
