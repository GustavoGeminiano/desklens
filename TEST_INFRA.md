# DeskLens Test Infrastructure (`TEST_INFRA.md`)

This document outlines the testing architecture, methodology, harness design, and execution guidelines for DeskLens.

---

## 1. Test Architecture Overview

DeskLens employs a lightweight, ultra-fast test infrastructure powered by the Node.js native test runner (`node:test` + `node:assert/strict`) and Rust's native `cargo test` framework.

### Key Characteristics:
- **Zero Third-Party Test Dependencies**: Runs directly on Node.js 18+ (tested on Node v24.19.0).
- **Sub-Second Execution**: The entire test suite of 93 tests executes in ~1.1 seconds.
- **Hermetic Isolation**: Tests run in self-contained VM contexts (`node:vm`) with simulated DOM and mocked Tauri IPC channels, avoiding reliance on physical display servers or WebView2 instances.
- **Multi-Tiered Coverage**: Follows the 4-tier testing hierarchy from unit boundaries up to real-world visual artifacts.

---

## 2. Test Suites Directory (`tests/`)

| File | Category | Tests | Description |
|---|---|---|---|
| `tests/translation_e2e.test.mjs` | E2E & Translation | 44 | Comprehensive 4-tier test suite covering paragraph grouping, contextual translation, detected language extraction, language selector flow, unified overlays, and Image 2 verbatim validation. |
| `tests/overlay.test.mjs` | Frontend UI | 7 | Base overlay rendering, coordinate mapping, dynamic font fitting, and click-to-copy interactions. |
| `tests/language_selection.test.mjs` | Frontend UX | 5 | Language selector DOM presence, default options, persistence in `localStorage`, and IPC argument dispatch. |
| `tests/code_selection.test.mjs` | Code OCR | 13 | Interactive code pills, multi-line selection, proportional font snapping, and indentation preservation. |
| `tests/adversarial_highlights_stress.test.mjs` | OCR Robustness | 13 | OCR noise filtering, scanner fold artifacts, dust tolerance, and coordinate boundary stress. |
| `tests/adversarial_overlay.test.mjs` | Security & Stress | 10 | Malformed boxes, XSS/HTML injection resilience, extreme dimensions, and lifecycle element cleanup. |
| `tests/challenger_m1_frontend.test.mjs` | Verification | 1 | Aspect-ratio filtering evaluation on single-character glyphs ('I', '1', '!'). |
| **Total** | | **93** | **100% Pass** |

---

## 3. 4-Tier Test Methodology

`tests/translation_e2e.test.mjs` is structured strictly according to the 4-tier testing methodology:

### Tier 1: Feature Coverage (>=5 tests per feature)
- **Feature 1: Paragraph Grouping (`T1.1.1` - `T1.1.5`)**:
  - Contiguous line clustering by vertical pitch.
  - Mathematical union bounding box $[ \min(x), \min(y), \max(x+w)-\min(x), \max(y+h)-\min(y) ]$.
  - Separation across vertical pitch thresholds ($> 1.5\times$).
  - Whitespace trimming and text concatenation.
  - Column gutter protection ($> 40\text{px}$).
- **Feature 2: Contextual Machine Translation (`T1.2.1` - `T1.2.5`)**:
  - Grammatical negation preservation ("não se" + "preocupe!" -> "don't worry", avoiding affirmative "worry!").
  - Elimination of hallucinated closing clauses ("do not hesitate to contact us").
  - Syntactic cohesion across sentence splits.
  - Direct association of translation with paragraph union bounding box.
  - Dual IPC contract support (legacy array `[...]` and enriched `{ boxes: [...], detected_source_lang: "..." }`).
- **Feature 3: Detected Language Extraction (`T1.3.1` - `T1.3.5`)**:
  - Extraction from translation provider response.
  - Support for `sourceLang: "auto"`.
  - Normalization of locale tags (`pt-BR` -> `pt`, `en-US` -> `en`).
  - Fallback on missing/null detection metadata.
  - Extraction from legacy array payloads with box metadata.
