use desklens_lib::ocr::{
    coalesce_and_structure_lines, extract_text_native,
    is_code_keyword, is_code_op, is_dark_image, invert_image, normalize_code_punctuation,
    parse_tesseract_tsv_scaled, segment_code_tokens,
    OcrLine, OcrWord,
};
use std::path::PathBuf;

#[test]
fn test_code_ocr_native_pipeline_end_to_end() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let dark_path = root.join("scripts").join("code_dark.png");
    let light_path = root.join("scripts").join("code_light.png");

    if !dark_path.exists() || !light_path.exists() {
        eprintln!("Test images not found, skipping end-to-end image check: {:?}", dark_path);
        return;
    }

    let dark_img = image::open(&dark_path).expect("open code_dark.png").to_rgba8();
    let light_img = image::open(&light_path).expect("open code_light.png").to_rgba8();

    // 1. Test Light Theme directly through extract_text_native
    let light_res = extract_text_native(&light_img).expect("extract light text");

    assert!(light_res.full_text.contains("def "), "Light theme must contain 'def'");
    assert!(light_res.full_text.contains(":"), "Light theme must contain colon ':'");
    assert!(light_res.full_text.contains("="), "Light theme must contain '=' operator");
    assert!(light_res.full_text.contains("#"), "Light theme must contain comment hash '#'");
    assert!(light_res.full_text.contains("    #"), "Light theme must preserve 4-space indentation for comment");
    assert!(light_res.full_text.contains("    result ="), "Light theme must preserve 4-space indentation for result");
    assert!(light_res.full_text.contains("    tag ="), "Light theme must preserve 4-space indentation for tag");
    assert!(light_res.full_text.contains("    return"), "Light theme must preserve 4-space indentation for return");
    assert!(light_res.full_text.contains("\"metric"), "Light theme must contain double quote '\"'");
    assert!(light_res.full_text.contains("'result'"), "Light theme must contain single quote key ''result''");
    assert!(light_res.full_text.contains("'tag'"), "Light theme must contain single quote key ''tag''");

    // 2. Test Dark Theme directly through extract_text_native
    let dark_res = extract_text_native(&dark_img).expect("extract dark text");

    assert!(dark_res.full_text.contains("def "), "Dark theme must contain 'def'");
    assert!(dark_res.full_text.contains(":"), "Dark theme must contain colon ':'");
    assert!(dark_res.full_text.contains("="), "Dark theme must contain '=' operator");
    assert!(dark_res.full_text.contains("#"), "Dark theme must contain comment hash '#'");
    assert!(dark_res.full_text.contains("    #"), "Dark theme must preserve 4-space indentation for comment");
    assert!(dark_res.full_text.contains("    result ="), "Dark theme must preserve 4-space indentation for result");
    assert!(dark_res.full_text.contains("    tag ="), "Dark theme must preserve 4-space indentation for tag");
    assert!(dark_res.full_text.contains("    return"), "Dark theme must preserve 4-space indentation for return");
    assert!(dark_res.full_text.contains("\"metric"), "Dark theme must contain double quote '\"'");
    assert!(dark_res.full_text.contains("'result'"), "Dark theme must contain single quote key ''result''");
    assert!(dark_res.full_text.contains("'tag'"), "Dark theme must contain single quote key ''tag''");

    // 3. Verify Bounding Boxes match coordinates within original unscaled bounds
    let (orig_w, orig_h) = (dark_img.width() as f64, dark_img.height() as f64);
    for line in &dark_res.lines {
        assert!(line.x >= 0.0 && line.x < orig_w, "Line x ({}) within image", line.x);
        assert!(line.y >= 0.0 && line.y < orig_h, "Line y ({}) within image", line.y);
        assert!(line.width > 0.0 && line.width <= orig_w, "Line width valid");
        assert!(line.height > 0.0 && line.height <= orig_h, "Line height valid");

        for word in &line.words {
            assert!(word.x >= 0.0 && word.x < orig_w, "Word x ({}) within image", word.x);
            assert!(word.y >= 0.0 && word.y < orig_h, "Word y ({}) within image", word.y);
            assert!(word.width > 0.0 && word.width <= orig_w, "Word width valid");
            assert!(word.height > 0.0 && word.height <= orig_h, "Word height valid");
        }
    }
}

#[test]
fn test_dark_theme_detection_and_inversion() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let dark_path = root.join("scripts").join("code_dark.png");
    let light_path = root.join("scripts").join("code_light.png");

    if dark_path.exists() && light_path.exists() {
        let dark_img = image::open(&dark_path).unwrap().to_rgba8();
        let light_img = image::open(&light_path).unwrap().to_rgba8();

        assert!(is_dark_image(&dark_img), "code_dark.png must be detected as dark theme");
        assert!(!is_dark_image(&light_img), "code_light.png must NOT be detected as dark theme");

        let inverted = invert_image(&dark_img);
        assert!(!is_dark_image(&inverted), "Inverted dark image must now be detected as light");

        let p1 = dark_img.get_pixel(10, 10);
        let p2 = inverted.get_pixel(10, 10);
        assert_eq!(p1[0], 255 - p2[0]);
        assert_eq!(p1[1], 255 - p2[1]);
        assert_eq!(p1[2], 255 - p2[2]);
        assert_eq!(p1[3], p2[3], "Alpha channel must be preserved");
    }
}

