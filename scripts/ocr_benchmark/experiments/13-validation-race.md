# Experiment 13 — validation race and stale session cleanup

Run 42504744818 (ce61077) stopped early on Linux in the existing diagnostic test
`test_timeout_stops_descendant_too`. Job 128804834985 shows FileNotFoundError for
`/proc/4966/stat`: the process was reaped between `exists()` and `read_text()`.
This was a test observation race, not a surviving OCR process. The assertion now
reads once and treats FileNotFoundError as successful termination; running
non-zombie descendants still fail after the same two-second cleanup allowance.

Review additionally found that a late crop reply was ignored by the frontend but
its backend session could remain active after Clear. Late replies now request
conditional cleanup of their own crop ID, preserving newer sessions. The existing
out-of-order regression asserts that cleanup. Queued worker calls recheck session
identity after acquiring the worker lock, so obsolete queued crops do not launch
unnecessary inference.

The experimental workflow now covers frontend and backend build inputs and test
files, and cancels superseded runs. Final native validation is rerun on this
revision; earlier completed 7c654fc results remain in experiment 12.

Local validation: 249 frontend, 18 diagnostic benchmark, and 5 Rust session tests
passed. The final frozen worker before these session-only changes passed literal
100/125/150/200% crops, including the accented input directory and empty PATH.
