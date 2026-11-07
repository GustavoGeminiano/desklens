//! Challenger M1.2 Independent Empirical Stress Test Suite
//! Focus areas:
//! 1. Boundary conditions:
//!    - Empty crops (1x1, 2x2, 0-ink).
//!    - Images with only vertical guide lines and no text.
//!    - Text with extreme aspect ratios (very tall single column, wide single line).
//!    - Light themes with light grey indentation guides.
//!    - Non-standard fonts and font sizes (tiny 12px to huge 75px).
//! 2. Verification that `h_spans[1]` parenthesis hack is truly gone and no code strings are corrupted.

use desklens_lib::ocr::*;
use image::{Rgba, RgbaImage};

// ============================================================================
// 1. BOUNDARY CONDITIONS: EMPTY CROPS (1x1, 2x2, 0-INK)
// ============================================================================

#[test]
fn test_boundary_empty_crops_1x1_and_2x2() {
    let test_cases: Vec<(&str, u32, u32, Rgba<u8>)> = vec![
        ("1x1 transparent", 1, 1, Rgba([0, 0, 0, 0])),
        ("1x1 white", 1, 1, Rgba([255, 255, 255, 255])),
        ("1x1 black", 1, 1, Rgba([0, 0, 0, 255])),
        ("1x1 dark gray #1e1e1e", 1, 1, Rgba([30, 30, 30, 255])),
        ("2x2 transparent", 2, 2, Rgba([0, 0, 0, 0])),
        ("2x2 white", 2, 2, Rgba([255, 255, 255, 255])),
        ("2x2 black", 2, 2, Rgba([0, 0, 0, 255])),
        ("2x2 dark gray", 2, 2, Rgba([30, 30, 30, 255])),
        ("2x2 syntax peach", 2, 2, Rgba([224, 108, 117, 255])),
        ("2x2 syntax cyan", 2, 2, Rgba([97, 175, 239, 255])),
        ("3x3 tiny mixed", 3, 3, Rgba([40, 44, 52, 255])),
    ];

    for (name, w, h, pixel) in test_cases {
        let mut img = RgbaImage::from_pixel(w, h, pixel);

        // A. Background estimation should never panic or produce NaN
        let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
        assert!(!bg_r.is_nan() && !bg_g.is_nan() && !bg_b.is_nan(), "{}: bg is NaN", name);

        // B. Dark image detection should not panic
        let _is_dark = is_dark_image(&img);

        // C. Contrast enhancement should not panic
        enhance_contrast(&mut img);

        // D. Guide line suppression and detection should return 0 / empty
        let masked = suppress_vertical_guidelines(&mut img);
        assert_eq!(masked, 0, "{}: masked should be 0", name);
        let guide_cols = detect_vertical_guideline_columns(&img);
        assert!(guide_cols.is_empty(), "{}: guide cols should be empty", name);

        // E. Gutter detection should return None
        let gutter = detect_gutter_boundary(&img);
        assert_eq!(gutter.gutter_boundary_x, None, "{}: gutter boundary should be None", name);
        assert!(!gutter.has_line_numbers, "{}: should not have line numbers", name);

        // F. Line band detection MUST return empty Vec without crashing
        let bands = detect_horizontal_line_bands(&img);
        assert!(bands.is_empty(), "{}: bands must be empty for tiny crop, got {:?}", name, bands);

        // G. Preprocessing with metadata must succeed
        let prep = preprocess_image_with_metadata(&img);
        assert_eq!(prep.vertical_guides_masked, 0);
    }
}

