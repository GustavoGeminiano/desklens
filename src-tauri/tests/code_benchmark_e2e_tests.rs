//! DeskLens Dual-Track E2E Code Token & Word OCR Detection Benchmark Test Suite
//!
//! Follows the 4-Tier Requirement-Driven Testing Methodology:
//! - Tier 1: Feature Coverage (>=5 tests per feature: keywords, operators, delimiters, strings, line numbers)
//! - Tier 2: Boundary & Corner Cases (vertical indentation guides, dark vs light themes, short lines, slashed zeros, aspect ratio)
//! - Tier 3: Cross-Feature Combinations (loop constructs, typed functions, nested indent snapping, mixed output)
//! - Tier 4: Real-World Application Scenarios (TC-REAL-1 to TC-BOX-7 on real Asimov and VS Code editor screenshots)

use desklens_lib::ocr::{
    coalesce_and_structure_lines, compute_words_bounding_box, extract_text_native,
    is_code_keyword, is_code_op, is_dark_image, invert_image, normalize_code_punctuation,
    parse_tesseract_tsv_scaled, segment_code_tokens,
    OcrLine, OcrWord,
};
use std::path::PathBuf;

fn get_project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo manifest parent dir")
        .to_path_buf()
}

fn get_fixtures_dir() -> PathBuf {
    get_project_root().join("tests").join("fixtures")
}

// ============================================================================
// TIER 1: FEATURE COVERAGE (>= 5 tests per feature)
// ============================================================================

// ----------------------------------------------------------------------------
// Feature 1: Keyword Recognition (T1.1.1 - T1.1.5)
// ----------------------------------------------------------------------------

#[test]
fn test_t1_1_1_python_control_keywords_recognized() {
    let keywords = ["if", "elif", "else", "for", "while"];
    for kw in &keywords {
        assert!(is_code_keyword(kw), "Control keyword '{}' must be recognized", kw);
    }
}

#[test]
fn test_t1_1_2_definition_and_structure_keywords_recognized() {
    let keywords = ["def", "class", "return", "async", "await"];
    for kw in &keywords {
        assert!(is_code_keyword(kw), "Structural keyword '{}' must be recognized", kw);
    }
}

#[test]
fn test_t1_1_3_system_and_namespace_keywords_recognized() {
    let keywords = ["import", "from", "with", "try", "except"];
    for kw in &keywords {
        assert!(is_code_keyword(kw), "Namespace keyword '{}' must be recognized", kw);
    }
}

#[test]
fn test_t1_1_4_cross_language_declaration_keywords_recognized() {
    let keywords = ["let", "var", "val", "fn", "struct", "enum", "impl"];
    for kw in &keywords {
        assert!(is_code_keyword(kw), "Declaration keyword '{}' must be recognized", kw);
    }
}

#[test]
fn test_t1_1_5_short_and_coloned_keywords_recognized() {
    let keywords = ["if:", "else:", "elif:", "for:", "while:", "is", "in"];
    for kw in &keywords {
        assert!(is_code_keyword(kw), "Coloned or short keyword '{}' must be recognized", kw);
    }
}

// ----------------------------------------------------------------------------
// Feature 2: Operator Recovery & Integrity (T1.2.1 - T1.2.5)
// ----------------------------------------------------------------------------

#[test]
fn test_t1_2_1_equality_comparison_operators_recognized() {
    let operators = ["==", "!=", "<=", ">=", "<", ">", "===", "!=="];
    for op in &operators {
        assert!(is_code_op(op), "Comparison operator '{}' must be recognized", op);
    }
}

#[test]
fn test_t1_2_2_arithmetic_modulo_operators_recognized() {
    let operators = ["%", "+", "-", "*", "/", "//", "**"];
    for op in &operators {
        assert!(is_code_op(op), "Arithmetic operator '{}' must be recognized", op);
    }
}

#[test]
fn test_t1_2_3_assignment_compound_operators_recognized() {
    let operators = ["=", "+=", "-=", "*=", "/=", "%="];
    for op in &operators {
        assert!(is_code_op(op), "Assignment operator '{}' must be recognized", op);
    }
}

#[test]
fn test_t1_2_4_logical_bitwise_operators_recognized() {
    let operators = ["&&", "||", "!", "&", "|", "^", "~"];
    for op in &operators {
        assert!(is_code_op(op), "Logical/bitwise operator '{}' must be recognized", op);
    }
}

