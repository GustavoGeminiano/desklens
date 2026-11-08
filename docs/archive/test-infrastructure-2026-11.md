> Historical snapshot from November 2026. Paths and counts below describe that
> experiment; use [the testing guide](../testing.md) for current commands.

# DeskLens Test Infrastructure (`TEST_INFRA.md`)

This document outlines the testing architecture, methodology, dataset fixtures, and execution guidelines for DeskLens, specifically focusing on the Code Token & Word OCR Detection pipeline and the interactive overlay.

---

## 1. Test Architecture Overview

DeskLens employs a lightweight, high-performance dual-track test infrastructure combining Rust native integration testing (`cargo test`) with Node.js native test runner (`node:test` + `node:assert/strict`).

### Key Characteristics:
- **Dual-Track Verification**:
  1. **Backend OCR Engine & Preprocessing**: Native Rust integration tests (`src-tauri/tests/`) exercising image contrast inversion, vertical indentation guide line suppression, horizontal band segmentation, token splitting, and native WinRT/Tesseract OCR pipelines.
  2. **Frontend Selection & Overlay Interaction**: Hermetic Node.js test suites (`tests/`) testing DOM coordinate scaling, interactive pill highlights, multi-line indentation preservation, and clipboard copying in isolated VM contexts.
- **Empirical Ground-Truth Dataset Fixtures**:
  - Embedded real-world screenshot crops extracted directly from user bug reports (`tests/fixtures/real_editor_asimov_crop.png`, `tests/fixtures/real_editor_gutter_crop.png`, `tests/fixtures/real_editor_baseline_crop.png`).
  - Realistic multi-line editor fixtures with 1px vertical guide lines for dark and light themes (`tests/fixtures/vscode_dark_real.png`, `tests/fixtures/vscode_light_real.png`).
- **Zero Third-Party Test Runners**: Uses Rust standard test harness and Node.js native test runner (`npm test`) without heavy external dependencies.
- **Strict 4-Tier Testing Methodology**: Structures test cases systematically from unit primitives up to empirical real-world screenshot validations.

---

## 2. Test Suites Directory Map

| Suite File | Track | Tests | Category | Description |
|---|---|---|---|---|
| `src-tauri/tests/code_benchmark_e2e_tests.rs` | Rust (Backend) | 44 | Benchmark & E2E | Comprehensive 4-Tier test suite covering code keywords, operators, delimiters, vertical guide suppression, dynamic pitch, and real Asimov screenshot benchmarks (TC-REAL-1 to TC-BOX-7). |
| `src-tauri/tests/code_ocr_tests.rs` | Rust (Backend) | 25 | Regression | Native OCR pipeline regression tests on synthetic loop/dark/light code benchmarks. |
| `src-tauri/tests/adversarial_tests.rs` | Rust (Backend) | 16 | Stress | TSV fuzzing, aspect ratio bounds, and NaN coordinate resilience. |
| `src-tauri/tests/ocr_newspaper_tests.rs` | Rust (Backend) | 12 | Layout | Multi-column layout, margin rejection, and headline separation. |
| `tests/code_selection.test.mjs` | Node.js (Frontend) | 13 | UI/UX | Interactive pill highlight positioning, multi-line drag selection, proportional font snapping, and hanging indent preservation. |
| `tests/translation_e2e.test.mjs` | Node.js (Frontend) | 44 | E2E & Translation | Paragraph clustering, contextual machine translation, detected language extraction, and unified translation overlays. |
| `tests/overlay.test.mjs` | Node.js (Frontend) | 7 | Frontend UI | Base overlay coordinate mapping and dynamic typography scaling. |
| `tests/language_selection.test.mjs` | Node.js (Frontend) | 5 | Frontend UX | Source/target language dropdowns and localStorage persistence. |
| `tests/adversarial_highlights_stress.test.mjs` | Node.js (Frontend) | 14 | Stress | Sub-3px dust filtering, margin fold tolerance, and extreme dimensions. |
| `tests/adversarial_overlay.test.mjs` | Node.js (Frontend) | 10 | Security | Malformed bounding boxes, XSS injection immunity, and DOM element cleanup. |