#[test]
fn test_boundary_zero_ink_crops_various_sizes() {
    let sizes = [(50, 50), (400, 300), (1000, 200), (30, 800)];

    for &(w, h) in &sizes {
        // White 0-ink
        let white_img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
        let bands_white = detect_horizontal_line_bands(&white_img);
        assert!(
            bands_white.is_empty(),
            "White {}x{} 0-ink crop must have 0 bands, got {:?}",
            w, h, bands_white
        );

        // Dark 0-ink (#1e1e1e)
        let mut dark_img = RgbaImage::from_pixel(w, h, Rgba([30, 30, 30, 255]));
        enhance_contrast(&mut dark_img);
        let bands_dark = detect_horizontal_line_bands(&dark_img);
        assert!(
            bands_dark.is_empty(),
            "Dark {}x{} 0-ink crop must have 0 bands, got {:?}",
            w, h, bands_dark
        );

        // Full metadata pipeline
        let raw_dark = RgbaImage::from_pixel(w, h, Rgba([30, 30, 30, 255]));
        let prep = preprocess_image_with_metadata(&raw_dark);
        let bands_prep = detect_horizontal_line_bands(&prep.processed_image);
        assert!(
            bands_prep.is_empty(),
            "Preprocessed dark {}x{} 0-ink crop must have 0 bands, got {:?}",
            w, h, bands_prep
        );
    }
}

// ============================================================================
// 2. BOUNDARY CONDITIONS: ONLY VERTICAL GUIDE LINES AND NO TEXT
// ============================================================================