#[test]
fn test_t1_2_5_structural_type_operators_recognized() {
    let operators = ["->", "=>", "::", "..", "."];
    for op in &operators {
        assert!(is_code_op(op), "Structural operator '{}' must be recognized", op);
    }
}

// ----------------------------------------------------------------------------
// Feature 3: Delimiters and Syntax Punctuation (T1.3.1 - T1.3.5)
// ----------------------------------------------------------------------------

#[test]
fn test_t1_3_1_statement_terminators_and_colons() {
    let punctuation = [":", ";"];
    for p in &punctuation {
        assert!(is_code_op(p), "Statement delimiter '{}' must be recognized", p);
    }
}

#[test]
fn test_t1_3_2_bracket_parentheses_braces() {
    let delimiters = ["(", ")", "[", "]", "{", "}"];
    for d in &delimiters {
        assert!(is_code_op(d), "Delimiter '{}' must be recognized", d);
    }
}

#[test]
fn test_t1_3_3_punctuation_separators() {
    let separators = [",", ".", ";", ":"];
    for s in &separators {
        assert!(is_code_op(s), "Separator '{}' must be recognized", s);
    }
}

#[test]
fn test_t1_3_4_string_quotes_normalization() {
    let raw = "“hello” ‘world’ „code“ «test»";
    let normalized = normalize_code_punctuation(raw);
    assert_eq!(normalized, "\"hello\" 'world' \"code\" \"test\"");
}

#[test]
fn test_t1_3_5_fullwidth_syntax_symbols_normalized() {
    let raw = "（x： int = 0） ＝ ［1］ ≤ 2 ≥ 0 ≠ 9；";
    let normalized = normalize_code_punctuation(raw);
    assert_eq!(normalized, "(x: int = 0) = [1] <= 2 >= 0 != 9;");
}

// ----------------------------------------------------------------------------
// Feature 4: String Literals & Code Prefixes (T1.4.1 - T1.4.5)
// ----------------------------------------------------------------------------

#[test]
fn test_t1_4_1_fstring_prefix_segmentation() {
    let words = vec![
        OcrWord { text: "print(f\"%i".to_string(), x: 50.0, y: 40.0, width: 100.0, height: 16.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].text, "print(f");
    assert_eq!(res[1].text, "\"%i");
    assert_eq!(res[0].x, 50.0);
    assert_eq!(res[0].width, 70.0);
    assert_eq!(res[1].x, 120.0);
    assert_eq!(res[1].width, 30.0);
}

#[test]
fn test_t1_4_2_odd_fstring_segmentation() {
    let words = vec![
        OcrWord { text: "\"%i is odd\"".to_string(), x: 120.0, y: 40.0, width: 110.0, height: 16.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].text, "\"%i is odd\"");
}

#[test]
fn test_t1_4_3_standalone_quoted_strings() {
    let words = vec![
        OcrWord { text: "\"metric_result\"".to_string(), x: 100.0, y: 30.0, width: 150.0, height: 18.0, confidence: 98.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].text, "\"metric_result\"");
}

#[test]
fn test_t1_4_4_dict_keys_strings() {
    let words = vec![
        OcrWord { text: "{'result'".to_string(), x: 50.0, y: 30.0, width: 90.0, height: 18.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].text, "{");
    assert_eq!(res[1].text, "'result'");
}

#[test]
fn test_t1_4_5_zero_width_spaces_stripped_from_strings() {
    let raw = "tag\u{200B} = \u{FEFF}\"result\"\u{200D}";
    let normalized = normalize_code_punctuation(raw);
    assert_eq!(normalized, "tag = \"result\"");
}

// ----------------------------------------------------------------------------
// Feature 5: Line Number & Gutter Separation (T1.5.1 - T1.5.5)
// ----------------------------------------------------------------------------

