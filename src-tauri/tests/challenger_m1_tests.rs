//! Challenger M1 Empirical Stress Test Suite
//!
//! Stress-tests:
//! 1. TSV parser adversarial edge cases: malformed inputs, NaN/Inf confidences, negative coords, 0-dims, Unicode & mixed scripts.
//! 2. Column gutter separation: exact empirical threshold where column merging occurs vs preservation across varying gutters (10px - 200px).
//! 3. Aspect ratio checks: behavior of vertical characters ('I', '1', 'l', '!', '[', '|') and whether valid text is dropped.

use desklens_lib::ocr::{
    coalesce_and_structure_lines, parse_tesseract_tsv_scaled, OcrLine, OcrWord,
};

// =============================================================================
// 1. TSV PARSER ADVERSARIAL STRESS TESTS
// =============================================================================

#[test]
fn test_adv_tsv_malformed_and_extreme_inputs() {
    // Empty TSV
    let res = parse_tesseract_tsv_scaled("", 1.0);
    assert_eq!(res.word_count, 0);
    assert_eq!(res.lines.len(), 0);
    assert_eq!(res.full_text, "");

    // Header only
    let res = parse_tesseract_tsv_scaled("level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n", 1.0);
    assert_eq!(res.word_count, 0);

    // Garbage / corrupt lines
    let garbage_tsv = "random text without tabs\n\
1\t2\t3\n\
level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\tnot_a_number\t10\t100\t20\t90.0\tValid\n\
5\t1\t1\t1\t1\t2\t120\tnot_a_number\t100\t20\t90.0\tWord\n\
\n\
\t\t\t\t\t\t\t\t\t\t\t\n\
5\t1\t1\t1\t1\t3\t240\t10\t100\t20\t95.0\tSuccess\n";
    let res = parse_tesseract_tsv_scaled(garbage_tsv, 1.0);
    // Should parse without crashing
    assert!(res.full_text.contains("Success"));
}

#[test]
fn test_adv_tsv_nan_and_inf_confidences() {
    // What happens when confidence is NaN, Inf, -Inf, or -1?
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t10\t10\t80\t20\tNaN\tNanWord\n\
5\t1\t1\t1\t1\t2\t100\t10\t80\t20\tinf\tInfWord\n\
5\t1\t1\t1\t1\t3\t200\t10\t80\t20\t-inf\tNegInfWord\n\
5\t1\t1\t1\t1\t4\t300\t10\t80\t20\t-1\tNegOneWord\n\
5\t1\t1\t1\t1\t5\t400\t10\t80\t20\t90.0\tNormalWord\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    // Check behavior: NegInf and NegOne are < 35.0, should be dropped.
    assert!(!res.full_text.contains("NegInfWord"));
    assert!(!res.full_text.contains("NegOneWord"));
    assert!(res.full_text.contains("NormalWord"));
    println!("[OBSERVATION] NaN word kept?: {}", res.full_text.contains("NanWord"));
    println!("[OBSERVATION] Inf word kept?: {}", res.full_text.contains("InfWord"));
    let json_res = serde_json::to_string(&res);
    println!("[OBSERVATION] JSON serialization with NaN result: {:?}", json_res);
}

#[test]
fn test_adv_tsv_negative_and_zero_dimensions() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t-50\t-20\t80\t20\t90.0\tNegCoords\n\
5\t1\t1\t1\t1\t2\t100\t10\t0\t20\t90.0\tZeroW\n\
5\t1\t1\t1\t1\t3\t200\t10\t80\t0\t90.0\tZeroH\n\
5\t1\t1\t1\t1\t4\t300\t10\t2.5\t20\t90.0\tSub3W\n\
5\t1\t1\t1\t1\t5\t400\t10\t80\t2.5\t90.0\tSub3H\n\
5\t1\t1\t1\t1\t6\t500\t10\t-50\t20\t90.0\tNegW\n\
5\t1\t1\t1\t1\t7\t600\t10\t80\t-20\t90.0\tNegH\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert!(!res.full_text.contains("ZeroW"));
    assert!(!res.full_text.contains("ZeroH"));
    assert!(!res.full_text.contains("Sub3W"));
    assert!(!res.full_text.contains("Sub3H"));
    assert!(!res.full_text.contains("NegW"));
    assert!(!res.full_text.contains("NegH"));
    assert!(res.full_text.contains("NegCoords"));
}