- **Feature 4: Language Selector Flow & Smart Target Switch (`T1.4.1` - `T1.4.5`)**:
  - Presence of `#lang-selector-group` in DOM.
  - Smart target switch (PT source + PT target -> switches to EN).
  - Smart target switch (EN source + EN target -> switches to PT).
  - Persistence of user selections to `localStorage`.
  - Dynamic translation button tooltip and title updates.
- **Feature 5: Unified Paragraph Overlay DOM & Typography (`T1.5.1` - `T1.5.5`)**:
  - Exactly 1 `.lens-translation-box` rendered for multi-line paragraph union.
  - Exact coordinate matching without baseline jitter.
  - CSS stylesheet dark background `#1a1a1a` and multi-line word wrapping.
  - Elimination of collision displacement offset.
  - Click-to-copy interaction copying entire paragraph to clipboard.

### Tier 2: Boundary & Corner Cases (9 tests)
- `T2.1`: Single-line box (isolated label or button) remains single-line with `is_paragraph: false`.
- `T2.2`: Empty, null, and whitespace-only lines safely filtered without creating zero-area overlays.
- `T2.3`: Multi-line paragraph of 5 lines correctly unified into single bounding box.
- `T2.4`: Hyphenated words across line breaks (`"desenvolvi-"` + `"mento"` de-hyphenated into `"desenvolvimento"`).
- `T2.5`: Terminal punctuation (`.`, `!`, `?`) followed by pitch jump establishes paragraph boundary.
- `T2.6`: Ultra-wide ribbon banner ($1600 \times 14\text{px}$) font size safely bounded.
- `T2.7`: Ultra-narrow tall column ($16 \times 350\text{px}$) font size safely bounded.
- `T2.8`: Microscopic 1px and 0px dust boxes discarded before translation dispatch.
- `T2.9`: Fallback timeout resilience: simulated backend network error returns original text cleanly.

### Tier 3: Cross-Feature Combinations (5 tests)
- `T3.1`: Multi-column layout separation: 2 parallel columns at overlapping Y ranges do not merge lines across columns.
- `T3.2`: Code vs natural text preservation: indented code blocks retain structure while prose is paragraph-grouped.
- `T3.3`: Source == Target Language Handling: Portuguese text with Portuguese target redirects to English.
- `T3.4`: End-to-End Pipeline: OCR lines -> Grouping -> Contextual Translation -> Unified DOM Overlay.
- `T3.5`: Mixed content: multi-line paragraph alongside isolated buttons/actions.

### Tier 4: Real-World Scenarios (5 tests)
- `T4.1`: **Image 2 Verbatim Portuguese Paragraph Real-World Verification**:
  - Verbatim text from user screenshot (Image 2):
    - Line 0: `"Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se"`
    - Line 1: `"preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas"`
    - Line 2: `"faz parte do processo de aprendizagem."`
  - Verifies exact union bounding box $[21, 14, 969, 69]$.
  - Verifies 268-character concatenated paragraph text.
  - Verifies English translation preserves negation (`"don't worry"` / `"do not worry"`), forbids isolated affirmative `"worry!"`, forbids hallucinated `"do not hesitate to contact us"`, and links `"Asking questions is part of the learning process"`.
  - Verifies exactly 1 overlay card rendered in DOM and copies all 268 characters (translated) on click.
- `T4.2`: Technical documentation with section header, body paragraph, and code sample.
- `T4.3`: Chat/messaging UI scenario with alternating user bubbles.
- `T4.4`: Academic two-column article excerpt (Abstract vs Introduction).
- `T4.5`: Full user workflow simulation: crop selection -> detected PT -> smart target EN -> translate -> unified overlay -> clipboard copy.

---

## 4. Execution Guidelines

### Running the Full Test Suite
```powershell
npm test
# OR directly via node:
node --test tests/*.test.mjs
```

### Running Specific Test Suites
```powershell
node --test tests/translation_e2e.test.mjs
node --test tests/overlay.test.mjs
node --test tests/language_selection.test.mjs
```

### Backend Rust Verification
```powershell
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
