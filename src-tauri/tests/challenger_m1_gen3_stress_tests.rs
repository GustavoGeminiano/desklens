//! Adversarial Empirical Stress Test Suite for Milestone 1 (Gen 3)
//! Preprocessing, Guide Line Suppression, and Dynamic Line Segmentation in `src-tauri/src/ocr.rs`

use desklens_lib::ocr::*;
use image::{Rgba, RgbaImage};
use std::path::PathBuf;

fn get_fixtures_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let p = manifest_dir.parent().unwrap().join("tests").join("fixtures");
    if p.exists() {
        p
    } else {
        manifest_dir.join("tests").join("fixtures")
    }
}

// ============================================================================
// 1. EMPIRICAL REAL-WORLD CROP VERIFICATION: real_editor_asimov_crop.png
// ============================================================================

#[test]
fn test_empirical_asimov_crop_line_bands_and_no_cut_characters() {
    let fixtures = get_fixtures_dir();
    let crop_path = fixtures.join("real_editor_asimov_crop.png");
    assert!(crop_path.exists(), "real_editor_asimov_crop.png fixture must exist at {:?}", crop_path);

    let raw_img = image::open(&crop_path).expect("open real_editor_asimov_crop.png").to_rgba8();
    let (w, h) = (raw_img.width(), raw_img.height());
    println!("[EMPIRICAL OBS] real_editor_asimov_crop.png dimensions: {} x {}", w, h);

    // 1. Verify perimeter background estimation on real crop
    let (bg_r, bg_g, bg_b) = estimate_background_color(&raw_img);
    println!("[EMPIRICAL OBS] Estimated background color: ({:.1}, {:.1}, {:.1})", bg_r, bg_g, bg_b);
    assert!(is_dark_image(&raw_img), "real_editor_asimov_crop.png must be classified as dark background");

    // 2. Preprocess with enhance_contrast
    let mut prepared = raw_img.clone();
    enhance_contrast(&mut prepared);

    // 3. Detect vertical guidelines
    let guide_cols = detect_vertical_guideline_columns(&prepared);
    println!("[EMPIRICAL OBS] Detected guideline columns in real crop: {:?}", guide_cols);

    // 4. Run detect_horizontal_line_bands
    let bands = detect_horizontal_line_bands(&prepared);
    println!("[EMPIRICAL OBS] Detected bands count: {}", bands.len());
    for (i, (y0, y1)) in bands.iter().enumerate() {
        println!("  Band {:2}: y = {:3}..{:3} (height = {:2}px)", i, y0, y1, y1 - y0);
    }

    // REQUIREMENT 1: >= 8 distinct bands
    assert!(
        bands.len() >= 8,
        "Requirement failed: real_editor_asimov_crop.png must have >= 8 distinct line bands, got: {}",
        bands.len()
    );

    // Compute row ink profile on the prepared image (excluding guide cols) to verify valleys
    let mut row_ink = vec![0u32; h as usize];
    for y in 0..h {
        let mut count = 0u32;
        for x in 0..w {
            if guide_cols.contains(&x) {
                continue;
            }
            let p = prepared.get_pixel(x, y);
            if p[0] < 200 && p[1] < 200 && p[2] < 200 {
                count += 1;
            }
        }
        row_ink[y as usize] = count;
    }

    // REQUIREMENT 2: No text row is cut horizontally through the middle of characters.
    // Between consecutive bands [y0_a, y1_a) and [y0_b, y1_b), the boundary is either:
    // (a) a whitespace gap where y1_a < y0_b, or
    // (b) a dynamic split where y0_b == y1_a + 1.
    // In both cases, the valley/gap rows must NOT have heavy character body ink.
    for (i, (y0, y1)) in bands.iter().enumerate() {
        assert!(y1 > y0, "Band {} must have positive height", i);
        let bh = y1 - y0;
        assert!(bh >= 6 && bh <= 35, "Band {} height {} out of expected range (6..35)", i, bh);
    }

    // REQUIREMENT 2: No text row is cut horizontally through the middle of characters.
    // For all 9 code/output text lines (bands 0..8), verify that the inter-line valleys
    // are true whitespace valleys (ink <= 10), confirming no character bodies are severed.
    for i in 0..8 {
        let (y0, y1) = bands[i];
        let next_y0 = bands[i + 1].0;
        let bh = y1 - y0;
        assert!(bh >= 12 && bh <= 26, "Text line band {} height {} must be 12..26px", i, bh);

        // Verify that the valley between line i and line i+1 has near-zero ink (true whitespace valley)
        for sy in y1..next_y0 {
            let ink = row_ink[sy as usize];
            assert!(
                ink <= 10,
                "Violation: inter-line valley between line {} and {} at y={} has heavy ink ({}), cutting characters!",
                i, i + 1, sy, ink
            );
        }
    }
    println!("[EMPIRICAL OBS] All 9 code and output text lines are cleanly separated by true whitespace valleys without severing any characters!");
}