---

## 3. 4-Tier Test Methodology (`code_benchmark_e2e_tests.rs`)

The code benchmark test harness is structured strictly around 4 verification tiers:

### Tier 1: Feature Coverage (≥5 tests per feature: 25 tests)
- **Feature 1: Keyword Recognition (`T1.1.1` - `T1.1.5`)**:
  - `T1.1.1`: Python control flow keywords (`for`, `in`, `if`, `else`, `while`, `elif`).
  - `T1.1.2`: Structural definition keywords (`def`, `class`, `return`, `async`, `await`).
  - `T1.1.3`: Namespace and module keywords (`import`, `from`, `with`, `try`, `except`).
  - `T1.1.4`: Cross-language declaration keywords (`let`, `var`, `val`, `fn`, `struct`, `enum`, `impl`).
  - `T1.1.5`: Coloned & short keywords (`if:`, `else:`, `elif:`, `for:`, `while:`, `is`, `in`).
- **Feature 2: Operator Recovery & Integrity (`T1.2.1` - `T1.2.5`)**:
  - `T1.2.1`: Equality and comparison operators (`==`, `!=`, `<=`, `>=`, `<`, `>`, `===`, `!==`).
  - `T1.2.2`: Arithmetic and modulo operators (`%`, `+`, `-`, `*`, `/`, `//`, `**`).
  - `T1.2.3`: Assignment and compound operators (`=`, `+=`, `-=`, `*=`, `/=`, `%=`).
  - `T1.2.4`: Logical and bitwise operators (`&&`, `||`, `!`, `&`, `|`, `^`, `~`).
  - `T1.2.5`: Structural and type operators (`->`, `=>`, `::`, `..`, `.`).
- **Feature 3: Delimiters and Syntax Punctuation (`T1.3.1` - `T1.3.5`)**:
  - `T1.3.1`: Statement terminators and colons (`:`, `;`).
  - `T1.3.2`: Enclosure delimiters (`(`, `)`, `[`, `]`, `{`, `}`).
  - `T1.3.3`: Separators (`,`, `.`, `;`, `:`).
  - `T1.3.4`: String quote normalization (typographic double/single quotes normalized to ASCII `"` and `'`).
  - `T1.3.5`: Fullwidth East Asian syntax normalization (`（`, `）`, `：`, `＝`, `［`, `］`, `≤`, `≥`, `≠`).
- **Feature 4: String Literals & Code Prefixes (`T1.4.1` - `T1.4.5`)**:
  - `T1.4.1`: Formatted string prefix in function call: `print(f"%i is even")` segmenting into `print(f` and `"%i`.
  - `T1.4.2`: Odd string literal in loop: `"%i is odd"`.
  - `T1.4.3`: Standalone string literals: `"metric_result"`.
  - `T1.4.4`: Dictionary key-value strings: `{'result'`.
  - `T1.4.5`: Elimination of invisible zero-width characters (`\u{200B}`, `\u{FEFF}`) from string tokens.
- **Feature 5: Line Number & Gutter Separation (`T1.5.1` - `T1.5.5`)**:
  - `T1.5.1`: Gutter digits `1`..`5` parsed without merging into code tokens.
  - `T1.5.2`: Gutter digits `7`..`10` separated from terminal console output.
  - `T1.5.3`: Multi-digit line number `10` preserved without splitting into `1` and `0`.
  - `T1.5.4`: Horizontal gutter gap preserves relative code indentation.
  - `T1.5.5`: Tight crop without gutter numbers produces valid lines and words.