#[test]
fn test_normalize_code_punctuation_all_variants() {
    // Typographic double quotes
    assert_eq!(normalize_code_punctuation("“hello”"), "\"hello\"");
    assert_eq!(normalize_code_punctuation("„test“"), "\"test\"");
    assert_eq!(normalize_code_punctuation("«code»"), "\"code\"");

    // Typographic single quotes, accents
    assert_eq!(normalize_code_punctuation("‘param’"), "'param'");
    assert_eq!(normalize_code_punctuation("′val′"), "'val'");

    // Code backticks must be preserved as valid syntax
    assert_eq!(normalize_code_punctuation("`item`"), "`item`");

    // Typographic dashes / hyphens
    assert_eq!(normalize_code_punctuation("a — b"), "a - b");
    assert_eq!(normalize_code_punctuation("x – y"), "x - y");
    assert_eq!(normalize_code_punctuation("1 − 2"), "1 - 2");

    // Non-breaking spaces
    assert_eq!(normalize_code_punctuation("val\u{00A0}=\u{202F}10"), "val = 10");

    // Code characters must remain untouched
    let code_sample = "def calculate(x: int = 10) -> dict:\n    # comment\n    return {'k': (x * 2)}";
    assert_eq!(normalize_code_punctuation(code_sample), code_sample);
}

#[test]
fn test_coalesce_and_structure_lines_indentation_and_blank_lines() {
    // Construct synthetic lines simulating code OCR output:
    // Line 0: import os (x=20, y=20)
    // Line 1: import sys (x=20, y=50)
    // [Blank line gap: y jumps from 50 to 110]
    // Line 2: def process(data: list) -> bool: (x=20, y=110)
    // Line 3:     # Validate items (x=60, y=140) -> 4 spaces indent!
    // Line 4:     total = (count * 2) (x=60, y=170) -> 4 spaces indent!
    // Line 5:         return total > 0 (x=100, y=200) -> 8 spaces indent!

    let raw_lines = vec![
        OcrLine {
            text: "import os".to_string(),
            x: 20.0,
            y: 20.0,
            width: 90.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "import".to_string(), x: 20.0, y: 20.0, width: 60.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "os".to_string(), x: 90.0, y: 20.0, width: 20.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "import sys".to_string(),
            x: 20.0,
            y: 50.0,
            width: 100.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "import".to_string(), x: 20.0, y: 50.0, width: 60.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "sys".to_string(), x: 90.0, y: 50.0, width: 30.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "def process(data: list) -> bool:".to_string(),
            x: 20.0,
            y: 110.0,
            width: 320.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "def".to_string(), x: 20.0, y: 110.0, width: 30.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "process(data:".to_string(), x: 60.0, y: 110.0, width: 130.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "list)".to_string(), x: 200.0, y: 110.0, width: 50.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "->".to_string(), x: 260.0, y: 110.0, width: 20.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "bool:".to_string(), x: 290.0, y: 110.0, width: 50.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "# Validate items".to_string(),
            x: 60.0,
            y: 140.0,
            width: 160.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "#".to_string(), x: 60.0, y: 140.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "Validate".to_string(), x: 80.0, y: 140.0, width: 80.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "items".to_string(), x: 170.0, y: 140.0, width: 50.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "total = (count * 2)".to_string(),
            x: 60.0,
            y: 170.0,
            width: 190.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "total".to_string(), x: 60.0, y: 170.0, width: 50.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "=".to_string(), x: 120.0, y: 170.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "(count".to_string(), x: 140.0, y: 170.0, width: 60.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "*".to_string(), x: 210.0, y: 170.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "2)".to_string(), x: 230.0, y: 170.0, width: 20.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "return total > 0".to_string(),
            x: 100.0,
            y: 200.0,
            width: 160.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "return".to_string(), x: 100.0, y: 200.0, width: 60.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "total".to_string(), x: 170.0, y: 200.0, width: 50.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: ">".to_string(), x: 230.0, y: 200.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "0".to_string(), x: 250.0, y: 200.0, width: 10.0, height: 20.0, confidence: 100.0 },
            ],
        },
    ];

    let (lines, full_text, boxes) = coalesce_and_structure_lines(raw_lines);

    assert_eq!(lines.len(), 6);
    assert_eq!(boxes.len(), 6);

    // Verify indentation
    assert_eq!(lines[0].text, "import os");
    assert_eq!(lines[1].text, "import sys");
    assert!(lines[2].text.starts_with("def process("));
    assert!(lines[3].text.starts_with("    # Validate"), "Line 3 must have 4 spaces indent: '{}'", lines[3].text);
    assert!(lines[4].text.starts_with("    total ="), "Line 4 must have 4 spaces indent: '{}'", lines[4].text);
    assert!(lines[5].text.starts_with("        return"), "Line 5 must have 8 spaces indent: '{}'", lines[5].text);

    // Verify blank line in full_text between imports and def
    let full_lines: Vec<&str> = full_text.lines().collect();
    assert_eq!(full_lines[0], "import os");
    assert_eq!(full_lines[1], "import sys");
    assert_eq!(full_lines[2], "", "Line index 2 must be empty newline");
    assert!(full_lines[3].starts_with("def process("));
    assert!(full_lines[4].starts_with("    #"));
    assert!(full_lines[5].starts_with("    total"));
    assert!(full_lines[6].starts_with("        return"));
}

#[test]
fn test_coalesce_and_structure_lines_reorders_split_horizontal_fragments() {
    // Simulate OCR artifact where a single line `result = (a + b) * 2` was split into two
    // lines that overlap vertically, but where the right fragment was returned first:
    // Fragment 1 (right): `(a + b) * 2` (x=150, y=100)
    // Fragment 2 (left): `result =` (x=50, y=101)
    let raw_split_lines = vec![
        OcrLine {
            text: "(a + b) * 2".to_string(),
            x: 150.0,
            y: 100.0,
            width: 90.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "(a".to_string(), x: 150.0, y: 100.0, width: 20.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "+".to_string(), x: 175.0, y: 100.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "b)".to_string(), x: 190.0, y: 100.0, width: 20.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "*".to_string(), x: 215.0, y: 100.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "2".to_string(), x: 230.0, y: 100.0, width: 10.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "result =".to_string(),
            x: 50.0,
            y: 101.0,
            width: 80.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "result".to_string(), x: 50.0, y: 101.0, width: 50.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "=".to_string(), x: 110.0, y: 101.0, width: 10.0, height: 20.0, confidence: 100.0 },
            ],
        },
    ];

    let (merged_lines, full_text, _) = coalesce_and_structure_lines(raw_split_lines);

    assert_eq!(merged_lines.len(), 1, "Must merge horizontally split fragments into 1 line");
    assert!(
        merged_lines[0].text.starts_with("result ="),
        "Left fragment 'result =' must come first: '{}'",
        merged_lines[0].text
    );
    assert!(
        merged_lines[0].text.contains("(a + b) * 2"),
        "Must contain right fragment in correct order: '{}'",
        merged_lines[0].text
    );
    assert_eq!(full_text, merged_lines[0].text);
}

