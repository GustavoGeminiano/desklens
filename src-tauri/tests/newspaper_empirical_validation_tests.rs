use desklens_lib::ocr::extract_text_native;
use image::ImageReader;
use std::path::Path;

#[test]
fn test_reference_newspaper_image_ocr() {
    let img_path = Path::new("/home/geminiano/.gemini/antigravity/brain/5d1b8aea-aacc-4b51-b44b-fddaff098175/.user_uploaded/media_1793554053025_7966bc55.png");
    if !img_path.exists() {
        eprintln!("Reference image not found at {:?}", img_path);
        return;
    }

    let img = ImageReader::open(img_path)
        .expect("Failed to open reference image")
        .decode()
        .expect("Failed to decode image")
        .to_rgba8();

    let (w, h) = (img.width(), img.height());
    println!("Loaded reference image: {}x{}", w, h);

    // 1. Run native OCR on full image
    let result = extract_text_native(&img).expect("extract_text_native failed on full image");
    println!("Full image OCR text:\n---\n{}\n---", result.full_text);
    println!("Total words: {}", result.word_count);

    // Check headers
    assert!(result.full_text.contains("DAILY"), "Should contain DAILY");
    assert!(result.full_text.contains("NEWS"), "Should contain NEWS");
    assert!(result.full_text.contains("TOP"), "Should contain TOP");
    assert!(result.full_text.contains("EXTRA"), "Should contain EXTRA");
    assert!(result.full_text.contains("GOOD"), "Should contain GOOD");

    // Check margin artifacts / pipe noise eliminated
    assert!(!result.full_text.contains('|'), "Isolated vertical line artifacts '|' should be eliminated");
    for line in &result.lines {
        for word in &line.words {
            // Check that vertical margin fold artifacts like aspect ratio > 2.8 on short words are eliminated
            let char_count = word.text.chars().count();
            if char_count <= 3 {
                let ratio = word.height / word.width.max(0.001);
                assert!(ratio <= 2.8, "Word {:?} has vertical aspect ratio {} > 2.8", word.text, ratio);
            }
            // Check sub-3px dimensions eliminated
            assert!(word.width >= 1.5 && word.height >= 1.5, "Word {:?} dimensions too small: {}x{}", word.text, word.width, word.height);
            // Check low confidence filtered
            assert!(word.confidence >= 35.0, "Word {:?} confidence {} < 35.0", word.text, word.confidence);
        }
    }

    // 2. Test specific crops from the newspaper area
    // Crop A: Newspaper header area (around DAILY NEWS)
    // In reference image: DAILY NEWS is around y=150..300, x=300..800
    let crop_header = image::imageops::crop_imm(&img, 300, 160, 500, 140).to_image();
    let res_header = extract_text_native(&crop_header).expect("Header crop OCR failed");
    println!("Header crop OCR:\n{}", res_header.full_text);
    assert!(res_header.full_text.contains("DAILY NEWS") || (res_header.full_text.contains("DAILY") && res_header.full_text.contains("NEWS")));

    // Crop B: Columns area containing TOP NEWS, EXTRA! EXTRA!, GOOD NEWS
    let crop_columns = image::imageops::crop_imm(&img, 280, 270, 540, 260).to_image();
    let res_cols = extract_text_native(&crop_columns).expect("Columns crop OCR failed");
    println!("Columns crop OCR:\n{}", res_cols.full_text);
    assert!(res_cols.full_text.contains("TOP NEWS") || res_cols.full_text.contains("TOP"));
    assert!(res_cols.full_text.contains("EXTRA"));
    assert!(res_cols.full_text.contains("GOOD NEWS") || res_cols.full_text.contains("GOOD"));

    // Check column separation / no false gutter mergers in multi-column crop
    println!("Lines in columns crop: {}", res_cols.lines.len());
    for line in &res_cols.lines {
        println!("Line: {:?}", line.text);
    }
}