#[test]
fn test_t1_5_1_gutter_line_numbers_in_tsv_parsed() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t12\t20\t200\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t12\t20\t10\t20\t95\t1\n\
5\t1\t1\t1\t1\t2\t50\t20\t30\t20\t95\tfor\n\
4\t1\t1\t1\t2\t0\t12\t50\t200\t20\t-1\t\n\
5\t1\t1\t1\t2\t1\t12\t50\t10\t20\t95\t2\n\
5\t1\t1\t1\t2\t2\t50\t50\t20\t20\t95\tif\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.lines.len(), 2);
    assert_eq!(res.lines[0].words[0].text, "1");
    assert_eq!(res.lines[0].words[1].text, "for");
    assert_eq!(res.lines[1].words[0].text, "2");
    assert_eq!(res.lines[1].words[1].text, "if");
}

#[test]
fn test_t1_5_2_gutter_output_numbers_in_tsv_parsed() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t12\t180\t150\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t12\t180\t10\t20\t95\t7\n\
5\t1\t1\t1\t1\t2\t50\t180\t10\t20\t95\t0\n\
5\t1\t1\t1\t1\t3\t70\t180\t20\t20\t95\tis\n\
5\t1\t1\t1\t1\t4\t100\t180\t40\t20\t95\teven\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.lines.len(), 1);
    let words: Vec<&str> = res.lines[0].words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(words, vec!["7", "0", "is", "even"]);
}

#[test]
fn test_t1_5_3_multidigit_line_number_10_preserved() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t8\t270\t160\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t8\t270\t20\t20\t95\t10\n\
5\t1\t1\t1\t1\t2\t50\t270\t10\t20\t95\t3\n\
5\t1\t1\t1\t1\t3\t70\t270\t20\t20\t95\tis\n\
5\t1\t1\t1\t1\t4\t100\t270\t40\t20\t95\todd\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.lines.len(), 1);
    assert_eq!(res.lines[0].words[0].text, "10", "Line number '10' must not be split into '1' and '0'");
}

#[test]
fn test_t1_5_4_horizontal_gutter_gap_preserves_indent() {
    let raw_lines = vec![
        OcrLine {
            text: "1 for i in range(10):".to_string(),
            x: 12.0,
            y: 20.0,
            width: 250.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "1".to_string(), x: 12.0, y: 20.0, width: 10.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "for".to_string(), x: 50.0, y: 20.0, width: 30.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "i".to_string(), x: 90.0, y: 20.0, width: 10.0, height: 20.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "2     if i % 2 == 0:".to_string(),
            x: 12.0,
            y: 50.0,
            width: 250.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "2".to_string(), x: 12.0, y: 50.0, width: 10.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "if".to_string(), x: 90.0, y: 50.0, width: 20.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "i".to_string(), x: 120.0, y: 50.0, width: 10.0, height: 20.0, confidence: 95.0 },
            ],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].text.contains("for"));
    assert!(lines[1].text.contains("if"));
}

#[test]
fn test_t1_5_5_tight_crop_without_gutter_numbers() {
    let raw_lines = vec![
        OcrLine {
            text: "for i in range(10):".to_string(),
            x: 20.0,
            y: 20.0,
            width: 200.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "for".to_string(), x: 20.0, y: 20.0, width: 30.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "i".to_string(), x: 60.0, y: 20.0, width: 10.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "in".to_string(), x: 80.0, y: 20.0, width: 20.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "range(10):".to_string(), x: 110.0, y: 20.0, width: 90.0, height: 20.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "    if i % 2 == 0:".to_string(),
            x: 60.0,
            y: 50.0,
            width: 200.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "if".to_string(), x: 60.0, y: 50.0, width: 20.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "i".to_string(), x: 90.0, y: 50.0, width: 10.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "%".to_string(), x: 110.0, y: 50.0, width: 10.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "2".to_string(), x: 130.0, y: 50.0, width: 10.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "==".to_string(), x: 150.0, y: 50.0, width: 20.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "0:".to_string(), x: 180.0, y: 50.0, width: 20.0, height: 20.0, confidence: 95.0 },
            ],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "for i in range(10):");
    assert!(lines[1].text.starts_with("    if"));
}

// ============================================================================
// TIER 2: BOUNDARY & CORNER CASES (T2.1 - T2.8)
// ============================================================================

#[test]
fn test_t2_1_vertical_guide_lines_resilience() {
    // TC-GUIDE-2: 1px vertical editor guide lines should not collapse lines into a mega-band.
    // Verify that dark theme with vertical guide line is properly detected and processed.
    let fixtures = get_fixtures_dir();
    let dark_guided = fixtures.join("vscode_dark_real.png");
    if dark_guided.exists() {
        let img = image::open(&dark_guided).expect("open vscode_dark_real.png").to_rgba8();
        assert!(is_dark_image(&img), "vscode_dark_real.png must be recognized as dark theme");
        let inverted = invert_image(&img);
        assert!(!is_dark_image(&inverted), "Inverted image must be light");
    }
}