#[test]
fn test_normalize_fullwidth_syntax_and_special_symbols() {
    // Fullwidth colons, semicolons, brackets, parentheses, equals
    let input = "def calculate（x： int， y： int = 0） -> bool：";
    let normalized = normalize_code_punctuation(input);
    assert_eq!(normalized, "def calculate(x: int, y: int = 0) -> bool:");

    // Fullwidth braces, brackets, operators
    let braces = "items ＝ ［｛'id'： 1｝］；";
    assert_eq!(normalize_code_punctuation(braces), "items = [{'id': 1}];");

    // Arrows and comparisons
    assert_eq!(normalize_code_punctuation("fn -> val => out"), "fn -> val => out");
    assert_eq!(normalize_code_punctuation("fn → val ⇒ out"), "fn -> val => out");
    assert_eq!(normalize_code_punctuation("a ≤ b && c ≥ d && x ≠ y"), "a <= b && c >= d && x != y");

    // Typographic ellipsis
    assert_eq!(normalize_code_punctuation("def stub(…):"), "def stub(...):");

    // Zero-width spaces, joiners and BOMs must be stripped to prevent syntax errors
    let with_zero_width = "def\u{200B} my_function\u{FEFF}():\u{200C}\n    pass\u{200D}";
    assert_eq!(normalize_code_punctuation(with_zero_width), "def my_function():\n    pass");
}

#[test]
fn test_coalesce_and_structure_lines_2_space_indentation() {
    // Synthesize 2-space indented code (Rust / JS / YAML style):
    // Line 0: function run() { (x=20, y=20)
    // Line 1:   let a = 1; (x=40, y=50) -> 2 spaces
    // Line 2:     let b = 2; (x=60, y=80) -> 4 spaces
    // Line 3:       let c = 3; (x=80, y=110) -> 6 spaces
    let raw_lines = vec![
        OcrLine {
            text: "function run() {".to_string(),
            x: 20.0,
            y: 20.0,
            width: 140.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "function".to_string(), x: 20.0, y: 20.0, width: 70.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "run()".to_string(), x: 100.0, y: 20.0, width: 40.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "{".to_string(), x: 150.0, y: 20.0, width: 10.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "let a = 1;".to_string(),
            x: 40.0,
            y: 50.0,
            width: 90.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "let".to_string(), x: 40.0, y: 50.0, width: 25.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "a".to_string(), x: 75.0, y: 50.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "=".to_string(), x: 95.0, y: 50.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "1;".to_string(), x: 115.0, y: 50.0, width: 15.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "let b = 2;".to_string(),
            x: 60.0,
            y: 80.0,
            width: 90.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "let".to_string(), x: 60.0, y: 80.0, width: 25.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "b".to_string(), x: 95.0, y: 80.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "=".to_string(), x: 115.0, y: 80.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "2;".to_string(), x: 135.0, y: 80.0, width: 15.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "let c = 3;".to_string(),
            x: 80.0,
            y: 110.0,
            width: 90.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "let".to_string(), x: 80.0, y: 110.0, width: 25.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "c".to_string(), x: 115.0, y: 110.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "=".to_string(), x: 135.0, y: 110.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "3;".to_string(), x: 155.0, y: 110.0, width: 15.0, height: 20.0, confidence: 100.0 },
            ],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].text, "function run() {");
    assert!(lines[1].text.starts_with("  let a = 1;"), "Expected 2 spaces: '{}'", lines[1].text);
    assert!(lines[2].text.starts_with("    let b = 2;"), "Expected 4 spaces: '{}'", lines[2].text);
    assert!(lines[3].text.starts_with("      let c = 3;"), "Expected 6 spaces: '{}'", lines[3].text);
}

#[test]
fn test_coalesce_and_structure_lines_proportional_tolerance_snapping() {
    // Proportional font variance test:
    // In proportional fonts, 4 spaces may be slightly narrower than 4 * char_w (e.g. 3.48 * char_w).
    // The unit detection should snap 3.48 to 4 spaces instead of rounding down to 3 spaces.
    let _char_w = 10.0;
    let base_x = 20.0;
    let indent_4_spaces_px = 34.8; // 34.8 / 10.0 = 3.48

    let raw_lines = vec![
        OcrLine {
            text: "def calculate():".to_string(),
            x: base_x,
            y: 20.0,
            width: 140.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "def".to_string(), x: base_x, y: 20.0, width: 30.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "calculate():".to_string(), x: base_x + 40.0, y: 20.0, width: 100.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "return 42".to_string(),
            x: base_x + indent_4_spaces_px,
            y: 50.0,
            width: 80.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "return".to_string(), x: base_x + indent_4_spaces_px, y: 50.0, width: 50.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "42".to_string(), x: base_x + indent_4_spaces_px + 60.0, y: 50.0, width: 20.0, height: 20.0, confidence: 100.0 },
            ],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 2);
    assert!(
        lines[1].text.starts_with("    return"),
        "3.48 char_w ratio must snap to 4 spaces, not 3 spaces: '{}'",
        lines[1].text
    );
}

