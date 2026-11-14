use desklens_lib::lens_ocr::extract_text_lens;
use desklens_lib::ocr::{OcrLine, OcrResult, OcrWord};
use desklens_lib::ocr_service::format_ocr_document;
use image::RgbaImage;

#[test]
fn test_lens_ocr_empty_or_zero_dimension_image() {
    let empty_img = RgbaImage::new(0, 0);
    let result = extract_text_lens(&empty_img, "auto").expect("Should handle 0x0 cleanly");
    assert_eq!(result.full_text, "");
    assert_eq!(result.lines.len(), 0);
    assert_eq!(result.word_count, 0);
}

#[test]
fn test_format_ocr_document_metadata_and_utf16() {
    let line1 = OcrLine {
        text: "Daily News".to_string(),
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 30.0,
        words: vec![
            OcrWord {
                text: "Daily".to_string(),
                x: 10.0,
                y: 20.0,
                width: 45.0,
                height: 30.0,
                confidence: 99.0,
            },
            OcrWord {
                text: "News".to_string(),
                x: 60.0,
                y: 20.0,
                width: 50.0,
                height: 30.0,
                confidence: 99.0,
            },
        ],
    };

    let ocr_res = OcrResult {
        full_text: "Daily News".to_string(),
        lines: vec![line1],
        word_count: 2,
        boxes: vec![],
        paragraphs: vec![],
    };

    let doc = format_ocr_document(ocr_res, "google-lens").expect("format succeeds");
    assert_eq!(doc["engine"], "google-lens");
    assert_eq!(doc["full_text"], "Daily News");
    assert_eq!(doc["lines"][0]["start_utf16"], 0);
    assert_eq!(doc["lines"][0]["end_utf16"], 10);
    assert_eq!(doc["lines"][0]["words"][0]["start_utf16"], 0);
    assert_eq!(doc["lines"][0]["words"][0]["end_utf16"], 5);
    assert_eq!(doc["lines"][0]["words"][1]["start_utf16"], 6);
    assert_eq!(doc["lines"][0]["words"][1]["end_utf16"], 10);
}

#[test]
fn test_google_lens_native_separators_and_exact_spacing() {
    let asimov_path = std::path::Path::new("../tests/fixtures/real_editor_asimov_crop.png");
    if !asimov_path.exists() {
        eprintln!("Skipping test: fixture not found at {:?}", asimov_path);
        return;
    }

    let img = match image::open(asimov_path) {
        Ok(dyn_img) => dyn_img.to_rgba8(),
        Err(e) => {
            eprintln!("Skipping test: could not open image: {}", e);
            return;
        }
    };

    match extract_text_lens(&img, "code") {
        Ok(result) => {
            println!("=== NATIVE SPACING FULL TEXT ===\n{}", result.full_text);

            // Verify words that should have spaces
            assert!(result.full_text.contains("Copy Text"), "Should keep space in 'Copy Text'");
            assert!(result.full_text.contains("Search Image"), "Should keep space in 'Search Image'");
            assert!(result.full_text.contains("for i in"), "Should keep space in 'for i in'");
            assert!(result.full_text.contains("if i % 2 == 0:"), "Should format 'if i % 2 == 0:' with exact spaces");

            // Verify symbols that should NOT have fake spaces
            assert!(result.full_text.contains("range(10):"), "Should NOT have spaces inside 'range(10):'");
            assert!(!result.full_text.contains("range ( 10 )"), "Must not have fake spaces inside range");
            assert!(!result.full_text.contains("CopyText"), "Must not drop space in 'Copy Text'");
        }
        Err(e) => {
            println!("Google Lens request unavailable in this run: {}", e);
        }
    }
}