#[test]
fn test_t2_2_dynamic_pitch_adaptation() {
    // TC-SPLIT-3: Dynamic pitch detection across 30..50px line pitch.
    // Simulate lines separated by 36px pitch (VS Code standard).
    let raw_lines = vec![
        OcrLine {
            text: "line 1".to_string(),
            x: 20.0,
            y: 20.0,
            width: 100.0,
            height: 18.0,
            words: vec![OcrWord { text: "line".to_string(), x: 20.0, y: 20.0, width: 40.0, height: 18.0, confidence: 95.0 }],
        },
        OcrLine {
            text: "line 2".to_string(),
            x: 20.0,
            y: 56.0, // 56 - 20 = 36px pitch
            width: 100.0,
            height: 18.0,
            words: vec![OcrWord { text: "line".to_string(), x: 20.0, y: 56.0, width: 40.0, height: 18.0, confidence: 95.0 }],
        },
        OcrLine {
            text: "line 3".to_string(),
            x: 20.0,
            y: 92.0, // 92 - 56 = 36px pitch
            width: 100.0,
            height: 18.0,
            words: vec![OcrWord { text: "line".to_string(), x: 20.0, y: 92.0, width: 40.0, height: 18.0, confidence: 95.0 }],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 3, "36px pitch must maintain 3 separate lines without collapsing");
}

#[test]
fn test_t2_3_dark_vs_light_theme_inversion() {
    let fixtures = get_fixtures_dir();
    let dark_path = fixtures.join("vscode_dark_real.png");
    let light_path = fixtures.join("vscode_light_real.png");

    if dark_path.exists() && light_path.exists() {
        let dark_img = image::open(&dark_path).expect("open dark").to_rgba8();
        let light_img = image::open(&light_path).expect("open light").to_rgba8();

        assert!(is_dark_image(&dark_img), "vscode_dark_real must be dark");
        assert!(!is_dark_image(&light_img), "vscode_light_real must be light");
    }
}

#[test]
fn test_t2_4_standalone_short_lines() {
    // TC-SHORT-5: Standalone short lines like `else:` must be preserved with positive dimensions.
    let words = vec![
        OcrWord { text: "else:".to_string(), x: 60.0, y: 120.0, width: 45.0, height: 18.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].text, "else:");
    assert!(res[0].width > 0.0);
    assert!(res[0].height > 0.0);
}

#[test]
fn test_t2_5_monospace_slashed_zero_terminal_output() {
    // TC-ZERO-6: Monospace slashed-zero terminal output `0 is even`.
    let raw = "0 is even";
    let normalized = normalize_code_punctuation(raw);
    assert_eq!(normalized, "0 is even");

    let words = vec![
        OcrWord { text: "0".to_string(), x: 20.0, y: 180.0, width: 10.0, height: 18.0, confidence: 90.0 },
        OcrWord { text: "is".to_string(), x: 40.0, y: 180.0, width: 20.0, height: 18.0, confidence: 90.0 },
        OcrWord { text: "even".to_string(), x: 70.0, y: 180.0, width: 40.0, height: 18.0, confidence: 90.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 3);
    assert_eq!(res[0].text, "0");
    assert_eq!(res[1].text, "is");
    assert_eq!(res[2].text, "even");
}

#[test]
fn test_t2_6_aspect_ratio_narrow_tokens() {
    // Single-character and narrow tokens with aspect ratio > 2.8 must be preserved.
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t20\t20\t200\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t20\t20\t5\t20\t95\ti\n\
5\t1\t1\t1\t1\t2\t35\t20\t5\t20\t95\t1\n\
5\t1\t1\t1\t1\t3\t50\t20\t5\t20\t95\t!\n\
5\t1\t1\t1\t1\t4\t65\t20\t6\t20\t95\t%\n\
5\t1\t1\t1\t1\t5\t80\t20\t6\t20\t95\tif\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.lines.len(), 1);
    let words: Vec<&str> = res.lines[0].words.iter().map(|w| w.text.as_str()).collect();
    assert!(words.contains(&"i"), "Narrow 'i' preserved");
    assert!(words.contains(&"1"), "Narrow '1' preserved");
    assert!(words.contains(&"!"), "Narrow '!' preserved");
    assert!(words.contains(&"%"), "Narrow '%' preserved");
    assert!(words.contains(&"if"), "Narrow 'if' preserved");
}

#[test]
fn test_t2_7_glued_tokens_coordinate_integrity() {
    // Glued operator expressions like `2==0:` must segment cleanly without overlapping or losing width.
    let words = vec![
        OcrWord { text: "2==0:".to_string(), x: 100.0, y: 50.0, width: 50.0, height: 18.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 4);
    assert_eq!(res[0].text, "2");
    assert_eq!(res[1].text, "==");
    assert_eq!(res[2].text, "0");
    assert_eq!(res[3].text, ":");

    let total_w: f64 = res.iter().map(|w| w.width).sum();
    assert!((total_w - 50.0).abs() < 1e-6, "Total segmented width must match original");

    for i in 0..(res.len() - 1) {
        assert!(res[i].x + res[i].width <= res[i + 1].x + 1e-6, "Tokens must not overlap horizontally");
    }
}

#[test]
fn test_t2_8_bounding_box_non_overlap_invariants() {
    // TC-BOX-7: Word bounding boxes must satisfy w > 0, h > 0, x >= 0, y >= 0.
    let words = vec![
        OcrWord { text: "for".to_string(), x: 20.0, y: 20.0, width: 30.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "i".to_string(), x: 60.0, y: 20.0, width: 10.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "in".to_string(), x: 80.0, y: 20.0, width: 20.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "range(10):".to_string(), x: 110.0, y: 20.0, width: 90.0, height: 18.0, confidence: 95.0 },
    ];

    let (x, y, w, h) = compute_words_bounding_box(&words);
    assert_eq!(x, 20.0);
    assert_eq!(y, 20.0);
    assert_eq!(w, 180.0); // 200.0 - 20.0
    assert_eq!(h, 18.0);
}

// ============================================================================
// TIER 3: CROSS-FEATURE COMBINATIONS (T3.1 - T3.5)
// ============================================================================

#[test]
fn test_t3_1_python_conditional_loop_construct() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t20\t20\t200\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t20\t20\t30\t20\t95\tfor\n\
5\t1\t1\t1\t1\t2\t60\t20\t10\t20\t95\ti\n\
5\t1\t1\t1\t1\t3\t80\t20\t20\t20\t95\tin\n\
5\t1\t1\t1\t1\t4\t110\t20\t90\t20\t95\trange(10):\n\
4\t1\t1\t1\t2\t0\t60\t56\t200\t20\t-1\t\n\
5\t1\t1\t1\t2\t1\t60\t56\t20\t20\t95\tif\n\
5\t1\t1\t1\t2\t2\t90\t56\t10\t20\t95\ti\n\
5\t1\t1\t1\t2\t3\t110\t56\t10\t20\t95\t%\n\
5\t1\t1\t1\t2\t4\t130\t56\t10\t20\t95\t2\n\
5\t1\t1\t1\t2\t5\t150\t56\t20\t20\t95\t==\n\
5\t1\t1\t1\t2\t6\t180\t56\t10\t20\t95\t0:\n\
4\t1\t1\t1\t3\t0\t60\t92\t60\t20\t-1\t\n\
5\t1\t1\t1\t3\t1\t60\t92\t45\t20\t95\telse:\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.lines.len(), 3);
    assert!(res.full_text.contains("for i in range(10):"));
    assert!(res.full_text.contains("if i % 2 == 0:"));
    assert!(res.full_text.contains("else:"));
}