### Tier 2: Boundary & Corner Cases (8 tests)
- `T2.1` (`TC-GUIDE-2`): **Vertical Guide Lines Resilience**: 1px grey vertical lines descending through indentation columns do not bridge rows or collapse image into a single mega-band.
- `T2.2` (`TC-SPLIT-3`): **Dynamic Pitch Adaptation**: Inter-line pitch across 30..50px dynamically segmented at genuine ink valleys without slicing glyphs in half.
- `T2.3`: **Dark vs Light Editor Themes**: Accurate contrast inversion on dark editor background (`#1e1e1e`) and light background (`#ffffff`).
- `T2.4` (`TC-SHORT-5`): **Standalone Short Lines**: Isolated short lines like `else:` detected with valid non-zero bounding box.
- `T2.5` (`TC-ZERO-6`): **Monospace Slashed-Zero Terminal Output**: `0 is even` recognized with digit `0`, word `is`, and word `even`.
- `T2.6`: **Aspect Ratio on Narrow Tokens**: Single-character and narrow tokens (`i`, `1`, `!`, `%`, `if`) with aspect ratio > 2.8 preserved.
- `T2.7`: **Glued Operator Expression Segmentation**: Chains like `2==0:` segment into `2`, `==`, `0`, `:` without coordinate drift or overlapping bounds.
- `T2.8` (`TC-BOX-7`): **Bounding Box Non-Overlap Invariants**: For all tokens, $w > 0$, $h > 0$, $x \ge 0$, $y \ge 0$, and within each line $x_i + w_i \le x_{i+1}$.

### Tier 3: Cross-Feature Combinations (5 tests)
- `T3.1`: **Python Conditional Loop Construct**: Simultaneous keywords (`for`, `in`, `if`, `else:`), operators (`%`, `==`), string calls (`print(f`), and numbers.
- `T3.2`: **Typed Function with Operators**: `def calculate(limit: int = 10) -> bool:` with return type arrow and colon.
- `T3.3`: **Nested Indentation Snapping**: 0-space `for`, 4-space `if`, 8-space `print` indentation hierarchy preserved.
- `T3.4`: **Mixed Code and Terminal Output**: Code buffer preceding an inter-block gap followed by console output lines.
- `T3.5`: **Dual-Engine TSV Coordinate Scaling**: Tesseract/WinRT TSV parsing with 2.0x DPI upscaling and coordinate recovery.

### Tier 4: Real-World Application Scenarios (6 tests)
- `T4.1` (**TC-REAL-1**): **Real Asimov Loop Graphic Verbatim Crop** (`tests/fixtures/real_editor_asimov_crop.png` from `media_1793996902023_9c063b4f.png`):
  - Line 1: `for`, `i`, `in`, `range(10):`
  - Line 2: `if`, `i`, `%`, `2`, `==`, `0:` (Zero dropped tokens)
  - Line 3: `print(f`, `"%i`, `is`, `even")`
  - Line 4: `else:`
  - Line 5: `print(f`, `"%i`, `is`, `odd")`
  - Line 7: `0`, `is`, `even`
  - Line 8: `1`, `is`, `odd`
  - Line 9: `2`, `is`, `even`
  - Line 10: `3`, `is`, `odd`
- `T4.2` (**TC-GUTTER-2**): **Real Asimov Gutter Crop** (`tests/fixtures/real_editor_gutter_crop.png` from `media_1793996685095_927ef312.png`):
  - Validates gutter line numbers `1`..`5` and `7`..`10` alongside code lines.
- `T4.3` (**TC-BASELINE-3**): **Nov 4 Baseline Crop** (`tests/fixtures/real_editor_baseline_crop.png` from `media_1793829362348_f78149df.png`):
  - Validates detection across tight baseline editor crop.
- `T4.4` (**TC-VSCODE-DARK**): **VS Code Dark Theme Snippet** (`tests/fixtures/vscode_dark_real.png`):
  - Validates dark theme with active 1px vertical guide lines at columns 4 and 8.
- `T4.5` (**TC-VSCODE-LIGHT**): **VS Code Light Theme Snippet** (`tests/fixtures/vscode_light_real.png`):
  - Validates light theme with active 1px vertical guide lines.
- `T4.6` (**TC-BOX-7**): **Bounding Box Non-Overlap and Tight Wrapping**:
  - Validates that bounding boxes across real screenshot tokens are non-zero, non-inverted, and non-overlapping.

---

## 4. Execution Guidelines

### Running Backend Code Benchmark E2E Tests
```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test code_benchmark_e2e_tests
```

### Running All Backend Rust Tests
```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

### Running Frontend Tests
```powershell
npm test
# OR directly:
node --test tests/*.test.mjs
```
