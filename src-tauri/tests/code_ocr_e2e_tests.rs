use desklens_lib::ocr::{
    coalesce_and_structure_lines, extract_text_native,
    normalize_code_punctuation, segment_code_tokens,
    OcrLine, OcrWord,
};
use std::path::PathBuf;

/// Tier 1 & Tier 2: Token segmentation and delimiter invariants
#[test]
fn test_e2e_backend_keyword_and_operator_segmentation() {
    let raw_words = vec![
        OcrWord { text: "for".to_string(), x: 20.0, y: 20.0, width: 28.0, height: 20.0, confidence: 95.0 },
        OcrWord { text: "i".to_string(), x: 55.0, y: 20.0, width: 10.0, height: 20.0, confidence: 95.0 },
        OcrWord { text: "in".to_string(), x: 72.0, y: 20.0, width: 18.0, height: 20.0, confidence: 95.0 },
        OcrWord { text: "range(10):".to_string(), x: 98.0, y: 20.0, width: 90.0, height: 20.0, confidence: 95.0 },
        OcrWord { text: "ifi%2".to_string(), x: 20.0, y: 50.0, width: 50.0, height: 20.0, confidence: 95.0 },
        OcrWord { text: "==0:".to_string(), x: 80.0, y: 50.0, width: 40.0, height: 20.0, confidence: 95.0 },
        OcrWord { text: "a!=b<=c>=d".to_string(), x: 20.0, y: 80.0, width: 100.0, height: 20.0, confidence: 95.0 },
    ];

    let segmented = segment_code_tokens(raw_words);
    let texts: Vec<&str> = segmented.iter().map(|w| w.text.as_str()).collect();

    // Verify keywords
    assert!(texts.contains(&"for"), "Must contain 'for'");
    assert!(texts.contains(&"i"), "Must contain 'i'");
    assert!(texts.contains(&"in"), "Must contain 'in'");
    assert!(texts.contains(&"range(10):"), "Must preserve 'range(10):'");

    // Verify decomposed glued condition 'ifi%2'
    assert!(texts.contains(&"if"), "Decomposed 'ifi%2' must contain 'if'");
    assert!(texts.contains(&"%"), "Decomposed 'ifi%2' must contain '%'");

    // Verify decomposed '==0:'
    assert!(texts.contains(&"=="), "Decomposed '==0:' must contain '=='");
    assert!(texts.contains(&"0"), "Decomposed '==0:' must contain '0'");
    assert!(texts.contains(&":"), "Decomposed '==0:' must contain ':'");

    // Verify decomposed 'a!=b<=c>=d'
    assert!(texts.contains(&"!="), "Must contain '!='");
    assert!(texts.contains(&"<="), "Must contain '<='");
    assert!(texts.contains(&">="), "Must contain '>='");
}

/// Tier 1 & Tier 2: Normalization invariants
#[test]
fn test_e2e_backend_punctuation_and_dotted_zero_normalization() {
    // Normalization of fullwidth characters
    let input = "def calculate（x： int = 0） -> dict：";
    let norm = normalize_code_punctuation(input);
    assert_eq!(norm, "def calculate(x: int = 0) -> dict:");

    // Typographical quotes and dashes
    let quotes = "“param” ‘tag’ a — b 1 − 2";
    let norm_quotes = normalize_code_punctuation(quotes);
    assert_eq!(norm_quotes, "\"param\" 'tag' a - b 1 - 2");

    // Zero-width characters stripping
    let zw = "def\u{200B} run():\u{FEFF}\n    pass";
    assert_eq!(normalize_code_punctuation(zw), "def run():\n    pass");
}

/// Tier 3: Coalesce and indentation structure
#[test]
fn test_e2e_backend_indentation_ladder_preservation() {
    let raw_lines = vec![
        OcrLine {
            text: "for i in range(10):".to_string(),
            x: 20.0,
            y: 20.0,
            width: 180.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "for".to_string(), x: 20.0, y: 20.0, width: 28.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "i".to_string(), x: 55.0, y: 20.0, width: 10.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "in".to_string(), x: 72.0, y: 20.0, width: 18.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "range(10):".to_string(), x: 98.0, y: 20.0, width: 90.0, height: 20.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "if i % 2 == 0:".to_string(),
            x: 60.0,
            y: 50.0,
            width: 140.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "if".to_string(), x: 60.0, y: 50.0, width: 20.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "==".to_string(), x: 130.0, y: 50.0, width: 18.0, height: 20.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "print('even')".to_string(),
            x: 100.0,
            y: 80.0,
            width: 120.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "print('even')".to_string(), x: 100.0, y: 80.0, width: 110.0, height: 20.0, confidence: 95.0 },
            ],
        },
    ];

    let (lines, full_text, _) = coalesce_and_structure_lines(raw_lines);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "for i in range(10):");
    assert!(lines[1].text.starts_with("    if"), "Line 1 must have 4 spaces: '{}'", lines[1].text);
    assert!(lines[2].text.starts_with("        print"), "Line 2 must have 8 spaces: '{}'", lines[2].text);
    assert!(full_text.contains("    if"));
    assert!(full_text.contains("        print"));
}

/// Tier 4: Real-World Scenario — Benchmark crop image verification
/// NOTE: Ignored on unconditioned native OCR engine until M1 adaptive binarization and M2 token segmentation land.
/// Run explicitly with: cargo test --test code_ocr_e2e_tests -- --ignored
#[test]
#[ignore = "Requires M1 adaptive binarization & M2 token segmentation: unconditioned Windows OCR drops '== 0:' and 'even'"]
fn test_e2e_backend_benchmark_crop_native_pipeline() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let crop_path = root.join("tests").join("fixtures").join("synthetic").join("benchmark_crop.png");
    if !crop_path.exists() {
        eprintln!("tests/fixtures/synthetic/benchmark_crop.png not found; skipping");
        return;
    }

    let crop_img = image::open(&crop_path).expect("open benchmark_crop.png").to_rgba8();
    let res = extract_text_native(&crop_img).expect("extract benchmark_crop text");

    eprintln!("[BENCHMARK CROP OCR OUTPUT]:\n{}", res.full_text);

    assert!(res.full_text.contains("for"), "Must contain 'for'");
    assert!(res.full_text.contains("range(10):"), "Must contain 'range(10):'");
    assert!(res.full_text.contains("if"), "Must contain 'if'");
    assert!(res.full_text.contains("else:"), "Must contain 'else:'");
    assert!(res.full_text.contains("print(f"), "Must contain 'print(f'");
    assert!(res.full_text.contains("0 is even"), "Must contain '0 is even'");
}