#[test]
fn test_colored_code_and_pastel_bullets_recovery() {
    let rapid_path = std::path::Path::new("../tests/fixtures/real_low_contrast_rapid.png");
    if rapid_path.exists() {
        let img = image::open(rapid_path).unwrap().to_rgba8();
        if let Ok(res) = extract_text_lens(&img, "auto") {
            println!("=== RAPID FULL TEXT ===\n{}", res.full_text);
            assert!(res.full_text.contains("- Rapid"), "Should recover '- Rapid' from iso-luminant pink-on-salmon image");
        }
    }

    let clean_monokai_path = std::path::Path::new("../tests/fixtures/clean_monokai.png");
    if clean_monokai_path.exists() {
        let img = image::open(clean_monokai_path).unwrap().to_rgba8();
        if let Ok(res) = extract_text_lens(&img, "code") {
            println!("=== CLEAN_MONOKAI FULL TEXT ===\n{}", res.full_text);
            assert!(res.full_text.contains("mensaje = raw_input"), "Should recover 'mensaje = raw_input'");
            assert!(res.full_text.contains("li=y+index"), "Should recover 'li=y+index'");
            assert!(res.full_text.contains("encoding =encoding+abc[li]"), "Should recover 'encoding =encoding+abc[li]'");
        }
    }

    let code1_path = std::path::Path::new("../tests/fixtures/real_code_monokai_crop.png");
    if code1_path.exists() {
        let img = image::open(code1_path).unwrap().to_rgba8();
        if let Ok(res) = extract_text_lens(&img, "code") {
            println!("=== CODE1 FULL TEXT ===\n{}", res.full_text);
            assert!(res.full_text.contains("mensaje = raw_input"), "Should recover '=' in 'mensaje = raw_input'");
            assert!(res.full_text.contains("clave = raw_input"), "Should recover '=' in 'clave = raw_input'");
            assert!(res.full_text.contains("posicion=0"), "Should recover 'posicion=0'");
            assert!(res.full_text.contains("ab=clave[0]"), "Should recover '=' in 'ab=clave[0]'");
            assert!(res.full_text.contains("index = abc.index(ab)"), "Should recover '=' in 'index = abc.index(ab)'");
            assert!(res.full_text.contains("encoding= \"\""), "Should recover 'encoding= \"\"'");
            assert!(res.full_text.contains("suma = 0"), "Should recover 'suma = 0'");
            assert!(res.full_text.contains("espacios=\"\""), "Should recover 'espacios=\"\"'");
            assert!(res.full_text.contains("espacios=espacio(mensaje)"), "Should recover 'espacios=espacio(mensaje)'");
            assert!(res.full_text.contains("if x==abc[y]"), "Should recover '==' in 'if x==abc[y]'");
        }
    }

    let code2_path = std::path::Path::new("../tests/fixtures/real_code_ip_crop.png");
    if code2_path.exists() {
        let img = image::open(code2_path).unwrap().to_rgba8();
        if let Ok(res) = extract_text_lens(&img, "code") {
            println!("=== CODE2 FULL TEXT ===\n{}", res.full_text);
            assert!(res.full_text.contains("ip = raw_input"), "Should recover '=' in 'ip = raw_input'");
            assert!(res.full_text.contains("mask = raw_input"), "Should keep '=' in 'mask = raw_input'");
            assert!(res.full_text.contains("broadcast, ip_rede = []"), "Should recover '=' in 'broadcast, ip_rede = []'");
        }
    }

    let canva_path = std::path::Path::new("../tests/fixtures/real_canva_pastel_crop.png");
    if canva_path.exists() {
        let img = image::open(canva_path).unwrap().to_rgba8();
        if let Ok(res) = extract_text_lens(&img, "auto") {
            println!("=== CANVA FULL TEXT ===\n{}", res.full_text);
            assert!(res.full_text.contains("- Precision cooking"), "Should recover '-' bullet before 'Precision cooking'");
            assert!(res.full_text.contains("assistants -> suggest"), "Should recover '->' arrow after 'assistants'");
            assert!(res.full_text.contains("- 3D food printing -> custom"), "Should recover '-' bullet and '->' arrow on 3D food printing line");
            assert!(res.full_text.contains("- Robotics -> chop"), "Should keep '-' bullet and '->' arrow on Robotics line");
        }
    }
}