#[test]
fn test_coalesce_and_structure_lines_single_character_words() {
    // Test that crops with only 1-character words compute char_w and format correctly
    let raw_lines = vec![
        OcrLine {
            text: "x = 1".to_string(),
            x: 20.0,
            y: 20.0,
            width: 50.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "x".to_string(), x: 20.0, y: 20.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "=".to_string(), x: 40.0, y: 20.0, width: 10.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "1".to_string(), x: 60.0, y: 20.0, width: 10.0, height: 20.0, confidence: 100.0 },
            ],
        },
    ];

    let (lines, full_text, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "x = 1");
    assert_eq!(full_text, "x = 1");
}

#[test]
fn test_is_dark_image_ignores_transparent_pixels() {
    // Create an image that is 100% transparent with (0, 0, 0, 0)
    let transparent = image::RgbaImage::new(50, 50);
    assert!(
        !is_dark_image(&transparent),
        "100% transparent image must not be detected as dark theme"
    );
}

#[test]
fn test_coalesce_and_structure_lines_does_not_merge_tightly_spaced_distinct_lines() {
    // Two consecutive distinct code lines with tight line spacing (e.g. 55% vertical overlap)
    // Line 1: def calculate(): (x=20..150, y=100..120)
    // Line 2:     return 42 (x=40..140, y=109..129)
    let raw_lines = vec![
        OcrLine {
            text: "def calculate():".to_string(),
            x: 20.0,
            y: 100.0,
            width: 130.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "def".to_string(), x: 20.0, y: 100.0, width: 30.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "calculate():".to_string(), x: 60.0, y: 100.0, width: 90.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "return 42".to_string(),
            x: 50.0,
            y: 109.0,
            width: 80.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "return".to_string(), x: 50.0, y: 109.0, width: 50.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "42".to_string(), x: 110.0, y: 109.0, width: 20.0, height: 20.0, confidence: 100.0 },
            ],
        },
    ];

    let (lines, full_text, _) = coalesce_and_structure_lines(raw_lines);

    assert_eq!(lines.len(), 2, "Must NOT merge tightly-spaced consecutive code lines");
    assert_eq!(lines[0].text, "def calculate():");
    assert!(lines[1].text.contains("return 42"));
    assert!(full_text.contains("def calculate():\n"));
}

#[test]
fn test_normalize_code_punctuation_extended_symbols() {
    let input = "let x ＝ ～y ＆＆ *ptr ｜｜ val ／ 2 ％ 10 ＾ 0x01； // ＄var ＠annot `code` obj．field ＿priv";
    let normalized = normalize_code_punctuation(input);
    assert_eq!(
        normalized,
        "let x = ~y && *ptr || val / 2 % 10 ^ 0x01; // $var @annot `code` obj.field _priv"
    );
}

#[test]
fn test_parse_tesseract_tsv_scaled_down_to_screen_pixels() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t40\t60\t200\t40\t-1\t\n\
5\t1\t1\t1\t1\t1\t40\t60\t80\t40\t95\tdef\n\
5\t1\t1\t1\t1\t2\t140\t60\t100\t40\t95\tfoo():\n\
4\t1\t1\t1\t2\t0\t120\t120\t160\t40\t-1\t\n\
5\t1\t1\t1\t2\t1\t120\t120\t80\t40\t95\tpass\n";

    // Upscaled 2x image (inv_scale = 0.5)
    let res = parse_tesseract_tsv_scaled(tsv, 0.5);

    assert_eq!(res.lines.len(), 2);
    // Coordinates scaled by 0.5
    assert_eq!(res.lines[0].x, 20.0);
    assert_eq!(res.lines[0].y, 30.0);
    assert_eq!(res.lines[0].width, 100.0);
    assert_eq!(res.lines[0].height, 20.0);

    // Indentation preserved
    assert_eq!(res.lines[0].text, "def foo():");
    assert!(res.lines[1].text.starts_with("    pass"), "Line 1 should be indented 4 spaces: '{}'", res.lines[1].text);
}

#[test]
fn test_coalesce_and_structure_lines_three_fragments_out_of_order() {
    // A single line `def calculate(value: int) -> bool:` split into 3 fragments:
    // Fragment 1 (left): `def ` (x=20..50, y=100)
    // Fragment 3 (right): `-> bool:` (x=230..300, y=100)
    // Fragment 2 (middle): `calculate(value: int)` (x=60..220, y=100)
    // When returned out of order [Frag 1, Frag 3, Frag 2], all 3 MUST be merged into 1 line.
    let raw_fragments = vec![
        OcrLine {
            text: "def".to_string(),
            x: 20.0,
            y: 100.0,
            width: 30.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "def".to_string(), x: 20.0, y: 100.0, width: 30.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "-> bool:".to_string(),
            x: 230.0,
            y: 100.0,
            width: 70.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "->".to_string(), x: 230.0, y: 100.0, width: 20.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "bool:".to_string(), x: 260.0, y: 100.0, width: 40.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "calculate(value: int)".to_string(),
            x: 60.0,
            y: 100.0,
            width: 160.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "calculate(value:".to_string(), x: 60.0, y: 100.0, width: 110.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "int)".to_string(), x: 180.0, y: 100.0, width: 40.0, height: 20.0, confidence: 100.0 },
            ],
        },
    ];

    let (lines, full_text, _) = coalesce_and_structure_lines(raw_fragments);
    assert_eq!(lines.len(), 1, "All 3 out-of-order fragments must coalesce into 1 single line");
    assert_eq!(lines[0].text, "def calculate(value: int) -> bool:");
    assert_eq!(full_text, "def calculate(value: int) -> bool:");
}

