//! Challenger M1.2 Gen 2 Empirical Stress Test Suite
//! Focus areas:
//! 1. No line numbers (normal code snippet without gutter) -> verify `detect_gutter_boundary` returns None
//!    and no words are discarded or shifted.
//! 2. Gutter line numbers: Single-digit (1..9), double-digit (10..99), and three-digit (100..999).
//! 3. Indented Python code with 4, 8, 12 leading spaces -> verify `indent_spaces` is preserved accurately.
//! 4. Band splitting resilience under continuous vertical gutter dividers and indentation guides.
//! 5. Short numeric token preservation (e.g. `0 is even`) without word dropping or false shifting.

use desklens_lib::ocr::*;
use image::{Rgba, RgbaImage};

// ============================================================================
// 1. NO LINE NUMBERS: NORMAL CODE SNIPPET WITHOUT GUTTER
// ============================================================================

#[test]
fn test_gutter_detection_no_line_numbers_clean_code() {
    let (w, h) = (350u32, 180u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Draw 4 lines of clean Python code without any gutter or divider
    // Line 0: def compute(x):
    // Line 1:     y = x * 2
    // Line 2:     if y > 10:
    // Line 3:         return y
    let line_y_starts = [15u32, 50u32, 85u32, 120u32];
    let line_x_starts = [20u32, 52u32, 52u32, 84u32]; // 0, 4, 4, 8 spaces relative offset

    for (y_top, x_left) in line_y_starts.iter().zip(line_x_starts.iter()) {
        for y in *y_top..(*y_top + 16) {
            for x in *x_left..(*x_left + 180) {
                if x % 40 > 8 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
    }

    // 1. detect_gutter_boundary MUST return None and false
    let gutter = detect_gutter_boundary(&img);
    println!(
        "[EMPIRICAL OBS] Clean code crop (no gutter): has_line_numbers={}, boundary={:?}",
        gutter.has_line_numbers, gutter.gutter_boundary_x
    );
    // Observe whether it falsely reports line numbers:
    let falsely_detected = gutter.has_line_numbers;
    println!("[CRITICAL FINDING] Clean code with indentation falsely detected as gutter? {}", falsely_detected);

    // 3. coalesce_and_structure_lines on raw lines: verify no words discarded or shifted
    let raw_lines = vec![
        OcrLine {
            text: "def compute(x):".to_string(),
            x: 20.0,
            y: 15.0,
            width: 140.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "def".to_string(), x: 20.0, y: 15.0, width: 28.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "compute(x):".to_string(), x: 55.0, y: 15.0, width: 105.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "y = x * 2".to_string(),
            x: 52.0,
            y: 50.0,
            width: 80.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "y".to_string(), x: 52.0, y: 50.0, width: 10.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "=".to_string(), x: 68.0, y: 50.0, width: 10.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "x".to_string(), x: 84.0, y: 50.0, width: 10.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "*".to_string(), x: 100.0, y: 50.0, width: 10.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "2".to_string(), x: 116.0, y: 50.0, width: 10.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "if y > 10:".to_string(),
            x: 52.0,
            y: 85.0,
            width: 85.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "if".to_string(), x: 52.0, y: 85.0, width: 18.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "y".to_string(), x: 76.0, y: 85.0, width: 10.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: ">".to_string(), x: 92.0, y: 85.0, width: 10.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "10:".to_string(), x: 108.0, y: 85.0, width: 25.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "return y".to_string(),
            x: 84.0,
            y: 120.0,
            width: 70.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "return".to_string(), x: 84.0, y: 120.0, width: 50.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "y".to_string(), x: 140.0, y: 120.0, width: 10.0, height: 16.0, confidence: 100.0 },
            ],
        },
    ];

    let (structured_lines, full_text, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(structured_lines.len(), 4, "Must retain all 4 lines");

    // Verify no words are dropped:
    assert!(full_text.contains("def compute(x):"), "full_text must contain Line 0");
    assert!(full_text.contains("return y"), "full_text must contain Line 3");

    // Line 0 must have 0 leading indent spaces
    assert!(
        structured_lines[0].text.starts_with("def"),
        "Line 0 must NOT have accidental leading indentation: '{}'",
        structured_lines[0].text
    );

    // Line 1 and 2 must have 4 spaces
    assert!(
        structured_lines[1].text.starts_with("    y"),
        "Line 1 must have 4-space indent: '{}'",
        structured_lines[1].text
    );
    assert!(
        structured_lines[2].text.starts_with("    if"),
        "Line 2 must have 4-space indent: '{}'",
        structured_lines[2].text
    );

    // Line 3 must have 8 spaces
    assert!(
        structured_lines[3].text.starts_with("        return"),
        "Line 3 must have 8-space indent: '{}'",
        structured_lines[3].text
    );

    // Verify word positions did not get altered/shifted corruptly
    assert_eq!(structured_lines[0].words[0].x, 20.0);
    assert_eq!(structured_lines[0].words[0].text, "def");
}

#[test]
fn test_gutter_detection_unindented_assignments() {
    let (w, h) = (300u32, 120u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // 3 unindented lines:
    // x = 10
    // y = 20
    // z = 30
    // Letters at x=20..28, space at x=29..38 (gap 10px), '=' at x=39..47, space at x=48..56 (gap 9px), '10' at x=57..73
    for i in 0..3 {
        let y_top = 15 + i * 32;
        // Letter x/y/z
        for y in y_top..(y_top + 16) { for x in 20..28 { img.put_pixel(x, y, Rgba([0, 0, 0, 255])); } }
        // '='
        for y in y_top..(y_top + 16) { for x in 39..47 { img.put_pixel(x, y, Rgba([0, 0, 0, 255])); } }
        // Number
        for y in y_top..(y_top + 16) { for x in 57..73 { img.put_pixel(x, y, Rgba([0, 0, 0, 255])); } }
    }

    let gutter = detect_gutter_boundary(&img);
    println!(
        "[EMPIRICAL OBS] Unindented assignments 'x = 10': has_line_numbers={}, boundary={:?}",
        gutter.has_line_numbers, gutter.gutter_boundary_x
    );
    assert!(
        !gutter.has_line_numbers,
        "CRITICAL: Unindented variable assignments 'x = 10' must NOT trigger false gutter detection, got {:?}",
        gutter
    );
}

// ============================================================================
// 2. SINGLE-DIGIT, DOUBLE-DIGIT, AND TRIPLE-DIGIT GUTTER LINE NUMBERS
// ============================================================================

#[test]
fn test_gutter_detection_single_digit_line_numbers() {
    // 5 lines with line numbers 1..5 in the gutter
    // Digits at x=15..25, vertical divider line at x=38, code starting at x=55
    let (w, h) = (280u32, 160u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Gutter digits 1..5
    for i in 0..5 {
        let y_top = 15 + i * 28;
        for y in y_top..(y_top + 14) {
            for x in 16..24 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    // Divider line at x=38
    for y in 5..155 {
        img.put_pixel(38, y, Rgba([180, 180, 180, 255]));
    }

    // Code lines at x=55..240
    for i in 0..5 {
        let y_top = 15 + i * 28;
        for y in y_top..(y_top + 14) {
            for x in 55..240 {
                if x % 35 > 8 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
    }

    let gutter = detect_gutter_boundary(&img);
    println!(
        "[EMPIRICAL OBS] Single-digit gutter: has_line_numbers={}, boundary={:?}",
        gutter.has_line_numbers, gutter.gutter_boundary_x
    );
    assert!(gutter.has_line_numbers, "Must detect 1-digit gutter line numbers");
    assert!(gutter.gutter_boundary_x.is_some(), "Must compute gutter boundary for 1-digit");
    let gx = gutter.gutter_boundary_x.unwrap();
    assert!(
        gx >= 36.0 && gx <= 44.0,
        "Gutter boundary must align with divider line: got {}",
        gx
    );

    // Test coalesce_and_structure_lines with single-digit gutter
    let raw_lines = vec![
        OcrLine {
            text: "1 for i in range(5):".to_string(),
            x: 16.0,
            y: 15.0,
            width: 180.0,
            height: 14.0,
            words: vec![
                OcrWord { text: "1".to_string(), x: 16.0, y: 15.0, width: 8.0, height: 14.0, confidence: 100.0 },
                OcrWord { text: "for".to_string(), x: 55.0, y: 15.0, width: 24.0, height: 14.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "2 print(i)".to_string(),
            x: 16.0,
            y: 43.0,
            width: 140.0,
            height: 14.0,
            words: vec![
                OcrWord { text: "2".to_string(), x: 16.0, y: 43.0, width: 8.0, height: 14.0, confidence: 100.0 },
                OcrWord { text: "print(i)".to_string(), x: 87.0, y: 43.0, width: 60.0, height: 14.0, confidence: 100.0 },
            ],
        },
    ];

    let (structured, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(structured.len(), 2);
    // Line 1: '1' is gutter, 'for' is base code token
    assert!(structured[0].text.starts_with("1    for"), "Line 1 must format gutter and code: '{}'", structured[0].text);
    // Line 2: '2' is gutter, 'print' at x=87 vs x=55 (32px offset = 4 spaces)
    assert!(structured[1].text.contains("    print"), "Line 2 must preserve 4-space code indent: '{}'", structured[1].text);
}

#[test]
fn test_gutter_detection_double_digit_line_numbers() {
    // 5 lines with line numbers 10..14 in the gutter
    // Digits at x=14..30 (double digit width ~16px), divider at x=44, code at x=60
    let (w, h) = (300u32, 160u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Gutter digits 10..14
    for i in 0..5 {
        let y_top = 15 + i * 28;
        for y in y_top..(y_top + 14) {
            for x in 14..30 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    // Divider line at x=44
    for y in 5..155 {
        img.put_pixel(44, y, Rgba([180, 180, 180, 255]));
    }

    // Code lines at x=60..260
    for i in 0..5 {
        let y_top = 15 + i * 28;
        for y in y_top..(y_top + 14) {
            for x in 60..260 {
                if x % 35 > 8 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
    }

    let gutter = detect_gutter_boundary(&img);
    println!(
        "[EMPIRICAL OBS] Double-digit gutter: has_line_numbers={}, boundary={:?}",
        gutter.has_line_numbers, gutter.gutter_boundary_x
    );
    assert!(gutter.has_line_numbers, "Must detect 2-digit gutter line numbers");
    assert!(gutter.gutter_boundary_x.is_some(), "Must compute gutter boundary for 2-digit");
    let gx = gutter.gutter_boundary_x.unwrap();
    assert!(
        gx >= 42.0 && gx <= 50.0,
        "Gutter boundary must align with divider line: got {}",
        gx
    );

    // Test coalesce_and_structure_lines with 2-digit line numbers: "10", "11"
    let raw_lines = vec![
        OcrLine {
            text: "10 x = 42".to_string(),
            x: 14.0,
            y: 15.0,
            width: 150.0,
            height: 14.0,
            words: vec![
                OcrWord { text: "10".to_string(), x: 14.0, y: 15.0, width: 16.0, height: 14.0, confidence: 100.0 },
                OcrWord { text: "x".to_string(), x: 60.0, y: 15.0, width: 10.0, height: 14.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "11 y = x * 2".to_string(),
            x: 14.0,
            y: 43.0,
            width: 180.0,
            height: 14.0,
            words: vec![
                OcrWord { text: "11".to_string(), x: 14.0, y: 43.0, width: 16.0, height: 14.0, confidence: 100.0 },
                OcrWord { text: "y".to_string(), x: 92.0, y: 43.0, width: 10.0, height: 14.0, confidence: 100.0 },
            ],
        },
    ];

    let (structured, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(structured.len(), 2);
    assert!(structured[0].text.starts_with("10    x"), "Line 10 must format gutter and code: '{}'", structured[0].text);
    assert!(structured[1].text.contains("    y"), "Line 11 must preserve 4-space code indent: '{}'", structured[1].text);
}

#[test]
fn test_gutter_detection_triple_digit_line_numbers() {
    // 5 lines with line numbers 100..104 in the gutter
    // Digits at x=10..34 (3-digit width ~24px), vertical divider at x=50, code at x=70
    let (w, h) = (340u32, 160u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Gutter digits 100..104
    for i in 0..5 {
        let y_top = 15 + i * 28;
        for y in y_top..(y_top + 14) {
            for x in 10..34 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    // Divider line at x=50
    for y in 5..155 {
        img.put_pixel(50, y, Rgba([180, 180, 180, 255]));
    }

    // Code lines at x=70..300
    for i in 0..5 {
        let y_top = 15 + i * 28;
        for y in y_top..(y_top + 14) {
            for x in 70..300 {
                if x % 35 > 8 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
    }

    let gutter = detect_gutter_boundary(&img);
    println!(
        "[EMPIRICAL OBS] Triple-digit gutter: has_line_numbers={}, boundary={:?}",
        gutter.has_line_numbers, gutter.gutter_boundary_x
    );
    assert!(gutter.has_line_numbers, "Must detect 3-digit gutter line numbers");
    assert!(gutter.gutter_boundary_x.is_some(), "Must compute gutter boundary for 3-digit");
    let gx = gutter.gutter_boundary_x.unwrap();
    assert!(
        gx >= 48.0 && gx <= 56.0,
        "Gutter boundary must align with divider line: got {}",
        gx
    );

    // Test coalesce_and_structure_lines with 3-digit line numbers: "100", "101"
    let raw_lines = vec![
        OcrLine {
            text: "100 def main():".to_string(),
            x: 10.0,
            y: 15.0,
            width: 170.0,
            height: 14.0,
            words: vec![
                OcrWord { text: "100".to_string(), x: 10.0, y: 15.0, width: 24.0, height: 14.0, confidence: 100.0 },
                OcrWord { text: "def".to_string(), x: 68.0, y: 15.0, width: 24.0, height: 14.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "101 pass".to_string(),
            x: 10.0,
            y: 43.0,
            width: 120.0,
            height: 14.0,
            words: vec![
                OcrWord { text: "101".to_string(), x: 10.0, y: 43.0, width: 24.0, height: 14.0, confidence: 100.0 },
                OcrWord { text: "pass".to_string(), x: 100.0, y: 43.0, width: 32.0, height: 14.0, confidence: 100.0 },
            ],
        },
    ];

    let (structured, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(structured.len(), 2);
    assert!(structured[0].text.starts_with("100    def"), "Line 100 must format gutter and code: '{}'", structured[0].text);
    assert!(structured[1].text.contains("    pass"), "Line 101 must preserve 4-space code indent: '{}'", structured[1].text);
}

// ============================================================================
// 3. INDENTED PYTHON CODE WITH 4, 8, 12 LEADING SPACES PRESERVATION
// ============================================================================

#[test]
fn test_python_indentation_preservation_4_8_12_spaces_without_gutter() {
    // Monospace font character width: 8.0px
    // 0 indent = 0px offset (x = 30.0)
    // 4 spaces = 32px offset (x = 62.0)
    // 8 spaces = 64px offset (x = 94.0)
    // 12 spaces = 96px offset (x = 126.0)
    let raw_lines = vec![
        OcrLine {
            text: "def process(items):".to_string(),
            x: 30.0,
            y: 20.0,
            width: 150.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "def".to_string(), x: 30.0, y: 20.0, width: 24.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "process(items):".to_string(), x: 62.0, y: 20.0, width: 118.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "for item in items:".to_string(),
            x: 62.0,
            y: 50.0,
            width: 140.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "for".to_string(), x: 62.0, y: 50.0, width: 24.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "item".to_string(), x: 94.0, y: 50.0, width: 32.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "if item.is_valid():".to_string(),
            x: 94.0,
            y: 80.0,
            width: 150.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "if".to_string(), x: 94.0, y: 80.0, width: 16.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "item.is_valid():".to_string(), x: 118.0, y: 80.0, width: 126.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "item.save()".to_string(),
            x: 126.0,
            y: 110.0,
            width: 90.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "item.save()".to_string(), x: 126.0, y: 110.0, width: 90.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "else:".to_string(),
            x: 94.0,
            y: 140.0,
            width: 40.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "else:".to_string(), x: 94.0, y: 140.0, width: 40.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "discard(item)".to_string(),
            x: 126.0,
            y: 170.0,
            width: 100.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "discard(item)".to_string(), x: 126.0, y: 170.0, width: 100.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "return True".to_string(),
            x: 62.0,
            y: 200.0,
            width: 90.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "return".to_string(), x: 62.0, y: 200.0, width: 48.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "True".to_string(), x: 118.0, y: 200.0, width: 34.0, height: 16.0, confidence: 100.0 },
            ],
        },
    ];

    let (structured, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(structured.len(), 7, "Must retain all 7 lines");

    // Line 0: 0 spaces indent
    assert!(
        structured[0].text.starts_with("def") && !structured[0].text.starts_with(" "),
        "Line 0 must have 0 indent spaces: '{}'",
        structured[0].text
    );

    // Line 1: exactly 4 spaces
    assert!(
        structured[1].text.starts_with("    for") && !structured[1].text.starts_with("     "),
        "Line 1 must have exactly 4 indent spaces: '{}'",
        structured[1].text
    );

    // Line 2: exactly 8 spaces
    assert!(
        structured[2].text.starts_with("        if") && !structured[2].text.starts_with("         "),
        "Line 2 must have exactly 8 indent spaces: '{}'",
        structured[2].text
    );

    // Line 3: exactly 12 spaces
    assert!(
        structured[3].text.starts_with("            item.save()") && !structured[3].text.starts_with("             "),
        "Line 3 must have exactly 12 indent spaces: '{}'",
        structured[3].text
    );

    // Line 4: exactly 8 spaces
    assert!(
        structured[4].text.starts_with("        else:") && !structured[4].text.starts_with("         "),
        "Line 4 must have exactly 8 indent spaces: '{}'",
        structured[4].text
    );

    // Line 5: exactly 12 spaces
    assert!(
        structured[5].text.starts_with("            discard(item)") && !structured[5].text.starts_with("             "),
        "Line 5 must have exactly 12 indent spaces: '{}'",
        structured[5].text
    );

    // Line 6: exactly 4 spaces
    assert!(
        structured[6].text.starts_with("    return") && !structured[6].text.starts_with("     "),
        "Line 6 must have exactly 4 indent spaces: '{}'",
        structured[6].text
    );
}

#[test]
fn test_python_indentation_preservation_4_8_12_spaces_with_gutter() {
    // Same Python code but with gutter line numbers 1..7 at x=12.0
    // Base code margin at x=55.0
    // 0 indent = x=55.0
    // 4 spaces = x=87.0 (32px offset)
    // 8 spaces = x=119.0 (64px offset)
    // 12 spaces = x=151.0 (96px offset)
    let raw_lines = vec![
        OcrLine {
            text: "1 def process(items):".to_string(),
            x: 12.0,
            y: 20.0,
            width: 170.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "1".to_string(), x: 12.0, y: 20.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "def".to_string(), x: 55.0, y: 20.0, width: 24.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "process(items):".to_string(), x: 87.0, y: 20.0, width: 118.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "2 for item in items:".to_string(),
            x: 12.0,
            y: 50.0,
            width: 160.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "2".to_string(), x: 12.0, y: 50.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "for".to_string(), x: 87.0, y: 50.0, width: 24.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "3 if item.is_valid():".to_string(),
            x: 12.0,
            y: 80.0,
            width: 170.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "3".to_string(), x: 12.0, y: 80.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "if".to_string(), x: 119.0, y: 80.0, width: 16.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "4 item.save()".to_string(),
            x: 12.0,
            y: 110.0,
            width: 120.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "4".to_string(), x: 12.0, y: 110.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "item.save()".to_string(), x: 151.0, y: 110.0, width: 90.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "5 else:".to_string(),
            x: 12.0,
            y: 140.0,
            width: 60.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "5".to_string(), x: 12.0, y: 140.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "else:".to_string(), x: 119.0, y: 140.0, width: 40.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "6 discard(item)".to_string(),
            x: 12.0,
            y: 170.0,
            width: 130.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "6".to_string(), x: 12.0, y: 170.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "discard(item)".to_string(), x: 151.0, y: 170.0, width: 100.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "7 return True".to_string(),
            x: 12.0,
            y: 200.0,
            width: 110.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "7".to_string(), x: 12.0, y: 200.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "return".to_string(), x: 87.0, y: 200.0, width: 48.0, height: 16.0, confidence: 100.0 },
            ],
        },
    ];

    let (structured, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(structured.len(), 7);

    // Line 0: "1    def" (gutter 1 + 4 gutter spaces + 0 code indent spaces)
    assert!(
        structured[0].text.starts_with("1    def"),
        "Line 0 must have gutter + 0 code indent: '{}'",
        structured[0].text
    );

    // Line 1: "2        for" (gutter 2 + 4 gutter spaces + 4 code indent spaces = 8 spaces after 2)
    assert!(
        structured[1].text.starts_with("2        for"),
        "Line 1 must have gutter + 4 code indent: '{}'",
        structured[1].text
    );

    // Line 2: "3            if" (gutter 3 + 4 gutter spaces + 8 code indent spaces = 12 spaces after 3)
    assert!(
        structured[2].text.starts_with("3            if"),
        "Line 2 must have gutter + 8 code indent: '{}'",
        structured[2].text
    );

    // Line 3: "4                item.save()" (gutter 4 + 4 gutter spaces + 12 code indent spaces = 16 spaces after 4)
    assert!(
        structured[3].text.starts_with("4                item.save()"),
        "Line 3 must have gutter + 12 code indent: '{}'",
        structured[3].text
    );

    // Line 4: "5            else:" (gutter 5 + 4 gutter spaces + 8 code indent spaces = 12 spaces after 5)
    assert!(
        structured[4].text.starts_with("5            else:"),
        "Line 4 must have gutter + 8 code indent: '{}'",
        structured[4].text
    );

    // Line 5: "6                discard(item)" (gutter 6 + 4 gutter spaces + 12 code indent spaces = 16 spaces after 6)
    assert!(
        structured[5].text.starts_with("6                discard(item)"),
        "Line 5 must have gutter + 12 code indent: '{}'",
        structured[5].text
    );

    // Line 6: "7        return" (gutter 7 + 4 gutter spaces + 4 code indent spaces = 8 spaces after 7)
    assert!(
        structured[6].text.starts_with("7        return"),
        "Line 6 must have gutter + 4 code indent: '{}'",
        structured[6].text
    );
}

// ============================================================================
// 4. BAND SPLITTING WITH GUTTER DIVIDER AND INDENTATION GUIDES
// ============================================================================

#[test]
fn test_band_splitting_with_gutter_divider_and_guidelines() {
    let (w, h) = (320u32, 220u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    let num_lines = 6;
    let pitch = 34u32;
    let line_h = 16u32;

    // Draw 6 code lines
    for i in 0..num_lines {
        let y_top = 10 + i * pitch;
        // Gutter digit at x=14..22
        for y in y_top..(y_top + line_h) {
            for x in 14..22 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        // Code words at x=55..280
        for y in y_top..(y_top + line_h) {
            for x in 55..280 {
                if x % 30 > 6 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
    }

    // Guideline 1: Vertical gutter divider running continuous across ALL rows 0..h at x=36
    for y in 0..h {
        img.put_pixel(36, y, Rgba([170, 170, 170, 255]));
    }

    // Guideline 2: Indentation guide at x=78 running continuous across rows 10..200
    for y in 10..200 {
        img.put_pixel(78, y, Rgba([185, 185, 185, 255]));
    }

    // Guideline 3: Indentation guide at x=110 running continuous across rows 40..170
    for y in 40..170 {
        img.put_pixel(110, y, Rgba([185, 185, 185, 255]));
    }

    let mut copy = img.clone();
    let suppressed = suppress_vertical_guidelines(&mut copy);
    let detected_cols = detect_vertical_guideline_columns(&img);
    println!("[DIAGNOSTIC] Suppressed {} pixels, detected guide cols: {:?}", suppressed, detected_cols);

    // Detect horizontal line bands
    let bands = detect_horizontal_line_bands(&img);
    println!(
        "[EMPIRICAL OBS] Band splitting with 3 vertical guidelines: detected {} bands (expected {})",
        bands.len(), num_lines
    );
    for (i, (y0, y1)) in bands.iter().enumerate() {
        println!("  Band {}: y={}..{} (h={}px)", i, y0, y1, y1 - y0);
    }

    assert_eq!(
        bands.len(),
        num_lines as usize,
        "Continuous vertical guidelines must NOT cause line bands to merge or collapse into mega-band"
    );

    for (i, &(y0, y1)) in bands.iter().enumerate() {
        let expected_top = 10 + (i as u32) * pitch;
        let expected_bot = expected_top + line_h;
        assert!(
            y0 <= expected_top + 2 && y1 >= expected_bot - 2,
            "Band {} ({}-{}) does not match line {}",
            i, y0, y1, i
        );
        assert!(
            (y1 - y0) <= 26,
            "Band {} height ({}px) indicates accidental line merging",
            i, y1 - y0
        );
    }
}

// ============================================================================
// 5. SHORT NUMERIC TOKENS (e.g. `0 is even`) PRESERVATION
// ============================================================================

#[test]
fn test_short_numeric_output_tokens_preservation() {
    // In crops of console output without gutter numbers:
    // Line 0: "0 is even"
    // Line 1: "1 is odd"
    // Line 2: "2 is even"
    let raw_lines = vec![
        OcrLine {
            text: "0 is even".to_string(),
            x: 20.0,
            y: 10.0,
            width: 90.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "0".to_string(), x: 20.0, y: 10.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "is".to_string(), x: 34.0, y: 10.0, width: 14.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "even".to_string(), x: 54.0, y: 10.0, width: 32.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "1 is odd".to_string(),
            x: 20.0,
            y: 35.0,
            width: 80.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "1".to_string(), x: 20.0, y: 35.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "is".to_string(), x: 34.0, y: 35.0, width: 14.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "odd".to_string(), x: 54.0, y: 35.0, width: 24.0, height: 16.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "2 is even".to_string(),
            x: 20.0,
            y: 60.0,
            width: 90.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "2".to_string(), x: 20.0, y: 60.0, width: 8.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "is".to_string(), x: 34.0, y: 60.0, width: 14.0, height: 16.0, confidence: 100.0 },
                OcrWord { text: "even".to_string(), x: 54.0, y: 60.0, width: 32.0, height: 16.0, confidence: 100.0 },
            ],
        },
    ];

    let (structured, full_text, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(structured.len(), 3, "Must retain all 3 lines");

    // Because word 0 to word 1 gap is (34 - (20 + 8)) = 6px (< 14px), "0", "1", "2"
    // are NOT treated as gutter numbers, but preserved as regular code/output tokens!
    assert!(full_text.contains("0 is even"), "Must preserve '0 is even' verbatim");
    assert!(full_text.contains("1 is odd"), "Must preserve '1 is odd' verbatim");
    assert!(full_text.contains("2 is even"), "Must preserve '2 is even' verbatim");

    assert_eq!(structured[0].words.len(), 3);
    assert_eq!(structured[0].words[0].text, "0");
    assert_eq!(structured[0].words[1].text, "is");
    assert_eq!(structured[0].words[2].text, "even");
}
