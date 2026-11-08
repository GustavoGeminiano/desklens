> Historical snapshot from November 2026. Paths and counts below describe that
> experiment; use [the testing guide](../testing.md) for current commands.

# Test Suite Readiness Report (`TEST_READY.md`)

**Date**: 2026-11-06  
**Author**: `test_writer_1_gen3` (Specialized Test Engineer)  
**Status**: **TEST SUITE READY — Benchmark Implemented (41 Passing / 3 Failing on Unpatched Backend)**  
**Target Milestone**: Code Token & Word OCR Detection (Milestones M1–M4)  

---

## 1. Executive Summary

A comprehensive 4-Tier requirement-driven E2E integration test suite has been designed, implemented, and verified in `src-tauri/tests/code_benchmark_e2e_tests.rs`. 

The test suite incorporates:
- **44 new automated Rust integration tests** covering keyword recognition, operator recovery, delimiter normalization, string literals, gutter separation, vertical guide resilience, dynamic pitch adaptation, and real editor screenshot verification.
- **5 empirical screenshot dataset fixtures** stored in `tests/fixtures/`, directly extracted from user bug reports and realistic VS Code environments.
- Combined with the 99 existing frontend tests (`npm test`), the overall test coverage stands at **143 tests**.

### Baseline Execution Result on Current Backend (`src-tauri/src/ocr.rs`):
```powershell
$ cargo test --manifest-path src-tauri/Cargo.toml --test code_benchmark_e2e_tests
test result: FAILED. 41 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.02s
```
- **Tier 1 (Feature Coverage)**: 25/25 PASSED (100%)
- **Tier 2 (Boundary & Corner Cases)**: 8/8 PASSED (100%)
- **Tier 3 (Cross-Feature Combinations)**: 5/5 PASSED (100%)
- **Tier 4 (Real-World Application Scenarios)**: 3/6 PASSED, 3 FAILED:
  - `test_t4_1_tc_real_1_asimov_loop_crop_verbatim`: **FAILED** (Catches exact bug reported in `ORIGINAL_REQUEST.md`)
  - `test_t4_2_tc_gutter_2_asimov_gutter_crop`: **FAILED** (Catches gutter crop token drop)
  - `test_t4_3_tc_baseline_3_editor_baseline_crop`: **FAILED** (Catches baseline crop token drop)
  - `test_t4_4_tc_vscode_dark_real_snippet`: **PASSED**
  - `test_t4_5_tc_vscode_light_real_snippet`: **PASSED**
  - `test_t4_6_tc_box_7_bounding_box_fidelity_and_non_overlap`: **PASSED**

---

## 2. Authoritative Ground Truth & Expected Output Derivation

All benchmark assertions are derived from empirical evidence in `ORIGINAL_REQUEST.md` and Explorer Survey Report `explorer_survey_2_gen3/handoff.md`:

### Ground Truth Token Inventory (Asimov Academy Python Loop Graphic):
- **Source Image**: `hub.asimov.academy/wp-content/uploads/2025/06/codigo-python-loops-exemplo-pratico-1024x689.webp`
- **User Screenshots**:
  - `media_1793996902023_9c063b4f.png` (Tight crop without gutter numbers)
  - `media_1793996685095_927ef312.png` (Wide crop with gutter numbers 1..10)
  - `media_1793829362348_f78149df.png` (Baseline editor crop)
- **Extracted Fixtures (`tests/fixtures/`)**:
  - `real_editor_asimov_crop.png` (461 x 341 px)
  - `real_editor_gutter_crop.png` (485 x 370 px)
  - `real_editor_baseline_crop.png` (461 x 224 px)
  - `vscode_dark_real.png` (540 x 380 px)
  - `vscode_light_real.png` (540 x 380 px)

### Expected OCR Token Inventory:
- **Line 1** ($y \approx 17..40$): `for`, `i`, `in`, `range(10):`
- **Line 2** ($y \approx 50..67$): `if`, `i`, `%`, `2`, `==`, `0:`
- **Line 3** ($y \approx 80..101$): `print(f`, `"%i`, `is`, `even")`
- **Line 4** ($y \approx 117..130$): `else:`
- **Line 5** ($y \approx 142..163$): `print(f`, `"%i`, `is`, `odd")`
- **Line 7** ($y \approx 184..200$): `0`, `is`, `even`
- **Line 8** ($y \approx 213..232$): `1`, `is`, `odd`
- **Line 9** ($y \approx 245..262$): `2`, `is`, `even`
- **Line 10** ($y \approx 275..294$): `3`, `is`, `odd`