#[test]
fn test_adv_tsv_unicode_emojis_and_mixed_scripts() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t10\t10\t800\t25\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t120\t25\t92.0\tDAILY_NEWS\n\
5\t1\t1\t1\t1\t2\t140\t10\t60\t25\t90.0\t新闻\n\
5\t1\t1\t1\t1\t3\t210\t10\t80\t25\t91.0\t東京\n\
5\t1\t1\t1\t1\t4\t300\t10\t30\t25\t95.0\t📰\n\
5\t1\t1\t1\t1\t5\t340\t10\t30\t25\t95.0\t🚀\n\
5\t1\t1\t1\t1\t6\t380\t10\t100\t25\t93.0\tSão_Paulo\n\
5\t1\t1\t1\t1\t7\t490\t10\t90\t25\t94.0\tMünchen\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert!(res.full_text.contains("DAILY_NEWS"));
    assert!(res.full_text.contains("新闻"));
    assert!(res.full_text.contains("東京"));
    assert!(res.full_text.contains("📰"));
    assert!(res.full_text.contains("🚀"));
    assert!(res.full_text.contains("São_Paulo"));
    assert!(res.full_text.contains("München"));
}

// =============================================================================
// 2. COLUMN GUTTER SEPARATION STRESS TESTS
// =============================================================================

#[test]
fn test_stress_column_gutter_sweep_body_text() {
    // Font height = 16px, character width = 8px.
    // max_allowed_gap = (16 * 3.5).max(8 * 6).max(40) = 56.0px.
    // Gutters: 10px, 30px, 50px, 60px, 100px, 200px.
    let gutters = [10.0, 30.0, 50.0, 55.0, 60.0, 70.0, 100.0, 200.0];

    for &g in &gutters {
        let col1_x = 50.0;
        let col1_w = 150.0;
        let col2_x = col1_x + col1_w + g;
        let col2_w = 150.0;

        let raw_lines = vec![
            OcrLine {
                text: "LEFT COLUMN TEXT".to_string(),
                x: col1_x,
                y: 100.0,
                width: col1_w,
                height: 16.0,
                words: vec![
                    OcrWord { text: "LEFT".to_string(), x: col1_x, y: 100.0, width: 40.0, height: 16.0, confidence: 95.0 },
                    OcrWord { text: "COLUMN".to_string(), x: col1_x + 50.0, y: 100.0, width: 50.0, height: 16.0, confidence: 95.0 },
                    OcrWord { text: "TEXT".to_string(), x: col1_x + 110.0, y: 100.0, width: 40.0, height: 16.0, confidence: 95.0 },
                ],
            },
            OcrLine {
                text: "RIGHT COLUMN TEXT".to_string(),
                x: col2_x,
                y: 100.0,
                width: col2_w,
                height: 16.0,
                words: vec![
                    OcrWord { text: "RIGHT".to_string(), x: col2_x, y: 100.0, width: 45.0, height: 16.0, confidence: 95.0 },
                    OcrWord { text: "COLUMN".to_string(), x: col2_x + 55.0, y: 100.0, width: 50.0, height: 16.0, confidence: 95.0 },
                    OcrWord { text: "TEXT".to_string(), x: col2_x + 115.0, y: 100.0, width: 35.0, height: 16.0, confidence: 95.0 },
                ],
            },
        ];

        let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
        let merged = lines.len() == 1;
        println!("[BODY GUTTER SWEEP] Gutter = {}px -> lines: {}, merged: {}", g, lines.len(), merged);
    }
}

