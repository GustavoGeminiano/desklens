use desklens_lib::ocr::{
    coalesce_and_structure_lines, extract_text_native,
    is_dark_image, invert_image, normalize_code_punctuation,
    parse_tesseract_tsv_scaled,
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
    println!("=== LIGHT THEME RESULT ===\n{}", light_res.full_text);

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
    println!("=== DARK THEME RESULT ===\n{}", dark_res.full_text);

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
                OcrWord { text: "import".to_string(), x: 20.0, y: 20.0, width: 60.0, height: 20.0 },
                OcrWord { text: "os".to_string(), x: 90.0, y: 20.0, width: 20.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "import sys".to_string(),
            x: 20.0,
            y: 50.0,
            width: 100.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "import".to_string(), x: 20.0, y: 50.0, width: 60.0, height: 20.0 },
                OcrWord { text: "sys".to_string(), x: 90.0, y: 50.0, width: 30.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "def process(data: list) -> bool:".to_string(),
            x: 20.0,
            y: 110.0,
            width: 320.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "def".to_string(), x: 20.0, y: 110.0, width: 30.0, height: 20.0 },
                OcrWord { text: "process(data:".to_string(), x: 60.0, y: 110.0, width: 130.0, height: 20.0 },
                OcrWord { text: "list)".to_string(), x: 200.0, y: 110.0, width: 50.0, height: 20.0 },
                OcrWord { text: "->".to_string(), x: 260.0, y: 110.0, width: 20.0, height: 20.0 },
                OcrWord { text: "bool:".to_string(), x: 290.0, y: 110.0, width: 50.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "# Validate items".to_string(),
            x: 60.0,
            y: 140.0,
            width: 160.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "#".to_string(), x: 60.0, y: 140.0, width: 10.0, height: 20.0 },
                OcrWord { text: "Validate".to_string(), x: 80.0, y: 140.0, width: 80.0, height: 20.0 },
                OcrWord { text: "items".to_string(), x: 170.0, y: 140.0, width: 50.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "total = (count * 2)".to_string(),
            x: 60.0,
            y: 170.0,
            width: 190.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "total".to_string(), x: 60.0, y: 170.0, width: 50.0, height: 20.0 },
                OcrWord { text: "=".to_string(), x: 120.0, y: 170.0, width: 10.0, height: 20.0 },
                OcrWord { text: "(count".to_string(), x: 140.0, y: 170.0, width: 60.0, height: 20.0 },
                OcrWord { text: "*".to_string(), x: 210.0, y: 170.0, width: 10.0, height: 20.0 },
                OcrWord { text: "2)".to_string(), x: 230.0, y: 170.0, width: 20.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "return total > 0".to_string(),
            x: 100.0,
            y: 200.0,
            width: 160.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "return".to_string(), x: 100.0, y: 200.0, width: 60.0, height: 20.0 },
                OcrWord { text: "total".to_string(), x: 170.0, y: 200.0, width: 50.0, height: 20.0 },
                OcrWord { text: ">".to_string(), x: 230.0, y: 200.0, width: 10.0, height: 20.0 },
                OcrWord { text: "0".to_string(), x: 250.0, y: 200.0, width: 10.0, height: 20.0 },
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
    // Fragment 1 (right): `(a + b) * 2` (x=200, y=100)
    // Fragment 2 (left): `result =` (x=50, y=101)
    let raw_split_lines = vec![
        OcrLine {
            text: "(a + b) * 2".to_string(),
            x: 200.0,
            y: 100.0,
            width: 100.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "(a".to_string(), x: 200.0, y: 100.0, width: 20.0, height: 20.0 },
                OcrWord { text: "+".to_string(), x: 230.0, y: 100.0, width: 10.0, height: 20.0 },
                OcrWord { text: "b)".to_string(), x: 250.0, y: 100.0, width: 20.0, height: 20.0 },
                OcrWord { text: "*".to_string(), x: 280.0, y: 100.0, width: 10.0, height: 20.0 },
                OcrWord { text: "2".to_string(), x: 300.0, y: 100.0, width: 10.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "result =".to_string(),
            x: 50.0,
            y: 101.0,
            width: 80.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "result".to_string(), x: 50.0, y: 101.0, width: 50.0, height: 20.0 },
                OcrWord { text: "=".to_string(), x: 110.0, y: 101.0, width: 10.0, height: 20.0 },
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
                OcrWord { text: "function".to_string(), x: 20.0, y: 20.0, width: 70.0, height: 20.0 },
                OcrWord { text: "run()".to_string(), x: 100.0, y: 20.0, width: 40.0, height: 20.0 },
                OcrWord { text: "{".to_string(), x: 150.0, y: 20.0, width: 10.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "let a = 1;".to_string(),
            x: 40.0,
            y: 50.0,
            width: 90.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "let".to_string(), x: 40.0, y: 50.0, width: 25.0, height: 20.0 },
                OcrWord { text: "a".to_string(), x: 75.0, y: 50.0, width: 10.0, height: 20.0 },
                OcrWord { text: "=".to_string(), x: 95.0, y: 50.0, width: 10.0, height: 20.0 },
                OcrWord { text: "1;".to_string(), x: 115.0, y: 50.0, width: 15.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "let b = 2;".to_string(),
            x: 60.0,
            y: 80.0,
            width: 90.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "let".to_string(), x: 60.0, y: 80.0, width: 25.0, height: 20.0 },
                OcrWord { text: "b".to_string(), x: 95.0, y: 80.0, width: 10.0, height: 20.0 },
                OcrWord { text: "=".to_string(), x: 115.0, y: 80.0, width: 10.0, height: 20.0 },
                OcrWord { text: "2;".to_string(), x: 135.0, y: 80.0, width: 15.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "let c = 3;".to_string(),
            x: 80.0,
            y: 110.0,
            width: 90.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "let".to_string(), x: 80.0, y: 110.0, width: 25.0, height: 20.0 },
                OcrWord { text: "c".to_string(), x: 115.0, y: 110.0, width: 10.0, height: 20.0 },
                OcrWord { text: "=".to_string(), x: 135.0, y: 110.0, width: 10.0, height: 20.0 },
                OcrWord { text: "3;".to_string(), x: 155.0, y: 110.0, width: 15.0, height: 20.0 },
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
                OcrWord { text: "def".to_string(), x: base_x, y: 20.0, width: 30.0, height: 20.0 },
                OcrWord { text: "calculate():".to_string(), x: base_x + 40.0, y: 20.0, width: 100.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "return 42".to_string(),
            x: base_x + indent_4_spaces_px,
            y: 50.0,
            width: 80.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "return".to_string(), x: base_x + indent_4_spaces_px, y: 50.0, width: 50.0, height: 20.0 },
                OcrWord { text: "42".to_string(), x: base_x + indent_4_spaces_px + 60.0, y: 50.0, width: 20.0, height: 20.0 },
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
                OcrWord { text: "x".to_string(), x: 20.0, y: 20.0, width: 10.0, height: 20.0 },
                OcrWord { text: "=".to_string(), x: 40.0, y: 20.0, width: 10.0, height: 20.0 },
                OcrWord { text: "1".to_string(), x: 60.0, y: 20.0, width: 10.0, height: 20.0 },
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
                OcrWord { text: "def".to_string(), x: 20.0, y: 100.0, width: 30.0, height: 20.0 },
                OcrWord { text: "calculate():".to_string(), x: 60.0, y: 100.0, width: 90.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "return 42".to_string(),
            x: 50.0,
            y: 109.0,
            width: 80.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "return".to_string(), x: 50.0, y: 109.0, width: 50.0, height: 20.0 },
                OcrWord { text: "42".to_string(), x: 110.0, y: 109.0, width: 20.0, height: 20.0 },
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
                OcrWord { text: "def".to_string(), x: 20.0, y: 100.0, width: 30.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "-> bool:".to_string(),
            x: 230.0,
            y: 100.0,
            width: 70.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "->".to_string(), x: 230.0, y: 100.0, width: 20.0, height: 20.0 },
                OcrWord { text: "bool:".to_string(), x: 260.0, y: 100.0, width: 40.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "calculate(value: int)".to_string(),
            x: 60.0,
            y: 100.0,
            width: 160.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "calculate(value:".to_string(), x: 60.0, y: 100.0, width: 110.0, height: 20.0 },
                OcrWord { text: "int)".to_string(), x: 180.0, y: 100.0, width: 40.0, height: 20.0 },
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
                OcrWord { text: "def".to_string(), x: 20.0, y: 20.0, width: 30.0, height: 20.0 },
                OcrWord { text: "query(alpha:".to_string(), x: 60.0, y: 20.0, width: 120.0, height: 20.0 },
                OcrWord { text: "int,".to_string(), x: 190.0, y: 20.0, width: 40.0, height: 20.0 },
            ],
        },
        OcrLine {
            text: "beta: int) -> None:".to_string(),
            x: 120.0,
            y: 50.0,
            width: 170.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "beta:".to_string(), x: 120.0, y: 50.0, width: 50.0, height: 20.0 },
                OcrWord { text: "int)".to_string(), x: 180.0, y: 50.0, width: 40.0, height: 20.0 },
                OcrWord { text: "->".to_string(), x: 230.0, y: 50.0, width: 20.0, height: 20.0 },
                OcrWord { text: "None:".to_string(), x: 260.0, y: 50.0, width: 50.0, height: 20.0 },
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