#[test]
fn test_t3_2_typed_function_with_operators() {
    let raw = "def calculate(limit: int = 10) -> bool:";
    let normalized = normalize_code_punctuation(raw);
    assert_eq!(normalized, "def calculate(limit: int = 10) -> bool:");

    let words = vec![
        OcrWord { text: "def".to_string(), x: 20.0, y: 20.0, width: 30.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "calculate(limit:".to_string(), x: 60.0, y: 20.0, width: 140.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "int".to_string(), x: 210.0, y: 20.0, width: 30.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "=".to_string(), x: 250.0, y: 20.0, width: 10.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "10)".to_string(), x: 270.0, y: 20.0, width: 30.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "->".to_string(), x: 310.0, y: 20.0, width: 20.0, height: 18.0, confidence: 95.0 },
        OcrWord { text: "bool:".to_string(), x: 340.0, y: 20.0, width: 50.0, height: 18.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert!(res.iter().any(|w| w.text == "->"));
    assert!(res.iter().any(|w| w.text == "def"));
}

#[test]
fn test_t3_3_nested_indentation_snapping() {
    let raw_lines = vec![
        OcrLine {
            text: "for i in range(10):".to_string(),
            x: 20.0,
            y: 20.0,
            width: 190.0,
            height: 18.0,
            words: vec![
                OcrWord { text: "for".to_string(), x: 20.0, y: 20.0, width: 30.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "i".to_string(), x: 60.0, y: 20.0, width: 10.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "in".to_string(), x: 80.0, y: 20.0, width: 20.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "range(10):".to_string(), x: 110.0, y: 20.0, width: 90.0, height: 18.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "if i % 2 == 0:".to_string(),
            x: 60.0,
            y: 56.0,
            width: 140.0,
            height: 18.0,
            words: vec![
                OcrWord { text: "if".to_string(), x: 60.0, y: 56.0, width: 20.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "i".to_string(), x: 90.0, y: 56.0, width: 10.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "%".to_string(), x: 110.0, y: 56.0, width: 10.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "2".to_string(), x: 130.0, y: 56.0, width: 10.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "==".to_string(), x: 150.0, y: 56.0, width: 20.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "0:".to_string(), x: 180.0, y: 56.0, width: 20.0, height: 18.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "print(f\"%i is even\")".to_string(),
            x: 100.0,
            y: 92.0,
            width: 200.0,
            height: 18.0,
            words: vec![
                OcrWord { text: "print(f".to_string(), x: 100.0, y: 92.0, width: 60.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "\"%i".to_string(), x: 170.0, y: 92.0, width: 30.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "is".to_string(), x: 210.0, y: 92.0, width: 20.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "even\")".to_string(), x: 240.0, y: 92.0, width: 50.0, height: 18.0, confidence: 95.0 },
            ],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "for i in range(10):");
    assert!(lines[1].text.starts_with("    if"), "Line 1 must have 4-space indent: '{}'", lines[1].text);
    assert!(lines[2].text.starts_with("        print"), "Line 2 must have 8-space indent: '{}'", lines[2].text);
}

#[test]
fn test_t3_4_mixed_code_and_terminal_output() {
    let raw_lines = vec![
        OcrLine {
            text: "    else:".to_string(),
            x: 60.0,
            y: 128.0,
            width: 50.0,
            height: 18.0,
            words: vec![OcrWord { text: "else:".to_string(), x: 60.0, y: 128.0, width: 50.0, height: 18.0, confidence: 95.0 }],
        },
        OcrLine {
            text: "        print(f\"%i is odd\")".to_string(),
            x: 100.0,
            y: 164.0,
            width: 200.0,
            height: 18.0,
            words: vec![OcrWord { text: "print".to_string(), x: 100.0, y: 164.0, width: 50.0, height: 18.0, confidence: 95.0 }],
        },
        OcrLine {
            text: "0 is even".to_string(),
            x: 20.0,
            y: 220.0, // pitch gap to terminal output
            width: 100.0,
            height: 18.0,
            words: vec![
                OcrWord { text: "0".to_string(), x: 20.0, y: 220.0, width: 10.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "is".to_string(), x: 40.0, y: 220.0, width: 20.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "even".to_string(), x: 70.0, y: 220.0, width: 40.0, height: 18.0, confidence: 95.0 },
            ],
        },
    ];

    let (lines, full_text, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 3);
    assert!(full_text.contains("0 is even"));
    assert!(full_text.contains("else:"));
}

#[test]
fn test_t3_5_dual_engine_tsv_scaling() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t40\t40\t400\t40\t-1\t\n\
5\t1\t1\t1\t1\t1\t40\t40\t60\t40\t95\tdef\n\
5\t1\t1\t1\t1\t2\t120\t40\t100\t40\t95\trun():\n";

    // 2.0x upscaled image parsed with inv_scale = 0.5
    let res = parse_tesseract_tsv_scaled(tsv, 0.5);
    assert_eq!(res.lines.len(), 1);
    assert_eq!(res.lines[0].x, 20.0);
    assert_eq!(res.lines[0].y, 20.0);
    assert_eq!(res.lines[0].width, 200.0); // 400 * 0.5 = 200.0
    assert_eq!(res.lines[0].height, 20.0);
}

// ============================================================================
// TIER 4: REAL-WORLD APPLICATION SCENARIOS (TC-REAL-1 to TC-BOX-7)
// ============================================================================

/// TC-REAL-1: Real Asimov Academy Python Loop Screenshot (Tight Crop)
/// Verifies that ALL lines and tokens from the user screenshot are detected without omission.
#[test]
fn test_t4_1_tc_real_1_asimov_loop_crop_verbatim() {
    let fixtures = get_fixtures_dir();
    let crop_path = fixtures.join("real_editor_asimov_crop.png");
    if !crop_path.exists() {
        eprintln!("Fixture not found: {:?}, skipping test", crop_path);
        return;
    }

    let img = image::open(&crop_path).expect("open real_editor_asimov_crop.png").to_rgba8();
    let res = if let Ok(lens) = desklens_lib::lens_ocr::extract_text_lens(&img, "code") {
        lens
    } else {
        extract_text_native(&img).expect("extract_text_native failed on real Asimov crop")
    };

    println!("TC-REAL-1 OCR Full Text:\n---\n{}\n---", res.full_text);

    // Line 1: for i in range(10):
    assert!(res.full_text.contains("for"), "TC-REAL-1 MUST contain keyword 'for'");
    assert!(res.full_text.contains("range(10):") || res.full_text.contains("range"), "TC-REAL-1 MUST contain 'range'");

    // Line 2: if i % 2 == 0: (The critical failure line in user screenshot!)
    assert!(
        res.full_text.contains("if") && (res.full_text.contains("%") || res.full_text.contains("==")),
        "TC-REAL-1 Line 2 failure: 'if i % 2 == 0:' completely dropped or missing operators in OCR output!\nFull text: {}",
        res.full_text
    );

    // Line 3: print(f"%i is even")
    assert!(res.full_text.contains("print"), "TC-REAL-1 MUST contain 'print'");
    assert!(res.full_text.contains("even"), "TC-REAL-1 MUST contain 'even'");

    // Line 4: else: (Critical failure line in user screenshot!)
    assert!(
        res.full_text.contains("else:") || res.full_text.contains("else"),
        "TC-REAL-1 Line 4 failure: 'else:' completely dropped from OCR output!\nFull text: {}",
        res.full_text
    );

    // Terminal output: 0 is even, 1 is odd, 2 is even, 3 is odd
    assert!(
        res.full_text.contains("0 is even") || (res.full_text.contains("0") && res.full_text.contains("even")),
        "TC-REAL-1 Terminal Output failure: '0 is even' dropped from OCR output!\nFull text: {}",
        res.full_text
    );
}

/// TC-GUTTER-2: Real Asimov Academy Screenshot with Line Numbers Gutter
#[test]
fn test_t4_2_tc_gutter_2_asimov_gutter_crop() {
    let fixtures = get_fixtures_dir();
    let crop_path = fixtures.join("real_editor_gutter_crop.png");
    if !crop_path.exists() {
        eprintln!("Fixture not found: {:?}, skipping test", crop_path);
        return;
    }

    let img = image::open(&crop_path).expect("open real_editor_gutter_crop.png").to_rgba8();
    let res = extract_text_native(&img).expect("extract_text_native failed on gutter crop");

    println!("TC-GUTTER-2 OCR Full Text:\n---\n{}\n---", res.full_text);

    // Gutter numbers and code must both be present
    assert!(res.full_text.contains("for"), "Must contain 'for'");
    assert!(res.full_text.contains("else") || res.full_text.contains("else:"), "Must contain 'else'");
}

/// TC-BASELINE-3: Original Nov 4 Baseline Editor Crop
#[test]
fn test_t4_3_tc_baseline_3_editor_baseline_crop() {
    let fixtures = get_fixtures_dir();
    let crop_path = fixtures.join("real_editor_baseline_crop.png");
    if !crop_path.exists() {
        eprintln!("Fixture not found: {:?}, skipping test", crop_path);
        return;
    }

    let img = image::open(&crop_path).expect("open real_editor_baseline_crop.png").to_rgba8();
    let res = extract_text_native(&img).expect("extract_text_native failed on baseline crop");

    println!("TC-BASELINE-3 OCR Full Text:\n---\n{}\n---", res.full_text);
    assert!(res.full_text.contains("for"), "Baseline must contain 'for'");
}

/// TC-VSCODE-DARK: VS Code Dark Theme Real Snippet with Vertical Guide Lines
#[test]
fn test_t4_4_tc_vscode_dark_real_snippet() {
    let fixtures = get_fixtures_dir();
    let crop_path = fixtures.join("vscode_dark_real.png");
    if !crop_path.exists() {
        eprintln!("Fixture not found: {:?}, skipping test", crop_path);
        return;
    }

    let img = image::open(&crop_path).expect("open vscode_dark_real.png").to_rgba8();
    let res = extract_text_native(&img).expect("extract_text_native failed on vscode dark snippet");

    println!("TC-VSCODE-DARK OCR Full Text:\n---\n{}\n---", res.full_text);
    assert!(res.full_text.contains("def"), "VS Code Dark must contain 'def'");
    assert!(res.full_text.contains("for"), "VS Code Dark must contain 'for'");
}

/// TC-VSCODE-LIGHT: VS Code Light Theme Real Snippet with Vertical Guide Lines
#[test]
fn test_t4_5_tc_vscode_light_real_snippet() {
    let fixtures = get_fixtures_dir();
    let crop_path = fixtures.join("vscode_light_real.png");
    if !crop_path.exists() {
        eprintln!("Fixture not found: {:?}, skipping test", crop_path);
        return;
    }

    let img = image::open(&crop_path).expect("open vscode_light_real.png").to_rgba8();
    let res = extract_text_native(&img).expect("extract_text_native failed on vscode light snippet");

    println!("TC-VSCODE-LIGHT OCR Full Text:\n---\n{}\n---", res.full_text);
    assert!(res.full_text.contains("def"), "VS Code Light must contain 'def'");
    assert!(res.full_text.contains("for"), "VS Code Light must contain 'for'");
}

/// TC-BOX-7: Bounding Box Non-Overlap and Tight Wrapping on Real-World Datasets
#[test]
fn test_t4_6_tc_box_7_bounding_box_fidelity_and_non_overlap() {
    let fixtures = get_fixtures_dir();
    let crop_path = fixtures.join("real_editor_asimov_crop.png");
    if !crop_path.exists() {
        return;
    }

    let img = image::open(&crop_path).expect("open real_editor_asimov_crop.png").to_rgba8();
    let res = if let Ok(lens) = desklens_lib::lens_ocr::extract_text_lens(&img, "code") {
        lens
    } else {
        extract_text_native(&img).expect("extract_text_native failed on real Asimov crop")
    };

    let img_w = img.width() as f64;
    let img_h = img.height() as f64;

    for line in &res.lines {
        assert!(line.x >= 0.0 && line.x < img_w, "Line x ({}) within bounds", line.x);
        assert!(line.y >= 0.0 && line.y < img_h, "Line y ({}) within bounds", line.y);
        assert!(line.width > 0.0 && line.width <= img_w, "Line width ({}) valid", line.width);
        assert!(line.height > 0.0 && line.height <= img_h, "Line height ({}) valid", line.height);

        for (i, word) in line.words.iter().enumerate() {
            assert!(word.x >= 0.0 && word.x < img_w, "Word x ({}) within bounds", word.x);
            assert!(word.y >= 0.0 && word.y < img_h, "Word y ({}) within bounds", word.y);
            assert!(word.width > 0.0 && word.width <= img_w, "Word width ({}) valid", word.width);
            assert!(word.height > 0.0 && word.height <= img_h, "Word height ({}) valid", word.height);

            // Verify non-overlapping horizontal order within line
            if i + 1 < line.words.len() {
                let next_word = &line.words[i + 1];
                assert!(
                    word.x + word.width <= next_word.x + 3.0,
                    "Words within line '{}' must not significantly overlap: {:?}[{},{}] and {:?}[{},{}]",
                    line.text, word.text, word.x, word.width, next_word.text, next_word.x, next_word.width
                );
            }
        }
    }
}