---

## 3. Empirical Failure Reproduction on Current Backend

Running the test suite against the unpatched implementation in `src-tauri/src/ocr.rs` captures the exact failure mode reported by the user:

### Output of `test_t4_1_tc_real_1_asimov_loop_crop_verbatim`:
```
TC-REAL-1 OCR Full Text:
---
 etse:


is even

is e-
---
panicked at 'TC-REAL-1 MUST contain keyword 'for''
```
**Diagnostic Breakdown**:
1. **Total Omission of Line 1**: `for i in range(10):` was completely dropped.
2. **Total Omission of Line 2**: `if i % 2 == 0:` was completely dropped.
3. **Total Omission of Line 3**: `print(f"%i is even")` was completely dropped.
4. **Corrupted Line 4**: `else:` was garbled into `etse:`.
5. **Total Omission of Line 5**: `print(f"%i is odd")` was completely dropped.
6. **Mangled Output Lines**: `0 is even` was dropped; `2 is even` had `2` dropped.

### Output of `test_t4_2_tc_gutter_2_asimov_gutter_crop`:
```
TC-GUTTER-2 OCR Full Text:
---
  1
I
      if i



    0 is even
---
panicked at 'Must contain 'for''
```
**Diagnostic Breakdown**:
1. Line 1 `for i in range(10):` dropped (only gutter `1` detected).
2. Line 2 `if i % 2 == 0:` severed into `if i`, dropping `% 2 == 0:`.
3. Line 4 `else:` completely dropped.

---

## 4. Implementation Directives & Escalation Notes for Implementers (Milestone M1 & M2)

To make `test_t4_1_tc_real_1_asimov_loop_crop_verbatim`, `test_t4_2_tc_gutter_2_asimov_gutter_crop`, and `test_t4_3_tc_baseline_3_editor_baseline_crop` pass, the following defects in `src-tauri/src/ocr.rs` must be fixed:

### Defect 1: Indentation Guide Line Collapse into Mega-Band (`src-tauri/src/ocr.rs:262-306`)
- **Observed Behavior**: `detect_horizontal_line_bands` checks `row_ink[y] >= 3`. The 1px vertical editor guide lines at columns 4 and 8 produce 6–7 ink pixels continuously across all rows, preventing `in_band` from ever resetting to false. All lines from Y=12 to Y=310 merge into a single 298px band.
- **Fix Required**:
  - Dynamically calculate row ink threshold (e.g., `row_ink[y] >= max(10, (w as f32 * 0.04) as u32)`), OR
  - Detect and mask continuous 1px vertical guide lines before band projection.

### Defect 2: Horizontal Glyph-Severing by Hardcoded +27px Band Slicer (`src-tauri/src/ocr.rs:308-333`)
- **Observed Behavior**: The fallback band splitter forces cuts within `cur_y0 + 13 .. cur_y0 + 27`. Because real editor line pitch is 30–42px, searching only up to +27px guarantees the cut falls in the middle of character bodies (at ink counts 40, 132, 162, 225), slicing letters in half horizontally.
- **Fix Required**:
  - Replace the static 27px search window with dynamic valley pitch detection (e.g. searching 30..50px around local minima) to split strictly within true inter-line valleys.

### Defect 3: Overfitted Parenthesis-Erasure Hack (`src-tauri/src/ocr.rs:2150-2169`)
- **Observed Behavior**: The recovery pass only attempted recognition with `h_spans[1]` erased assuming `if (i % 2 == 0):`. Standard Python code does not have parentheses around `if i % 2 == 0:`, so the heuristic never triggered.
- **Fix Required**:
  - Remove this brittle hack and rely on clean band isolation and unscaled 1.0x engine pass with 35px padding.

### Defect 4: Short Token Suppression & Operator Recovery (`src-tauri/src/ocr.rs`)
- **Observed Behavior**: Windows OCR drops sparse short tokens (`if`, `i`, `%`, `2`, `==`, `0:`, standalone `else:`) in full-page passes.
- **Fix Required**:
  - Pass clean isolated line bands (with 35–40px padding) to OCR so short code tokens are recognized as a coherent line.
  - Apply operator recovery for `==` and `%`.
  - Normalize slashed zeros (`@` -> `0`) adjacent to numerals.

---

## 5. Verification Commands

### Run the Code Benchmark E2E Test Suite
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test code_benchmark_e2e_tests
```

### Run All Backend Tests
```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

### Run Frontend Highlight & Selection Tests
```powershell
npm test
```