#[test]
fn test_normalize_fullwidth_asian_code_and_ligatures() {
    // Fullwidth letters, numbers, spaces, operators from East Asian OCR models
    let asian_input = "ｄｅｆ　ｃａｌｃｕｌａｔｅ（ｘ：　ｉｎｔ　＝　１０）　⟶　ｂｏｏｌ：\n　　ｒｅｔｕｒｎ　ｘ　≡　１０　∧　ｙ　≢　２０；";
    let normalized = normalize_code_punctuation(asian_input);
    assert!(normalized.starts_with("def calculate(x: int = 10) -> bool:"));
    assert!(normalized.contains("return x === 10"));
    assert!(normalized.contains("y !== 20;"));
}

#[test]
fn test_coalesce_and_structure_lines_hanging_indentation_continuation() {
    // Hanging indent (continuation line aligned at column 10, e.g. 10 spaces in 10px font)
    // Line 0: def query(alpha: int, (x=20, char_w=10)
    // Line 1:           beta: int) -> None: (x=120 -> 100px / 10 = 10 spaces)
    // Must NOT be rounded up to 12 spaces by a 4-space tab grid!
    let raw_lines = vec![
        OcrLine {
            text: "def query(alpha: int,".to_string(),
            x: 20.0,
            y: 20.0,
            width: 200.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "def".to_string(), x: 20.0, y: 20.0, width: 30.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "query(alpha:".to_string(), x: 60.0, y: 20.0, width: 120.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "int,".to_string(), x: 190.0, y: 20.0, width: 40.0, height: 20.0, confidence: 100.0 },
            ],
        },
        OcrLine {
            text: "beta: int) -> None:".to_string(),
            x: 120.0,
            y: 50.0,
            width: 170.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "beta:".to_string(), x: 120.0, y: 50.0, width: 50.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "int)".to_string(), x: 180.0, y: 50.0, width: 40.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "->".to_string(), x: 230.0, y: 50.0, width: 20.0, height: 20.0, confidence: 100.0 },
                OcrWord { text: "None:".to_string(), x: 260.0, y: 50.0, width: 50.0, height: 20.0, confidence: 100.0 },
            ],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "def query(alpha: int,");
    assert_eq!(
        lines[1].text,
        "          beta: int) -> None:",
        "Hanging indent must be preserved with exactly 10 spaces, not forced to 12: '{}'",
        lines[1].text
    );
}

#[test]
fn test_segment_code_tokens_exact_coordinates_and_no_drift() {
    // 1. Keyword coalescing: 'e' + 'Ise:' -> 'else:'
    let words = vec![
        OcrWord { text: "e".to_string(), x: 100.0, y: 50.0, width: 8.0, height: 16.0, confidence: 90.0 },
        OcrWord { text: "Ise:".to_string(), x: 108.0, y: 50.0, width: 24.0, height: 16.0, confidence: 90.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].text, "else:");
    assert_eq!(res[0].x, 100.0);
    assert_eq!(res[0].width, 32.0);

    // 2. Glued keyword + identifier: 'fori' -> 'for', 'i'
    let words = vec![
        OcrWord { text: "fori".to_string(), x: 50.0, y: 20.0, width: 40.0, height: 16.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].text, "for");
    assert_eq!(res[1].text, "i");
    assert_eq!(res[0].x, 50.0);
    assert_eq!(res[0].width, 30.0); // 3 chars * 10.0
    assert_eq!(res[1].x, 80.0);    // 50.0 + 30.0
    assert_eq!(res[1].width, 10.0); // 1 char * 10.0
    assert!((res[0].width + res[1].width - 40.0).abs() < 1e-6);

    // 3. Glued operator expression: 'ifi%2' -> 'if', 'i', '%', '2'
    let words = vec![
        OcrWord { text: "ifi%2".to_string(), x: 50.0, y: 20.0, width: 50.0, height: 16.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 4);
    assert_eq!(res[0].text, "if");
    assert_eq!(res[1].text, "i");
    assert_eq!(res[2].text, "%");
    assert_eq!(res[3].text, "2");
    assert_eq!(res[0].x, 50.0);
    assert_eq!(res[0].width, 20.0);
    assert_eq!(res[1].x, 70.0);
    assert_eq!(res[1].width, 10.0);
    assert_eq!(res[2].x, 80.0);
    assert_eq!(res[2].width, 10.0);
    assert_eq!(res[3].x, 90.0);
    assert_eq!(res[3].width, 10.0);

    // 4. Glued operators and colons: '2==0:' -> '2', '==', '0', ':'
    let words = vec![
        OcrWord { text: "2==0:".to_string(), x: 100.0, y: 20.0, width: 50.0, height: 16.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 4);
    assert_eq!(res[0].text, "2");
    assert_eq!(res[1].text, "==");
    assert_eq!(res[2].text, "0");
    assert_eq!(res[3].text, ":");
    assert_eq!(res[0].x, 100.0);
    assert_eq!(res[0].width, 10.0);
    assert_eq!(res[1].x, 110.0);
    assert_eq!(res[1].width, 20.0);
    assert_eq!(res[2].x, 130.0);
    assert_eq!(res[2].width, 10.0);
    assert_eq!(res[3].x, 140.0);
    assert_eq!(res[3].width, 10.0);

    // 5. String literal prefix in function call: 'print(f"%i' -> 'print(f', '"%i'
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

    // 6. Coordinate invariant check: sum of subtoken widths equals original width, non-overlapping
    let total_width: f64 = res.iter().map(|w| w.width).sum();
    assert!((total_width - 100.0).abs() < 1e-6);
    assert_eq!(res[0].x + res[0].width, res[1].x);
}

#[test]
fn test_is_code_op_exhaustive() {
    let operators = [
        "===", "!==", ">>>",
        "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=", "&&", "||",
        "->", "=>", "::", "..", "<<", ">>", "++", "--", "**", "//",
        "=", "+", "-", "*", "/", "%", "<", ">", "!", "&", "|", "^",
        "~", ":", ";", "?", ".", ",", "(", ")", "[", "]", "{", "}",
    ];
    for op in &operators {
        assert!(is_code_op(op), "Operator '{}' must be recognized by is_code_op", op);
        assert!(is_code_op(&format!(" {} ", op)), "Padded operator ' {} ' must be recognized", op);
    }

    let non_operators = [
        "for", "else", "if", "def", "class", "while", "return", "import",
        "foo", "bar_baz", "var1", "123", "abc", "", "   ",
    ];
    for non_op in &non_operators {
        assert!(!is_code_op(non_op), "Non-operator '{}' must NOT be recognized by is_code_op", non_op);
    }
}

#[test]
fn test_is_code_keyword_exhaustive() {
    let keywords = &[
        "if", "if:", "else", "else:", "elif", "elif:", "while", "while:", "for", "for:",
        "def", "let", "var", "val", "fn", "in", "is", "try", "try:", "except", "with",
        "return", "class", "import", "from", "async", "await", "struct", "enum", "impl",
    ];
    for &kw in keywords {
        assert!(is_code_keyword(kw), "Keyword '{}' must be recognized by is_code_keyword", kw);
    }

    let non_keywords = &["variable", "total", "foo", "bar_baz", "123", "", "=="];
    for &non_kw in non_keywords {
        assert!(!is_code_keyword(non_kw), "Non-keyword '{}' must NOT be recognized by is_code_keyword", non_kw);
    }
}

#[test]
fn test_tesseract_tsv_preserves_code_operators_and_keywords() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t20\t20\t300\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t20\t20\t30\t20\t95\tfor\n\
5\t1\t1\t1\t1\t2\t60\t20\t10\t20\t95\ti\n\
5\t1\t1\t1\t1\t3\t80\t20\t20\t20\t95\tin\n\
5\t1\t1\t1\t1\t4\t110\t20\t90\t20\t95\trange(10):\n\
4\t1\t1\t1\t2\t0\t60\t50\t200\t20\t-1\t\n\
5\t1\t1\t1\t2\t1\t60\t50\t20\t20\t95\tif\n\
5\t1\t1\t1\t2\t2\t90\t50\t10\t20\t95\ti\n\
5\t1\t1\t1\t2\t3\t110\t50\t10\t20\t95\t%\n\
5\t1\t1\t1\t2\t4\t130\t50\t10\t20\t95\t2\n\
5\t1\t1\t1\t2\t5\t150\t50\t20\t20\t95\t==\n\
5\t1\t1\t1\t2\t6\t180\t50\t10\t20\t95\t0:\n\
4\t1\t1\t1\t3\t0\t20\t80\t50\t20\t-1\t\n\
5\t1\t1\t1\t3\t1\t20\t80\t45\t20\t95\telse:\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.lines.len(), 3, "Expected 3 lines parsed");

    assert!(res.full_text.contains("for i in range(10):"));
    assert!(res.full_text.contains("if i % 2 == 0:"));
    assert!(res.full_text.contains("else:"));

    let line0_words: Vec<&str> = res.lines[0].words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(line0_words, vec!["for", "i", "in", "range(10):"]);

    let line1_words: Vec<&str> = res.lines[1].words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(line1_words, vec!["if", "i", "%", "2", "==", "0", ":"]);

    let line2_words: Vec<&str> = res.lines[2].words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(line2_words, vec!["else:"]);
}