#[test]
fn test_boundary_only_vertical_guide_lines_no_text() {
    let (w, h) = (400u32, 300u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Guideline 1: 1px continuous guide at x=35 (gutter divider)
    for y in 0..h {
        img.put_pixel(35, y, Rgba([160, 160, 160, 255]));
    }

    // Guideline 2: 1px continuous indentation guide at x=70
    for y in 0..h {
        img.put_pixel(70, y, Rgba([180, 180, 180, 255]));
    }

    // Guideline 3: 2px continuous indentation guide at x=105, 106
    for y in 0..h {
        img.put_pixel(105, y, Rgba([180, 180, 180, 255]));
        img.put_pixel(106, y, Rgba([180, 180, 180, 255]));
    }

    // Guideline 4: Dashed guide line at x=140 (4px on, 4px off)
    for y in 0..h {
        if (y / 4) % 2 == 0 {
            img.put_pixel(140, y, Rgba([175, 175, 175, 255]));
        }
    }

    // Guideline 5: Short vertical rule at x=200, length 20px (less than 24px threshold)
    for y in 50..70 {
        img.put_pixel(200, y, Rgba([150, 150, 150, 255]));
    }

    // Absolutely NO text pixels are drawn anywhere in the image!

    // 1. Check guideline suppression
    let mut working = img.clone();
    let suppressed = suppress_vertical_guidelines(&mut working);
    println!("[EMPIRICAL OBS] Suppressed {} guide line pixels in text-free image", suppressed);
    assert!(suppressed > 0, "Must suppress continuous guide lines");

    // 2. Crucial test: detect_horizontal_line_bands MUST return 0 bands!
    // Vertical guidelines by themselves must NEVER produce phantom text lines!
    let bands = detect_horizontal_line_bands(&img);
    println!("[EMPIRICAL OBS] Bands detected in text-free guide-line image: {:?}", bands);
    assert!(
        bands.is_empty(),
        "CRITICAL FAILURE: Image with only vertical guide lines produced phantom text bands: {:?}",
        bands
    );
}

// ============================================================================
// 3. EXTREME ASPECT RATIOS: TALL SINGLE COLUMN & WIDE SINGLE LINE
// ============================================================================

#[test]
fn test_extreme_aspect_ratio_tall_single_column() {
    // Very tall narrow column: 35px wide x 720px tall
    // Contains 12 stacked single-token lines (e.g. line numbers 1 to 12 in a narrow gutter crop)
    let (w, h) = (35u32, 720u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    let num_lines = 12;
    let pitch = 55u32;
    let line_h = 16u32;

    for i in 0..num_lines {
        let y_top = 20 + i * pitch;
        // Draw character glyph (width 12px, height 16px) centered in the 35px column (x=11..23)
        for y in y_top..(y_top + line_h) {
            for x in 11..23 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    let bands = detect_horizontal_line_bands(&img);
    println!("[EMPIRICAL OBS] Tall column (35x720): detected {} bands", bands.len());
    for (idx, (y0, y1)) in bands.iter().enumerate() {
        println!("  Band {:2}: y = {:3}..{:3} (h = {:2}px)", idx, y0, y1, y1 - y0);
    }

    assert_eq!(
        bands.len(),
        num_lines as usize,
        "Tall single column must segment all {} lines, got {:?}",
        num_lines,
        bands
    );

    // Verify each band matches the expected line position
    for (i, &(y0, y1)) in bands.iter().enumerate() {
        let expected_top = 20 + (i as u32) * pitch;
        let expected_bot = expected_top + line_h;
        assert!(
            y0 <= expected_top + 2 && y1 >= expected_bot - 2,
            "Band {} ({}-{}) does not match expected ({}-{})",
            i, y0, y1, expected_top, expected_bot
        );
    }
}

#[test]
fn test_extreme_aspect_ratio_wide_single_line() {
    // Very wide single line: 2400px wide x 35px tall
    // Realistic: a user crops a single long statement in a full-screen 4K editor
    let (w, h) = (2400u32, 35u32);

    // Test on dark theme
    let mut dark_img = RgbaImage::from_pixel(w, h, Rgba([30, 30, 30, 255]));
    // Draw text from x=50 to x=2300, y=8..24 (height 16px)
    for y in 8..24 {
        for x in 50..2300 {
            // Draw pseudo-text words separated by whitespace gaps every 80px
            if x % 80 > 15 {
                dark_img.put_pixel(x, y, Rgba([220, 220, 220, 255]));
            }
        }
    }

    enhance_contrast(&mut dark_img);
    let bands = detect_horizontal_line_bands(&dark_img);
    println!("[EMPIRICAL OBS] Wide single line (2400x35): detected {} bands", bands.len());
    for (idx, (y0, y1)) in bands.iter().enumerate() {
        println!("  Band {}: y = {}..{} (h = {}px)", idx, y0, y1, y1 - y0);
    }

    assert_eq!(
        bands.len(),
        1,
        "Wide single line crop must yield exactly 1 band, got {:?}",
        bands
    );
    assert!(bands[0].0 <= 10 && bands[0].1 >= 22, "Band boundaries must tightly encompass the text");
}

#[test]
fn test_extreme_aspect_ratio_wide_multi_line() {
    // Wide multi-line image: 2200px wide x 160px tall with 4 lines
    let (w, h) = (2200u32, 160u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    let num_lines = 4;
    let pitch = 38u32;
    let line_h = 18u32;

    for i in 0..num_lines {
        let y_top = 10 + i * pitch;
        for y in y_top..(y_top + line_h) {
            for x in 40..2100 {
                if x % 100 > 18 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
    }

    let bands = detect_horizontal_line_bands(&img);
    println!("[EMPIRICAL OBS] Wide multi-line (2200x160): detected {} bands", bands.len());
    assert_eq!(
        bands.len(),
        4,
        "Wide multi-line crop must detect exactly 4 bands, got {:?}",
        bands
    );
}

// ============================================================================
// 4. LIGHT THEMES WITH LIGHT GREY INDENTATION GUIDES
// ============================================================================

#[test]
fn test_light_themes_with_various_shades_of_indentation_guides() {
    // Tests light theme (white #FFFFFF background) with 5 lines of code
    // and vertical indentation guides across different shades of grey:
    // Case 1: Light grey #E0E0E0 (224, 224, 224)
    // Case 2: Standard VS Code light guide #D4D4D4 (212, 212, 212)
    // Case 3: Medium grey #B8B8B8 (184, 184, 184)
    // Case 4: Dark grey #888888 (136, 136, 136)
    let guide_shades = [224u8, 212u8, 184u8, 136u8];

    for &shade in &guide_shades {
        let (w, h) = (320u32, 200u32);
        let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

        // 5 lines of code at pitch 36px
        // Line 0: def calculate(x):
        // Line 1:     if x > 0:
        // Line 2:         return x * 2
        // Line 3:     else:
        // Line 4:         return 0
        let indents = [30u32, 60u32, 90u32, 60u32, 90u32];
        for (i, &ind_x) in indents.iter().enumerate() {
            let y_top = 12 + (i as u32) * 36;
            for y in y_top..(y_top + 16) {
                for x in ind_x..280 {
                    img.put_pixel(x, y, Rgba([20, 20, 20, 255]));
                }
            }
        }

        // Draw indentation guide lines at x=55 (level 1) and x=85 (level 2)
        // running continuously across rows 10..180
        for y in 10..180 {
            img.put_pixel(55, y, Rgba([shade, shade, shade, 255]));
            img.put_pixel(85, y, Rgba([shade, shade, shade, 255]));
        }

        // Also draw horizontal minus/hyphen '-' crossing the guide line at x=55
        // (to ensure crossing character strokes are preserved)
        let cross_y = 12 + 36 + 8; // In Line 1
        for x in 50..=60 {
            img.put_pixel(x, cross_y, Rgba([0, 0, 0, 255]));
        }

        let bands = detect_horizontal_line_bands(&img);
        println!(
            "[LIGHT THEME OBS] Guide shade {} (luma {}): detected {} bands",
            shade, shade, bands.len()
        );
        for (idx, (y0, y1)) in bands.iter().enumerate() {
            println!("    Band {}: {}..{} (h={})", idx, y0, y1, y1 - y0);
        }

        assert_eq!(
            bands.len(),
            5,
            "Light theme with guide shade {} failed: expected 5 bands, got {:?}",
            shade,
            bands
        );

        // Verify crossing character stroke was not erased
        let mut prep = img.clone();
        suppress_vertical_guidelines(&mut prep);
        let crossing_px = prep.get_pixel(55, cross_y);
        assert!(
            crossing_px[0] < 50,
            "Crossing stroke at (55, {}) was damaged: {:?}",
            cross_y,
            crossing_px
        );
    }
}

// ============================================================================
// 5. NON-STANDARD FONTS AND FONT SIZES (TINY 12px TO HUGE 75px)
// ============================================================================

#[test]
fn test_non_standard_font_sizes_clean_and_bridged() {
    // Tests 5 distinct font scales:
    // 1. Tiny font: pitch = 14px, line height = 8px, gap = 6px
    // 2. Small font: pitch = 22px, line height = 13px, gap = 9px
    // 3. Medium standard font: pitch = 36px, line height = 20px, gap = 16px
    // 4. Large font: pitch = 52px, line height = 30px, gap = 22px
    // 5. Huge presentation font: pitch = 75px, line height = 44px, gap = 31px
    let font_scales = [
        ("Tiny (14px)", 14u32, 8u32, 4),
        ("Small (22px)", 22u32, 13u32, 4),
        ("Standard (36px)", 36u32, 20u32, 4),
        ("Large (52px)", 52u32, 30u32, 4),
        ("Huge (75px)", 75u32, 44u32, 3),
    ];

    for &(label, pitch, line_h, num_lines) in &font_scales {
        let (w, h) = (240u32, pitch * num_lines + 30);
        let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

        for i in 0..num_lines {
            let y_top = 12 + i * pitch;
            for y in y_top..(y_top + line_h) {
                for x in 30..210 {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }

        let bands = detect_horizontal_line_bands(&img);
        println!(
            "[FONT SCALE OBS] {}: pitch={}px, line_h={}px -> detected {} bands",
            label, pitch, line_h, bands.len()
        );
        assert_eq!(
            bands.len(),
            num_lines as usize,
            "Font scale {} failed: expected {} bands, got {:?}",
            label,
            num_lines,
            bands
        );
    }
}

#[test]
fn test_non_standard_font_variable_line_heights_in_same_crop() {
    // In rich documents or Jupyter notebooks / markdown previews, heading font is large (height 32px),
    // followed by body code lines (height 16px, pitch 32px), followed by output lines (height 12px, pitch 22px).
    let (w, h) = (300u32, 260u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Line 0: Large Heading at y=10..38 (h=28px)
    for y in 10..38 { for x in 30..260 { img.put_pixel(x, y, Rgba([0, 0, 0, 255])); } }

    // Line 1: Code statement at y=55..73 (h=18px)
    for y in 55..73 { for x in 50..220 { img.put_pixel(x, y, Rgba([20, 20, 20, 255])); } }

    // Line 2: Code statement at y=90..108 (h=18px)
    for y in 90..108 { for x in 50..240 { img.put_pixel(x, y, Rgba([20, 20, 20, 255])); } }

    // Line 3: Small console output at y=130..144 (h=14px)
    for y in 130..144 { for x in 30..190 { img.put_pixel(x, y, Rgba([60, 60, 60, 255])); } }

    // Line 4: Small console output at y=155..169 (h=14px)
    for y in 155..169 { for x in 30..180 { img.put_pixel(x, y, Rgba([60, 60, 60, 255])); } }

    let bands = detect_horizontal_line_bands(&img);
    println!("[VARIABLE SIZES OBS] Detected bands across mixed sizes: {}", bands.len());
    for (i, (y0, y1)) in bands.iter().enumerate() {
        println!("  Band {}: {}..{} (h={})", i, y0, y1, y1 - y0);
    }

    assert_eq!(
        bands.len(),
        5,
        "Variable font size document must detect all 5 distinct lines, got {:?}",
        bands
    );
}

// ============================================================================
// 6. VERIFY H_SPANS[1] PARENTHESIS HACK ELIMINATION & CODE STRING INTEGRITY
// ============================================================================

#[test]
fn test_h_spans_hack_elimination_and_token_integrity() {
    // Verify that code segmentation logic does not contain hardcoded `h_spans[1]` erasure
    // or parenthesis injection that destroys Python constructs like `if i % 2 == 0:`

    let raw_source = include_str!("../src/ocr.rs");

    // 1. Check for absence of `h_spans[1]`
    assert!(
        !raw_source.contains("h_spans[1]"),
        "CRITICAL REGRESSION: `h_spans[1]` indexing found in src/ocr.rs!"
    );

    // 2. Check for absence of hardcoded parenthesis insertion
    assert!(
        !raw_source.contains("candidate_slices.push") || !raw_source.contains("spans[1]"),
        "Parenthesis slice mutation heuristic must not exist"
    );

    // 3. Confirm that `h_spans` in line-band recovery only slices halves and individual spans
    // without pixel modification
    assert!(
        raw_source.contains("let mid = h_spans.len() / 2;"),
        "Expected midpoint span division in line-band recovery pass"
    );

    println!("[EMPIRICAL OBS] Verified: `h_spans[1]` parenthesis hack is 100% absent from src/ocr.rs");
}

// ============================================================================
// 7. EMPIRICAL STRESS TEST: NUMERAL TOPS ('0', '2') GLYPH-SEVERING RESISTANCE
// ============================================================================

#[test]
fn test_adversarial_numeral_tops_zero_and_two_no_glyph_severing() {
    // Stress test boundary dilation and line band detection on numerals '0' and '2'
    // with faint anti-aliasing on top arches, ultra-wide crops (2500px),
    // and tight inter-line spacing.
    let (w, h) = (2500u32, 200u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Line 0 (y=20..38): starts with numeral '0' (as in "0 is even")
    // Top arch has only 2 anti-aliased pixels at y=20 (ink = 2)
    // Next row has 4 pixels at y=21
    // Body has 12 pixels per row at y=22..36
    // Bottom arch has 3 pixels at y=37
    img.put_pixel(100, 20, Rgba([140, 140, 140, 255]));
    img.put_pixel(101, 20, Rgba([140, 140, 140, 255]));
    for x in 99..=102 {
        img.put_pixel(x, 21, Rgba([80, 80, 80, 255]));
    }
    for y in 22..=36 {
        for x in 97..=104 {
            if x <= 99 || x >= 102 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }
    for x in 99..=102 {
        img.put_pixel(x, 37, Rgba([80, 80, 80, 255]));
    }
    // Remainder of line 0: " is even"
    for y in 24..35 {
        for x in 120..220 {
            if x % 20 > 5 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    // Line 1 (y=55..73): starts with numeral '2' (as in "2 is even")
    // Top arch of '2' at y=55 has faint 3px stroke (ink = 3)
    // Upper loop right side at y=56..61
    // Diagonal stroke at y=62..67
    // Horizontal base at y=68..71 (10px wide)
    for x in 100..=102 {
        img.put_pixel(x, 55, Rgba([150, 150, 150, 255]));
    }
    for y in 56..=61 {
        for x in 102..=104 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    for y in 62..=67 {
        let diag_x = 104 - (y - 62);
        img.put_pixel(diag_x, y, Rgba([0, 0, 0, 255]));
        img.put_pixel(diag_x + 1, y, Rgba([0, 0, 0, 255]));
    }
    for y in 68..=71 {
        for x in 96..=105 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    // Remainder of line 1: " is even"
    for y in 58..70 {
        for x in 120..220 {
            if x % 20 > 5 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    let bands = detect_horizontal_line_bands(&img);
    println!("[NUMERAL TEST OBS] Detected bands count: {}", bands.len());
    for (i, (y0, y1)) in bands.iter().enumerate() {
        println!("  Band {}: y = {}..{} (height = {}px)", i, y0, y1, y1 - y0);
    }

    assert_eq!(bands.len(), 2, "Must detect exactly 2 lines");

    // Line 0 top arch check:
    assert!(
        bands[0].0 <= 20,
        "SEVERING VIOLATION: Line 0 band start (y={}) severed top arch of '0' (at y=20)!",
        bands[0].0
    );
    assert!(
        bands[0].1 >= 37,
        "Line 0 band end (y={}) truncated bottom of '0' (at y=37)",
        bands[0].1
    );

    // Line 1 top arch check:
    assert!(
        bands[1].0 <= 55,
        "SEVERING VIOLATION: Line 1 band start (y={}) severed top arch of '2' (at y=55)!",
        bands[1].0
    );
    assert!(
        bands[1].1 >= 71,
        "Line 1 band end (y={}) truncated base of '2' (at y=71)",
        bands[1].1
    );
}

#[test]
fn test_adversarial_numeral_at_absolute_image_edges() {
    // Numerals touching the absolute top (y=0) and absolute bottom (y=h-1) of crop
    let (w, h) = (300u32, 80u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Numeral '0' at y = 0..16 (touching top edge)
    for y in 0..16 {
        for x in 20..35 {
            if y == 0 || y == 15 || x <= 22 || x >= 32 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    // Numeral '2' at y = 60..79 (touching bottom edge h-1 = 79)
    for y in 60..80 {
        for x in 20..35 {
            if y == 60 || y >= 76 || x >= 31 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }

    let bands = detect_horizontal_line_bands(&img);
    println!("[EDGE NUMERAL OBS] Detected bands: {:?}", bands);
    assert_eq!(bands.len(), 2, "Must detect 2 bands touching image edges");
    assert_eq!(bands[0].0, 0, "Band 0 must start at 0 without panic");
    assert!(bands[0].1 >= 16, "Band 0 encompasses '0'");
    assert!(bands[1].0 <= 60, "Band 1 encompasses top of '2'");
    assert_eq!(bands[1].1, 80, "Band 1 must reach bottom edge without overflow");
}