// ============================================================================
// 2. ADVERSARIAL STRESS: DENSE VERTICAL GUIDE LINES (1px, 2px, dashed, indented)
// ============================================================================

#[test]
fn test_stress_multiple_dense_vertical_guide_lines() {
    let (w, h) = (300u32, 240u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // 6 text lines (height 16px each, pitch 36px)
    for line_idx in 0..6 {
        let y_top = 10 + line_idx * 36;
        for y in y_top..(y_top + 16) {
            for x in 90..260 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    // Dense guide lines:
    // Column 20: 1px guide line (gutter divider)
    for y in 0..h { img.put_pixel(20, y, Rgba([160, 160, 160, 255])); }
    // Column 40: 1px indentation guide (indent 1)
    for y in 0..h { img.put_pixel(40, y, Rgba([180, 180, 180, 255])); }
    // Column 60: 2px indentation guide (indent 2)
    for y in 0..h {
        img.put_pixel(60, y, Rgba([180, 180, 180, 255]));
        img.put_pixel(61, y, Rgba([180, 180, 180, 255]));
    }
    // Column 80: 1px indentation guide (indent 3)
    for y in 0..h { img.put_pixel(80, y, Rgba([180, 180, 180, 255])); }

    let mut working = img.clone();
    let suppressed_count = suppress_vertical_guidelines(&mut working);
    println!("[STRESS OBS] Suppressed {} vertical guide line pixels", suppressed_count);
    assert!(suppressed_count > 0, "Must suppress guide line pixels");

    let bands = detect_horizontal_line_bands(&img);
    println!("[STRESS OBS] Detected bands with 4 dense guide lines: {}", bands.len());
    assert_eq!(bands.len(), 6, "Must detect exactly 6 distinct line bands, not collapsing into mega-band");

    for (i, (y0, y1)) in bands.iter().enumerate() {
        let expected_top = 10 + (i as u32) * 36;
        let expected_bot = expected_top + 16;
        assert!(
            y0 <= &(expected_top + 2) && y1 >= &(expected_bot - 2),
            "Band {} ({}..{}) must match line {} ({}..{})",
            i, y0, y1, i, expected_top, expected_bot
        );
    }
}

#[test]
fn test_stress_dashed_vertical_guide_lines() {
    let (w, h) = (300u32, 200u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // 5 text lines
    for line_idx in 0..5 {
        let y_top = 10 + line_idx * 36;
        for y in y_top..(y_top + 16) {
            for x in 70..250 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    // Dashed guide lines: 3px on, 3px off (common in VS Code indent guides)
    // At x=30, x=50
    for y in 0..h {
        if (y / 3) % 2 == 0 {
            img.put_pixel(30, y, Rgba([170, 170, 170, 255]));
            img.put_pixel(50, y, Rgba([170, 170, 170, 255]));
        }
    }

    let bands = detect_horizontal_line_bands(&img);
    println!("[STRESS OBS] Detected bands with dashed guide lines: {}", bands.len());
    assert_eq!(bands.len(), 5, "Dashed guide lines must not bridge lines into mega-band");
}

#[test]
fn test_stress_guide_lines_intersecting_horizontal_strokes() {
    // Tests that horizontal character strokes crossing vertical guide lines are NOT destroyed by suppression
    let (w, h) = (100u32, 60u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Vertical guide line at x=50 from y=5 to y=55
    for y in 5..55 {
        img.put_pixel(50, y, Rgba([180, 180, 180, 255]));
    }

    // Horizontal minus / dash stroke '-' at y=25, x=45..55 crossing the guide line at x=50
    for x in 45..=55 {
        img.put_pixel(x, 25, Rgba([0, 0, 0, 255]));
    }

    suppress_vertical_guidelines(&mut img);

    // Verify: guide line at non-crossing row y=10 is inpainted to white
    assert_eq!(img.get_pixel(50, 10)[0], 255, "Guide line pixel at (50, 10) must be inpainted to white");
    // Verify: crossing horizontal stroke at (50, 25) remains dark ink
    assert!(
        img.get_pixel(50, 25)[0] < 50,
        "Crossing character stroke at (50, 25) must be preserved: {:?}",
        img.get_pixel(50, 25)
    );
}

// ============================================================================
// 3. ADVERSARIAL STRESS: VARIABLE LINE PITCHES (20px, 32px, 45px, 60px)
// ============================================================================

#[test]
fn test_stress_variable_line_pitches_clean() {
    let pitches = [20u32, 32u32, 45u32, 60u32];

    for &pitch in &pitches {
        let num_lines = 4;
        let line_height = (pitch * 3 / 5).max(8);
        let h = pitch * num_lines + 30;
        let w = 200u32;
        let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

        for line_idx in 0..num_lines {
            let y_top = 15 + line_idx * pitch;
            for y in y_top..(y_top + line_height) {
                for x in 30..170 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }

        let bands = detect_horizontal_line_bands(&img);
        println!("[PITCH OBS] Pitch {}px (line_h {}px): detected {} bands", pitch, line_height, bands.len());
        assert_eq!(
            bands.len(),
            num_lines as usize,
            "Failed clean segmentation at pitch {}px: expected {} bands, got {:?}",
            pitch,
            num_lines,
            bands
        );
    }
}

#[test]
fn test_stress_variable_line_pitches_merged_valley_detection() {
    // Tests dynamic valley pitch splitter when lines are artificially bridged by a 1px vertical bridge
    // (e.g. bracket or brace or cursor)
    let pitches = [24u32, 34u32, 44u32];

    for &pitch in &pitches {
        let line_height = 14u32;
        let h = pitch * 3 + 20;
        let w = 200u32;
        let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

        // 3 lines
        for line_idx in 0..3 {
            let y_top = 10 + line_idx * pitch;
            for y in y_top..(y_top + line_height) {
                for x in 40..160 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }

        // Bridge line 0 and line 1 with a thin 2px vertical stroke at x=42 (like an open brace or bracket)
        let y_bridge_start = 10 + line_height;
        let y_bridge_end = 10 + pitch;
        for y in y_bridge_start..y_bridge_end {
            img.put_pixel(42, y, Rgba([0, 0, 0, 255]));
            img.put_pixel(43, y, Rgba([0, 0, 0, 255]));
        }

        let bands = detect_horizontal_line_bands(&img);
        println!("[MERGED PITCH OBS] Pitch {}px with bridge: detected {} bands", pitch, bands.len());
        assert_eq!(
            bands.len(),
            3,
            "Dynamic pitch valley splitter must split bridged lines at pitch {}px: got {:?}",
            pitch,
            bands
        );
    }
}

// ============================================================================
// 4. ADVERSARIAL STRESS: MIXED BLOCKS, EMPTY LINES, AND COMMENT LINES
// ============================================================================

#[test]
fn test_stress_mixed_empty_lines_and_comments() {
    let (w, h) = (300u32, 300u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Line 0: Header comment `# File: example.py` at y=10..24
    for y in 10..24 { for x in 30..180 { img.put_pixel(x, y, Rgba([60, 60, 60, 255])); } }

    // Line 1: Empty line at y=25..55 (30px whitespace gap)

    // Line 2: `def test_func():` at y=55..69
    for y in 55..69 { for x in 30..160 { img.put_pixel(x, y, Rgba([0, 0, 0, 255])); } }

    // Line 3: Indented comment `    # do work` at y=85..99
    for y in 85..99 { for x in 70..190 { img.put_pixel(x, y, Rgba([80, 80, 80, 255])); } }

    // Line 4: Indented statement `    return 42` at y=115..129
    for y in 115..129 { for x in 70..170 { img.put_pixel(x, y, Rgba([0, 0, 0, 255])); } }

    // Line 5: Empty line at y=130..180 (50px multi-empty gap)

    // Line 6: Single-character closing line `pass` or `}` at y=180..194
    for y in 180..194 { for x in 30..70 { img.put_pixel(x, y, Rgba([0, 0, 0, 255])); } }

    let bands = detect_horizontal_line_bands(&img);
    println!("[MIXED OBS] Detected bands in mixed code/empty/comments: {}", bands.len());
    for (i, (y0, y1)) in bands.iter().enumerate() {
        println!("  Band {}: {}..{} (h={})", i, y0, y1, y1 - y0);
    }

    assert_eq!(bands.len(), 5, "Must detect exactly the 5 non-empty lines, ignoring blank gaps");

    // Check no band spans across the 50px blank gap (y=130..180)
    for (y0, y1) in &bands {
        assert!(!(*y0 < 130 && *y1 > 180), "Band must not bridge across large blank line gap");
    }
}

// ============================================================================
// 5. ADVERSARIAL STRESS: LOW CONTRAST SYNTAX THEMES
// ============================================================================

#[test]
fn test_stress_solarized_dark_theme() {
    // Solarized Dark: BG = #002B36 (0, 43, 54)
    let (w, h) = (200u32, 120u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([0, 43, 54, 255]));

    // Keyword green #859900 (133, 153, 0) at y=15..29
    for y in 15..29 { for x in 20..80 { img.put_pixel(x, y, Rgba([133, 153, 0, 255])); } }
    // Keyword orange #CB4B16 (203, 75, 22) at y=15..29
    for y in 15..29 { for x in 90..140 { img.put_pixel(x, y, Rgba([203, 75, 22, 255])); } }

    // Low contrast comment #586E75 (88, 110, 117) at y=50..64
    for y in 50..64 { for x in 20..150 { img.put_pixel(x, y, Rgba([88, 110, 117, 255])); } }

    // Variable blue #268BD2 (38, 139, 210) at y=85..99
    for y in 85..99 { for x in 20..120 { img.put_pixel(x, y, Rgba([38, 139, 210, 255])); } }

    let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
    assert_eq!((bg_r as u8, bg_g as u8, bg_b as u8), (0, 43, 54), "Must accurately identify Solarized Dark BG");
    assert!(is_dark_image(&img), "Solarized Dark must be dark image");

    enhance_contrast(&mut img);

    // After enhancement, background must be white and syntax tokens must be black ink
    assert_eq!(img.get_pixel(0, 0)[0], 255, "BG must become white");
    assert_eq!(img.get_pixel(30, 20)[0], 0, "Keyword green must become solid black ink");
    assert_eq!(img.get_pixel(30, 55)[0], 0, "Comment base01 must become solid black ink");
    assert_eq!(img.get_pixel(30, 90)[0], 0, "Variable blue must become solid black ink");

    let bands = detect_horizontal_line_bands(&img);
    println!("[SOLARIZED DARK OBS] Detected bands: {}", bands.len());
    assert_eq!(bands.len(), 3, "Must segment all 3 Solarized Dark lines");
}

#[test]
fn test_stress_solarized_light_theme() {
    // Solarized Light: BG = #FDF6E3 (253, 246, 227)
    let (w, h) = (200u32, 120u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([253, 246, 227, 255]));

    // Keyword green #859900 (133, 153, 0) at y=15..29
    for y in 15..29 { for x in 20..80 { img.put_pixel(x, y, Rgba([133, 153, 0, 255])); } }

    // Low contrast comment base1 #93A1A1 (147, 161, 161) at y=50..64
    for y in 50..64 { for x in 20..150 { img.put_pixel(x, y, Rgba([147, 161, 161, 255])); } }

    // Keyword orange #CB4B16 (203, 75, 22) at y=85..99
    for y in 85..99 { for x in 20..120 { img.put_pixel(x, y, Rgba([203, 75, 22, 255])); } }

    let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
    assert_eq!((bg_r as u8, bg_g as u8, bg_b as u8), (253, 246, 227), "Must accurately identify Solarized Light BG");
    assert!(!is_dark_image(&img), "Solarized Light must NOT be dark image");

    enhance_contrast(&mut img);

    let bands = detect_horizontal_line_bands(&img);
    println!("[SOLARIZED LIGHT OBS] Detected bands: {}", bands.len());
    assert_eq!(bands.len(), 3, "Must segment all 3 Solarized Light lines");
}

#[test]
fn test_stress_subtle_low_contrast_theme() {
    // Subtle low contrast: Dark grey editor BG (35, 35, 35), dim text (80, 80, 80) [diff = 45]
    let (w, h) = (200u32, 80u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([35, 35, 35, 255]));

    for y in 12..26 { for x in 20..160 { img.put_pixel(x, y, Rgba([80, 80, 80, 255])); } }
    for y in 45..59 { for x in 20..160 { img.put_pixel(x, y, Rgba([80, 80, 80, 255])); } }

    enhance_contrast(&mut img);

    // Text diff (45) > 38.0 threshold, so it maps to black ink (0)
    assert_eq!(img.get_pixel(30, 18)[0], 0, "Dim text must map to black ink");

    let bands = detect_horizontal_line_bands(&img);
    println!("[LOW CONTRAST OBS] Detected bands: {}", bands.len());
    assert_eq!(bands.len(), 2, "Must segment both low-contrast lines");
}

// ============================================================================
// 6. ADVERSARIAL STRESS: EXTREME SHAPES, CORNERS, AND BOUNDARIES
// ============================================================================

#[test]
fn test_stress_extreme_shapes_and_noise() {
    // 1. Zero/tiny sizes
    let tiny = RgbaImage::new(4, 4);
    assert_eq!(detect_horizontal_line_bands(&tiny), Vec::new(), "Tiny 4x4 must return empty");

    let narrow = RgbaImage::new(5, 100);
    assert_eq!(detect_horizontal_line_bands(&narrow), Vec::new(), "Narrow 5px must return empty");

    // 2. Extremely wide image (1000 x 30) with a single text line
    let mut wide = RgbaImage::from_pixel(1000, 30, Rgba([255, 255, 255, 255]));
    for y in 8..22 {
        for x in 100..900 {
            wide.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let bands_wide = detect_horizontal_line_bands(&wide);
    assert_eq!(bands_wide.len(), 1, "Must handle 1000px wide line");
    assert!(bands_wide[0].0 <= 10 && bands_wide[0].1 >= 20);

    // 3. Fully dark solid image (no text)
    let solid_dark = RgbaImage::from_pixel(100, 100, Rgba([20, 20, 20, 255]));
    let bands_solid = detect_horizontal_line_bands(&solid_dark);
    assert_eq!(bands_solid, Vec::new(), "Solid image must have no bands");
}