#[test]
fn test_code_ocr_loop_conditional_constructs() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let dark_path = root.join("scripts").join("code_loop_dark.png");
    let light_path = root.join("scripts").join("code_loop_light.png");
    if !dark_path.exists() || !light_path.exists() {
        return;
    }
    let dark_img = image::open(&dark_path).unwrap().to_rgba8();
    let light_img = image::open(&light_path).unwrap().to_rgba8();
    let dark_res = extract_text_native(&dark_img).unwrap();
    let light_res = extract_text_native(&light_img).unwrap();

    // Verify key code constructs
    assert!(dark_res.full_text.contains("for "), "Dark must contain 'for'");
    assert!(dark_res.full_text.contains("in "), "Dark must contain 'in'");
    assert!(dark_res.full_text.contains("range(10):"), "Dark must contain 'range(10):'");
    assert!(dark_res.full_text.contains("if "), "Dark must contain 'if'");
    assert!(dark_res.full_text.contains("%"), "Dark must contain '%'");
    assert!(dark_res.full_text.contains("else:"), "Dark must contain 'else:'");
    assert!(dark_res.full_text.contains("print("), "Dark must contain 'print('");
    assert!(dark_res.full_text.contains("%i is even"), "Dark must contain '%i is even'");
    assert!(dark_res.full_text.contains("%i is odd"), "Dark must contain '%i is odd'");

    assert!(light_res.full_text.contains("for "), "Light must contain 'for'");
    assert!(light_res.full_text.contains("in "), "Light must contain 'in'");
    assert!(light_res.full_text.contains("range(10):"), "Light must contain 'range(10):'");
    assert!(light_res.full_text.contains("if "), "Light must contain 'if'");
    assert!(light_res.full_text.contains("%"), "Light must contain '%'");
    assert!(light_res.full_text.contains("else:"), "Light must contain 'else:'");
    assert!(light_res.full_text.contains("print("), "Light must contain 'print('");
    assert!(light_res.full_text.contains("%i is even"), "Light must contain '%i is even'");
    assert!(light_res.full_text.contains("%i is odd"), "Light must contain '%i is odd'");

    // Verify fine-grained token segmentation across dark and light themes (operators, delimiters, variable names)
    let dark_words: Vec<&str> = dark_res.lines.iter().flat_map(|l| &l.words).map(|w| w.text.as_str()).collect();
    assert!(dark_words.contains(&"for"), "Dark words must contain 'for': {:?}", dark_words);
    assert!(dark_words.contains(&"if"), "Dark words must contain 'if': {:?}", dark_words);
    assert!(dark_words.contains(&"i"), "Dark words must contain identifier 'i': {:?}", dark_words);
    assert!(dark_words.contains(&"("), "Dark words must segment opening delimiter '(': {:?}", dark_words);
    assert!(dark_words.contains(&")"), "Dark words must segment closing delimiter ')': {:?}", dark_words);
    assert!(dark_words.contains(&":"), "Dark words must segment statement delimiter ':': {:?}", dark_words);
    assert!(dark_words.contains(&"%"), "Dark words must contain operator '%': {:?}", dark_words);
    assert!(dark_words.contains(&"else:"), "Dark words must contain keyword 'else:': {:?}", dark_words);

    let light_words: Vec<&str> = light_res.lines.iter().flat_map(|l| &l.words).map(|w| w.text.as_str()).collect();
    assert!(light_words.contains(&"for"), "Light words must contain 'for': {:?}", light_words);
    assert!(light_words.contains(&"if"), "Light words must contain 'if': {:?}", light_words);
    assert!(light_words.contains(&"i"), "Light words must contain identifier 'i': {:?}", light_words);
    assert!(light_words.contains(&"("), "Light words must segment opening delimiter '(': {:?}", light_words);
    assert!(light_words.contains(&")"), "Light words must segment closing delimiter ')': {:?}", light_words);
    assert!(light_words.contains(&":"), "Light words must segment statement delimiter ':': {:?}", light_words);
    assert!(light_words.contains(&"%"), "Light words must contain operator '%': {:?}", light_words);
    assert!(light_words.contains(&"else:"), "Light words must contain keyword 'else:': {:?}", light_words);

    // 3. Verify Bounding Boxes match coordinates within original unscaled bounds
    let (orig_w, orig_h) = (dark_img.width() as f64, dark_img.height() as f64);
    for line in &dark_res.lines {
        assert!(line.x >= 0.0 && line.x < orig_w, "Line x ({}) within image", line.x);
        assert!(line.y >= 0.0 && line.y < orig_h, "Line y ({}) within image", line.y);
        assert!(line.width > 0.0 && line.width <= orig_w, "Line width valid");
        assert!(line.height > 0.0 && line.height <= orig_h, "Line height valid");

        for word in &line.words {
            assert!(word.x >= 0.0 && word.x < orig_w, "Word x ({}) within image", word.x);
            assert!(word.y >= 0.0 && word.y < orig_h, "Word y ({}) within image", word.y);
            assert!(word.width > 0.0 && word.width <= orig_w, "Word width valid");
            assert!(word.height > 0.0 && word.height <= orig_h, "Word height valid");
        }
    }
}