#[test]
fn test_stress_column_gutter_sweep_headline_text() {
    // Large headline font: height = 36px, min_h = 36.
    // max_allowed_gap = (36 * 3.5).max(...) = 126.0px!
    // Gutters: 10px, 30px, 50px, 80px, 100px, 120px, 130px, 200px.
    let gutters = [10.0, 30.0, 50.0, 80.0, 100.0, 120.0, 130.0, 200.0];

    for &g in &gutters {
        let col1_x = 50.0;
        let col1_w = 200.0;
        let col2_x = col1_x + col1_w + g;
        let col2_w = 200.0;

        let raw_lines = vec![
            OcrLine {
                text: "TOP NEWS".to_string(),
                x: col1_x,
                y: 50.0,
                width: col1_w,
                height: 36.0,
                words: vec![
                    OcrWord { text: "TOP".to_string(), x: col1_x, y: 50.0, width: 80.0, height: 36.0, confidence: 96.0 },
                    OcrWord { text: "NEWS".to_string(), x: col1_x + 95.0, y: 50.0, width: 105.0, height: 36.0, confidence: 96.0 },
                ],
            },
            OcrLine {
                text: "GOOD NEWS".to_string(),
                x: col2_x,
                y: 50.0,
                width: col2_w,
                height: 36.0,
                words: vec![
                    OcrWord { text: "GOOD".to_string(), x: col2_x, y: 50.0, width: 90.0, height: 36.0, confidence: 95.0 },
                    OcrWord { text: "NEWS".to_string(), x: col2_x + 100.0, y: 50.0, width: 100.0, height: 36.0, confidence: 95.0 },
                ],
            },
        ];

        let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
        let merged = lines.len() == 1;
        println!("[HEADLINE GUTTER SWEEP] Gutter = {}px -> lines: {}, merged: {}", g, lines.len(), merged);
    }
}

// =============================================================================
// 3. ASPECT RATIO CHECKS FOR VERTICAL CHARACTERS & TOKENS
// =============================================================================

#[test]
fn test_stress_vertical_characters_isolated_and_in_sentence() {
    // Vertical characters: 'I', '1', 'l', '!', '[', ']'
    // Font height = 20px, character width = 5px.
    // Aspect ratio = 20.0 / 5.0 = 4.0 > 2.8!
    // char_count = 1 <= 3!
    // Test sentence: "I see 1 item !"
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t10\t10\t300\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t5\t20\t92.0\tI\n\
5\t1\t1\t1\t1\t2\t25\t10\t30\t20\t95.0\tsee\n\
5\t1\t1\t1\t1\t3\t65\t10\t5\t20\t90.0\t1\n\
5\t1\t1\t1\t1\t4\t80\t10\t40\t20\t93.0\titem\n\
5\t1\t1\t1\t1\t5\t130\t10\t5\t20\t91.0\t!\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    println!("[OBSERVATION VERTICAL CHARS] Full text: {:?}", res.full_text);
    println!("[OBSERVATION VERTICAL CHARS] Word count: {}", res.word_count);

    // Let's check which words survived
    let has_i = res.full_text.contains("I");
    let has_see = res.full_text.contains("see");
    let has_1 = res.full_text.contains("1");
    let has_item = res.full_text.contains("item");
    let has_excl = res.full_text.contains("!");

    println!("[ASPECT RATIO RESULT] 'I' kept: {}, 'see' kept: {}, '1' kept: {}, 'item' kept: {}, '!' kept: {}",
        has_i, has_see, has_1, has_item, has_excl);
}

#[test]
fn test_stress_code_pipe_and_brackets_aspect_ratio() {
    // In code snippets: `|x| [0]`
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t10\t10\t200\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t4\t20\t95.0\t|\n\
5\t1\t1\t1\t1\t2\t20\t10\t10\t20\t95.0\tx\n\
5\t1\t1\t1\t1\t3\t35\t10\t4\t20\t95.0\t|\n\
5\t1\t1\t1\t1\t4\t50\t10\t6\t20\t95.0\t[\n\
5\t1\t1\t1\t1\t5\t60\t10\t10\t20\t95.0\t0\n\
5\t1\t1\t1\t1\t6\t75\t10\t6\t20\t95.0\t]\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    println!("[CODE SYNTAX ASPECT RATIO] Full text: {:?}", res.full_text);
}