#[test]
fn test_benchmark_crop_ocr() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let crop_path = root.join("scripts").join("benchmark_crop.png");
    if !crop_path.exists() {
        return;
    }
    let crop_img = image::open(&crop_path).expect("open benchmark_crop.png").to_rgba8();
    let res = extract_text_native(&crop_img).expect("extract benchmark crop text");

    // Verify that the benchmark crop tokens are recognized across syntax-highlighted lines and console output
    assert!(res.full_text.contains("for"), "Benchmark crop must contain 'for'");
    assert!(res.full_text.contains("range(10):"), "Benchmark crop must contain 'range(10):'");
    assert!(res.full_text.contains("else:"), "Benchmark crop must coalesce and contain 'else:'");
    assert!(res.full_text.contains("if"), "Benchmark crop must segment and contain 'if'");
    assert!(res.full_text.contains("print(f"), "Benchmark crop must contain 'print(f'");
    assert!(res.full_text.contains("0 is even"), "Benchmark crop must contain '0 is even'");
    assert!(res.full_text.contains("1 is odd"), "Benchmark crop must contain '1 is odd'");
    assert!(res.full_text.contains("2 is even"), "Benchmark crop must contain '2 is even'");
    assert!(res.full_text.contains("3 is odd"), "Benchmark crop must contain '3 is odd'");

    // Verify word-level token segmentation and coordinate integrity
    let all_words: Vec<&OcrWord> = res.lines.iter().flat_map(|l| &l.words).collect();
    let texts: Vec<&str> = all_words.iter().map(|w| w.text.as_str()).collect();
    assert!(texts.contains(&"for"), "Token 'for' must be present in words: {:?}", texts);
    assert!(texts.contains(&"if"), "Token 'if' must be present in words: {:?}", texts);
    assert!(texts.contains(&"i"), "Token 'i' must be present in words: {:?}", texts);
    assert!(texts.contains(&"%"), "Token '%' must be present in words: {:?}", texts);
    assert!(texts.contains(&"2"), "Token '2' must be present in words: {:?}", texts);
    assert!(texts.contains(&"else:"), "Token 'else:' must be present in words: {:?}", texts);
    assert!(texts.contains(&"0"), "Token '0' must be present in words: {:?}", texts);
    assert!(texts.contains(&"is"), "Token 'is' must be present in words: {:?}", texts);
    assert!(texts.contains(&"even"), "Token 'even' must be present in words: {:?}", texts);
    assert!(texts.contains(&"odd"), "Token 'odd' must be present in words: {:?}", texts);

    for w in &all_words {
        assert!(w.width > 0.0, "Word width must be positive: {:?}", w);
        assert!(w.height > 0.0, "Word height must be positive: {:?}", w);
        assert!(w.x >= 0.0, "Word x must be non-negative: {:?}", w);
        assert!(w.y >= 0.0, "Word y must be non-negative: {:?}", w);
    }
}

#[test]
fn test_segment_multibyte_identifiers_with_operators() {
    // 1. Accented identifier with double equals: 'ação==1'
    let words = vec![
        OcrWord { text: "ação==1".to_string(), x: 50.0, y: 20.0, width: 70.0, height: 16.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 3);
    assert_eq!(res[0].text, "ação");
    assert_eq!(res[1].text, "==");
    assert_eq!(res[2].text, "1");
    // Verify coordinate accuracy and no overlap
    assert_eq!(res[0].x, 50.0);
    assert_eq!(res[0].width, 40.0); // 4 chars * 10.0
    assert_eq!(res[1].x, 90.0);    // 2 chars * 10.0
    assert_eq!(res[1].width, 20.0);
    assert_eq!(res[2].x, 110.0);   // 1 char * 10.0
    assert_eq!(res[2].width, 10.0);
    let total_w: f64 = res.iter().map(|w| w.width).sum();
    assert!((total_w - 70.0).abs() < 1e-6);

    // 2. Accented identifier with inequality operator: 'número!=0'
    let words2 = vec![
        OcrWord { text: "número!=0".to_string(), x: 100.0, y: 40.0, width: 90.0, height: 18.0, confidence: 95.0 },
    ];
    let res2 = segment_code_tokens(words2);
    assert_eq!(res2.len(), 3);
    assert_eq!(res2[0].text, "número");
    assert_eq!(res2[1].text, "!=");
    assert_eq!(res2[2].text, "0");
    assert_eq!(res2[0].x, 100.0);
    assert_eq!(res2[0].width, 60.0); // 6 chars * 10.0
    assert_eq!(res2[1].x, 160.0);
    assert_eq!(res2[1].width, 20.0); // 2 chars * 10.0
    assert_eq!(res2[2].x, 180.0);
    assert_eq!(res2[2].width, 10.0); // 1 char * 10.0

    // 3. Accented identifier with comparison operator: 'índice<=10'
    let words3 = vec![
        OcrWord { text: "índice<=10".to_string(), x: 200.0, y: 60.0, width: 100.0, height: 18.0, confidence: 95.0 },
    ];
    let res3 = segment_code_tokens(words3);
    assert_eq!(res3.len(), 3);
    assert_eq!(res3[0].text, "índice");
    assert_eq!(res3[1].text, "<=");
    assert_eq!(res3[2].text, "10");
    assert_eq!(res3[0].x, 200.0);
    assert_eq!(res3[0].width, 60.0); // 6 chars * 10.0
    assert_eq!(res3[1].x, 260.0);
    assert_eq!(res3[1].width, 20.0); // 2 chars * 10.0
    assert_eq!(res3[2].x, 280.0);
    assert_eq!(res3[2].width, 20.0); // 2 chars * 10.0
}

#[test]
fn test_tesseract_tsv_preserves_narrow_code_keywords() {
    // Test that narrow keywords with aspect ratio > 2.8 are preserved and not dropped as paper margin artifacts.
    // e.g. 'if' (6x20 -> ratio 3.33), 'in' (6x20 -> ratio 3.33), 'if:' (8x24 -> ratio 3.0), 'else:' (10x30 -> ratio 3.0)
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t20\t20\t300\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t20\t20\t6\t20\t95\tif\n\
5\t1\t1\t1\t1\t2\t35\t20\t6\t20\t95\tin\n\
5\t1\t1\t1\t1\t3\t50\t20\t8\t24\t95\tif:\n\
5\t1\t1\t1\t1\t4\t70\t20\t10\t30\t95\telse:\n\
5\t1\t1\t1\t1\t5\t95\t20\t8\t24\t95\tfor\n\
5\t1\t1\t1\t1\t6\t115\t20\t8\t24\t95\tdef\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.lines.len(), 1);

    let words: Vec<&str> = res.lines[0].words.iter().map(|w| w.text.as_str()).collect();
    assert!(words.contains(&"if"), "Keyword 'if' must be preserved: {:?}", words);
    assert!(words.contains(&"in"), "Keyword 'in' must be preserved: {:?}", words);
    assert!(words.contains(&"if:"), "Keyword 'if:' must be preserved: {:?}", words);
    assert!(words.contains(&"else:"), "Keyword 'else:' must be preserved: {:?}", words);
    assert!(words.contains(&"for"), "Keyword 'for' must be preserved: {:?}", words);
    assert!(words.contains(&"def"), "Keyword 'def' must be preserved: {:?}", words);
}

#[test]
fn test_segment_code_tokens_parentheses_and_colons() {
    // 1. Opening parenthesis glued to identifier: '(i'
    let words = vec![
        OcrWord { text: "(i".to_string(), x: 20.0, y: 10.0, width: 20.0, height: 16.0, confidence: 95.0 },
    ];
    let res = segment_code_tokens(words);
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].text, "(");
    assert_eq!(res[1].text, "i");
    assert_eq!(res[0].x, 20.0);
    assert_eq!(res[0].width, 10.0);
    assert_eq!(res[1].x, 30.0);
    assert_eq!(res[1].width, 10.0);

    // 2. Unmatched closing parenthesis and colon: '0):'
    let words2 = vec![
        OcrWord { text: "0):".to_string(), x: 100.0, y: 10.0, width: 30.0, height: 16.0, confidence: 95.0 },
    ];
    let res2 = segment_code_tokens(words2);
    assert_eq!(res2.len(), 3);
    assert_eq!(res2[0].text, "0");
    assert_eq!(res2[1].text, ")");
    assert_eq!(res2[2].text, ":");
    assert_eq!(res2[0].x, 100.0);
    assert_eq!(res2[0].width, 10.0);
    assert_eq!(res2[1].x, 110.0);
    assert_eq!(res2[1].width, 10.0);
    assert_eq!(res2[2].x, 120.0);
    assert_eq!(res2[2].width, 10.0);

    // 3. Function call with argument list like 'range(10):' must remain together
    let words3 = vec![
        OcrWord { text: "range(10):".to_string(), x: 200.0, y: 10.0, width: 90.0, height: 16.0, confidence: 95.0 },
    ];
    let res3 = segment_code_tokens(words3);
    assert_eq!(res3.len(), 1);
    assert_eq!(res3[0].text, "range(10):");
}


