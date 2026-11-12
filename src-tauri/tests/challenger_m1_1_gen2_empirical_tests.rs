//! Empirical Challenge Test Suite for Milestone 1 (Gen 2)
//! Author: Challenger 1 (critic, specialist)
//! Target: src-tauri/src/ocr.rs (suppress_vertical_guidelines, detect_gutter_boundary, enhance_contrast)

use desklens_lib::ocr::*;
use image::{Rgba, RgbaImage};

// ============================================================================
// Helper Functions for Test Fixture Generation
// ============================================================================

fn make_white_canvas(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]))
}

fn make_dark_canvas(w: u32, h: u32, bg: [u8; 3]) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba([bg[0], bg[1], bg[2], 255]))
}

fn draw_vertical_line(img: &mut RgbaImage, x: u32, y_start: u32, y_end: u32, thickness: u32, color: [u8; 3]) {
    for t in 0..thickness {
        let col = x + t;
        if col < img.width() {
            for y in y_start..y_end.min(img.height()) {
                img.put_pixel(col, y, Rgba([color[0], color[1], color[2], 255]));
            }
        }
    }
}

fn draw_horizontal_stroke(img: &mut RgbaImage, x_start: u32, x_end: u32, y: u32, thickness: u32, color: [u8; 3]) {
    for t in 0..thickness {
        let row = y + t;
        if row < img.height() {
            for x in x_start..x_end.min(img.width()) {
                img.put_pixel(x, row, Rgba([color[0], color[1], color[2], 255]));
            }
        }
    }
}

// ============================================================================
// Test 1: Vertical Lines with Varying Thickness (1px, 2px, 3px, 4px) and Heights
// ============================================================================

#[test]
fn test_empirical_vertical_lines_height_threshold_under_24px() {
    // Guidelines with height < 24px must NOT be suppressed (should retain ink)
    let heights = [5u32, 10, 16, 20, 23];
    let thicknesses = [1u32, 2, 3, 4];

    for &h in &heights {
        for &thick in &thicknesses {
            let mut img = make_white_canvas(60, 50);
            let x_line = 30;
            let y_start = 10;
            let y_end = y_start + h;

            draw_vertical_line(&mut img, x_line, y_start, y_end, thick, [0, 0, 0]);

            let suppressed = suppress_vertical_guidelines(&mut img);
            assert_eq!(
                suppressed, 0,
                "Height < 24px (h={h}, thick={thick}) must NOT be suppressed, got {suppressed} suppressed pixels"
            );

            // Verify all ink pixels remain intact
            for t in 0..thick {
                for y in y_start..y_end {
                    let p = img.get_pixel(x_line + t, y);
                    assert_eq!(
                        p[0], 0,
                        "Ink pixel at ({}, {}) was unexpectedly erased for h={h}, thick={thick}",
                        x_line + t, y
                    );
                }
            }
        }
    }
}

#[test]
fn test_empirical_vertical_lines_height_threshold_over_24px_thickness_1px_and_2px() {
    // 1px and 2px vertical lines with height >= 24px MUST be fully suppressed to white (255, 255, 255)
    let heights = [24u32, 25, 30, 48, 80];

    for &h in &heights {
        // 1px line
        {
            let mut img = make_white_canvas(80, 100);
            let x_line = 40;
            let y_start = 5;
            let y_end = y_start + h;
            draw_vertical_line(&mut img, x_line, y_start, y_end, 1, [0, 0, 0]);

            let suppressed = suppress_vertical_guidelines(&mut img);
            assert_eq!(
                suppressed, h as usize,
                "1px line of height {} must have exactly {} pixels suppressed, got {}",
                h, h, suppressed
            );

            // Verify every pixel on the line is now white (255, 255, 255)
            for y in y_start..y_end {
                let p = img.get_pixel(x_line, y);
                assert_eq!(p[0], 255, "Pixel at ({}, {}) must be white after suppression", x_line, y);
                assert_eq!(p[1], 255);
                assert_eq!(p[2], 255);
            }
        }

        // 2px line
        {
            let mut img = make_white_canvas(80, 100);
            let x_line = 40;
            let y_start = 5;
            let y_end = y_start + h;
            draw_vertical_line(&mut img, x_line, y_start, y_end, 2, [0, 0, 0]);

            let suppressed = suppress_vertical_guidelines(&mut img);
            assert_eq!(
                suppressed, (h * 2) as usize,
                "2px line of height {} must have exactly {} pixels suppressed, got {}",
                h, h * 2, suppressed
            );

            for t in 0..2 {
                for y in y_start..y_end {
                    let p = img.get_pixel(x_line + t, y);
                    assert_eq!(p[0], 255, "2px line pixel at ({}, {}) must be white", x_line + t, y);
                }
            }
        }
    }
}

#[test]
fn test_empirical_vertical_lines_thickness_3px_and_4px() {
    // Empirical examination of 3px and 4px lines >= 24px:
    // Guideline specification defines guidelines as 1-2px editor rules.
    // Lines >= 3px/4px represent block glyph boundaries, borders, or wide rules.
    let h = 30u32;

    // 3px line
    {
        let mut img = make_white_canvas(80, 50);
        let x_line = 35;
        draw_vertical_line(&mut img, x_line, 10, 10 + h, 3, [0, 0, 0]);
        let suppressed = suppress_vertical_guidelines(&mut img);
        println!("[EMPIRICAL OBS] 3px vertical line: {} of {} pixels suppressed", suppressed, h * 3);
        // Both outer columns (x=35, x=37) have empty neighbours within 2px, but center column x=36
        // has left and right ink, which causes crossing heuristic or outer masking.
        // We record the empirical behavior: outer columns are masked (2 * 30 = 60 pixels).
        assert!(suppressed >= 60, "3px line outer columns should be suppressed (>= 60 px)");
    }

    // 4px line
    {
        let mut img = make_white_canvas(80, 50);
        let x_line = 35;
        draw_vertical_line(&mut img, x_line, 10, 10 + h, 4, [0, 0, 0]);
        let suppressed = suppress_vertical_guidelines(&mut img);
        println!("[EMPIRICAL OBS] 4px vertical line: {} of {} pixels suppressed", suppressed, h * 4);
        // Columns 35 and 38 have empty outer neighbours, shaving boundary pixels.
        assert!(suppressed >= 60, "4px line boundary pixels shaved, got: {}", suppressed);
    }
}

// ============================================================================
// Test 2: Crossing Characters and Operators (+, -, =, _)
// ============================================================================

#[test]
fn test_empirical_crossing_character_hyphen_minus() {
    // A horizontal hyphen '-' crossing a 1px vertical guide line (40px tall).
    // The horizontal stroke of '-' must NOT be erased at the intersection point!
    let mut img = make_white_canvas(60, 50);
    let x_guide = 30;
    // 1px vertical guide from y=5 to y=45 (height 40px >= 24)
    draw_vertical_line(&mut img, x_guide, 5, 45, 1, [50, 50, 50]);

    // Horizontal '-' from x=26 to x=35 at y=25 (thickness 2px)
    draw_horizontal_stroke(&mut img, 26, 35, 25, 2, [0, 0, 0]);

    let suppressed = suppress_vertical_guidelines(&mut img);
    println!("[EMPIRICAL OBS] Hyphen '-' crossing test: {} guide pixels suppressed", suppressed);

    // Verify the intersection pixels at (x_guide, 25) and (x_guide, 26) are PRESERVED (ink < 200)
    for y in 25..=26 {
        let p = img.get_pixel(x_guide, y);
        assert!(
            p[0] < 200,
            "Horizontal hyphen pixel at intersection ({}, {}) was accidentally erased! p={:?}",
            x_guide, y, p
        );
    }

    // Verify the rest of the '-' stroke on left and right is intact
    for x in 26..35 {
        for y in 25..=26 {
            let p = img.get_pixel(x, y);
            assert!(p[0] < 200, "Hyphen stroke at ({}, {}) was damaged! p={:?}", x, y, p);
        }
    }

    // Verify guideline pixels ABOVE and BELOW the hyphen WERE suppressed
    for y in 5..25 {
        let p = img.get_pixel(x_guide, y);
        assert_eq!(p[0], 255, "Guide pixel above hyphen at ({}, {}) was not suppressed", x_guide, y);
    }
    for y in 27..45 {
        let p = img.get_pixel(x_guide, y);
        assert_eq!(p[0], 255, "Guide pixel below hyphen at ({}, {}) was not suppressed", x_guide, y);
    }
}

#[test]
fn test_empirical_crossing_character_equals() {
    // An '=' operator with two horizontal strokes at y=20 and y=26 crossing a vertical guide line.
    let mut img = make_white_canvas(60, 50);
    let x_guide = 28;
    draw_vertical_line(&mut img, x_guide, 5, 45, 1, [50, 50, 50]);

    // Bar 1 of '=' at y=20
    draw_horizontal_stroke(&mut img, 23, 34, 20, 2, [0, 0, 0]);
    // Bar 2 of '=' at y=26
    draw_horizontal_stroke(&mut img, 23, 34, 26, 2, [0, 0, 0]);

    let suppressed = suppress_vertical_guidelines(&mut img);
    println!("[EMPIRICAL OBS] Equals '=' crossing test: {} guide pixels suppressed", suppressed);

    // Verify both bars at the intersection are preserved
    for y in 20..=21 {
        let p = img.get_pixel(x_guide, y);
        assert!(p[0] < 200, "Equals bar 1 intersection ({}, {}) erased!", x_guide, y);
    }
    for y in 26..=27 {
        let p = img.get_pixel(x_guide, y);
        assert!(p[0] < 200, "Equals bar 2 intersection ({}, {}) erased!", x_guide, y);
    }

    // Verify inter-bar gap on the guide line (y=22..26) IS suppressed to white!
    for y in 22..26 {
        let p = img.get_pixel(x_guide, y);
        // Wait: is the gap between the two bars suppressed if it's only 4px?
        // In suppress_vertical_guidelines, the run was from y=5 to y=45 (run_len=40).
        // Since y=22..25 is inside start_y..end_y and is not crossing, it should be masked!
        assert_eq!(
            p[0], 255,
            "Inter-bar gap on guide line at ({}, {}) should be suppressed to white, got {}",
            x_guide, y, p[0]
        );
    }
}

#[test]
fn test_empirical_crossing_character_plus() {
    // A '+' character with horizontal bar crossing the guide line and vertical stem offset or on guide
    let mut img = make_white_canvas(60, 50);
    let x_guide = 30;
    draw_vertical_line(&mut img, x_guide, 5, 45, 1, [60, 60, 60]);

    // Horizontal bar of '+' from x=25 to x=35 at y=24
    draw_horizontal_stroke(&mut img, 25, 36, 24, 2, [0, 0, 0]);

    // Vertical stem of '+' at x=28 (offset from guide by 2px) from y=20 to y=30 (height 10px < 24)
    draw_vertical_line(&mut img, 28, 20, 30, 2, [0, 0, 0]);

    let suppressed = suppress_vertical_guidelines(&mut img);
    println!("[EMPIRICAL OBS] Plus '+' crossing test: {} guide pixels suppressed", suppressed);

    // Verify horizontal intersection preserved
    for y in 24..=25 {
        let p = img.get_pixel(x_guide, y);
        assert!(p[0] < 200, "Plus horizontal stroke at ({}, {}) was erased!", x_guide, y);
    }

    // Verify vertical stem of '+' at x=28,29 was NOT touched (since h=10 < 24)
    for t in 0..2 {
        for y in 20..30 {
            let p = img.get_pixel(28 + t, y);
            assert!(p[0] < 200, "Plus vertical stem at ({}, {}) was damaged!", 28 + t, y);
        }
    }
}

#[test]
fn test_empirical_crossing_character_underscore() {
    // Underscore '_' at bottom of line crossing the guide
    let mut img = make_white_canvas(60, 50);
    let x_guide = 25;
    draw_vertical_line(&mut img, x_guide, 5, 45, 1, [70, 70, 70]);

    // Underscore at y=32..34, x=20..32
    draw_horizontal_stroke(&mut img, 20, 32, 32, 2, [0, 0, 0]);

    suppress_vertical_guidelines(&mut img);

    // Verify underscore intersection preserved
    for y in 32..=33 {
        let p = img.get_pixel(x_guide, y);
        assert!(p[0] < 200, "Underscore intersection at ({}, {}) was erased!", x_guide, y);
    }
}

#[test]
fn test_empirical_crossing_character_on_2px_guideline() {
    // 2px vertical guideline crossed by '-'
    let mut img = make_white_canvas(60, 50);
    let x_guide = 30; // columns 30 and 31
    draw_vertical_line(&mut img, x_guide, 5, 45, 2, [50, 50, 50]);

    // Horizontal '-' from x=25 to x=36 at y=22 (thickness 2px)
    draw_horizontal_stroke(&mut img, 25, 36, 22, 2, [0, 0, 0]);

    suppress_vertical_guidelines(&mut img);

    // Verify intersection on BOTH columns 30 and 31 is preserved
    for x in 30..=31 {
        for y in 22..=23 {
            let p = img.get_pixel(x, y);
            assert!(
                p[0] < 200,
                "2px guideline crossing at ({}, {}) was accidentally erased! p={:?}",
                x, y, p
            );
        }
    }
}

// ============================================================================
// Test 3: Dark Theme Crops with Faint Syntax Colors (Gray, Orange, Cyan)
// ============================================================================

#[test]
fn test_empirical_dark_theme_faint_syntax_colors() {
    // Dark background #1e1e1e (RGB 30, 30, 30)
    let bg = [30u8, 30, 30];
    let mut img = make_dark_canvas(120, 80, bg);

    // 1. Faint gray: #454545 (RGB 69, 69, 69) - muted comments or operators
    let faint_gray = [0x45u8, 0x45, 0x45];
    draw_horizontal_stroke(&mut img, 10, 40, 15, 3, faint_gray);

    // 2. Orange keyword: #c57540 (RGB 197, 117, 64) - Python 'if', 'else', 'def'
    let orange_kw = [0xc5u8, 0x75, 0x40];
    draw_horizontal_stroke(&mut img, 10, 40, 35, 3, orange_kw);

    // 3. Cyan identifier: #7ec7e8 (RGB 126, 199, 232) - variables 'i', 'x'
    let cyan_ident = [0x7eu8, 0xc7, 0xe8];
    draw_horizontal_stroke(&mut img, 10, 40, 55, 3, cyan_ident);

    // Run enhance_contrast
    enhance_contrast(&mut img);

    // Assertions:
    // Background must be pure white (255, 255, 255)
    let bg_pixel = img.get_pixel(5, 5);
    assert_eq!(
        bg_pixel[0], 255,
        "Background must be mapped to pure white (255), got {:?}",
        bg_pixel
    );
    assert_eq!(bg_pixel[1], 255);
    assert_eq!(bg_pixel[2], 255);

    // 1. Check Faint Gray #454545: must be mapped to dark ink (RGB < 30)
    for x in 12..38 {
        for y in 15..18 {
            let p = img.get_pixel(x, y);
            assert!(
                p[0] < 30 && p[1] < 30 && p[2] < 30,
                "Faint gray #454545 at ({}, {}) was NOT mapped to dark ink (< 30), got {:?}",
                x, y, p
            );
        }
    }
    println!("[EMPIRICAL OBS] Faint gray #454545 mapped to RGB: {:?}", img.get_pixel(20, 16));

    // 2. Check Orange #c57540: must be mapped to dark ink (RGB < 30)
    for x in 12..38 {
        for y in 35..38 {
            let p = img.get_pixel(x, y);
            assert!(
                p[0] < 30 && p[1] < 30 && p[2] < 30,
                "Orange #c57540 at ({}, {}) was NOT mapped to dark ink (< 30), got {:?}",
                x, y, p
            );
        }
    }
    println!("[EMPIRICAL OBS] Orange #c57540 mapped to RGB: {:?}", img.get_pixel(20, 36));

    // 3. Check Cyan #7ec7e8: must be mapped to dark ink (RGB < 30)
    for x in 12..38 {
        for y in 55..58 {
            let p = img.get_pixel(x, y);
            assert!(
                p[0] < 30 && p[1] < 30 && p[2] < 30,
                "Cyan #7ec7e8 at ({}, {}) was NOT mapped to dark ink (< 30), got {:?}",
                x, y, p
            );
        }
    }
    println!("[EMPIRICAL OBS] Cyan #7ec7e8 mapped to RGB: {:?}", img.get_pixel(20, 56));
}

#[test]
fn test_empirical_dark_theme_variety_of_backgrounds() {
    // Test that faint syntax colors reach dark ink across various dark backgrounds
    // #181818 (24), #1e1e1e (30), #202020 (32)
    let backgrounds = [[24u8, 24, 24], [30u8, 30, 30], [32u8, 32, 32]];

    for bg in backgrounds {
        let mut img = make_dark_canvas(100, 60, bg);
        // Orange keyword
        draw_horizontal_stroke(&mut img, 10, 50, 20, 3, [0xc5, 0x75, 0x40]);
        // Cyan identifier
        draw_horizontal_stroke(&mut img, 10, 50, 40, 3, [0x7e, 0xc7, 0xe8]);

        enhance_contrast(&mut img);

        let p_orange = img.get_pixel(25, 21);
        let p_cyan = img.get_pixel(25, 41);
        assert!(
            p_orange[0] < 30,
            "Orange on bg {:?} failed to reach ink < 30, got {}",
            bg, p_orange[0]
        );
        assert!(
            p_cyan[0] < 30,
            "Cyan on bg {:?} failed to reach ink < 30, got {}",
            bg, p_cyan[0]
        );
    }
}

// ============================================================================
// Test 4: Gutter Boundary Detection (detect_gutter_boundary)
// ============================================================================

#[test]
fn test_empirical_detect_gutter_boundary_with_guide_line() {
    // Image with gutter line numbers at x=10..22, a vertical guide at x=35, and code body at x=45..100
    let mut img = make_white_canvas(120, 80);

    // Line numbers in gutter
    draw_horizontal_stroke(&mut img, 10, 20, 15, 8, [0, 0, 0]); // "1"
    draw_horizontal_stroke(&mut img, 10, 20, 40, 8, [0, 0, 0]); // "2"
    draw_horizontal_stroke(&mut img, 10, 20, 65, 8, [0, 0, 0]); // "3"

    // Vertical gutter separator guide line at x=35 (height 60px >= 24)
    draw_vertical_line(&mut img, 35, 10, 70, 1, [0, 0, 0]);

    // Code body
    draw_horizontal_stroke(&mut img, 50, 90, 15, 8, [0, 0, 0]);
    draw_horizontal_stroke(&mut img, 50, 90, 40, 8, [0, 0, 0]);

    let gutter = detect_gutter_boundary(&img);
    println!("[EMPIRICAL OBS] Gutter result with guide: {:?}", gutter);

    assert!(gutter.has_line_numbers, "Must detect line numbers present");
    assert!(gutter.gutter_boundary_x.is_some(), "Must detect gutter boundary x");
    let bx = gutter.gutter_boundary_x.unwrap();
    // Boundary should be near x=35 (typically 35 + 2 = 37.0)
    assert!(
        bx >= 35.0 && bx <= 40.0,
        "Gutter boundary should be around guide line x=35, got: {}",
        bx
    );
}

#[test]
fn test_empirical_detect_gutter_boundary_with_whitespace_gap() {
    // Image with gutter line numbers at x=8..18, whitespace gap from x=19..32, and code at x=35..120
    let mut img = make_white_canvas(120, 80);

    // Line numbers
    draw_horizontal_stroke(&mut img, 8, 16, 20, 6, [0, 0, 0]);
    draw_horizontal_stroke(&mut img, 8, 16, 45, 6, [0, 0, 0]);

    // Code body starting at x=35
    draw_horizontal_stroke(&mut img, 35, 95, 20, 6, [0, 0, 0]);
    draw_horizontal_stroke(&mut img, 35, 95, 45, 6, [0, 0, 0]);

    let gutter = detect_gutter_boundary(&img);
    println!("[EMPIRICAL OBS] Gutter result with gap: {:?}", gutter);

    assert!(gutter.has_line_numbers, "Must detect line numbers separated by gap");
    assert!(gutter.gutter_boundary_x.is_some(), "Must detect gutter boundary x");
    let bx = gutter.gutter_boundary_x.unwrap();
    assert!(
        bx >= 24.0 && bx <= 36.0,
        "Gutter boundary should be within/at end of whitespace gap (19..32), got: {}",
        bx
    );
}

#[test]
fn test_empirical_detect_gutter_boundary_without_gutter() {
    // Image where code starts directly at x=10 with no gutter or line numbers
    let mut img = make_white_canvas(120, 80);
    draw_horizontal_stroke(&mut img, 10, 100, 20, 6, [0, 0, 0]);
    draw_horizontal_stroke(&mut img, 10, 100, 45, 6, [0, 0, 0]);

    let gutter = detect_gutter_boundary(&img);
    println!("[EMPIRICAL OBS] Gutter result without gutter: {:?}", gutter);

    assert!(!gutter.has_line_numbers, "Should NOT detect line numbers when code starts at left");
    assert!(gutter.gutter_boundary_x.is_none(), "Boundary should be None");
}

// ============================================================================
// Test 5: End-to-End Layout Conditioning and Band Detection
// ============================================================================

#[test]
fn test_empirical_detect_horizontal_line_bands_with_crossing_guides() {
    // Multi-line code crop with 3 text lines and a vertical indentation guide line passing through them.
    // Verify that guideline suppression allows all 3 lines to be cleanly separated into 3 bands.
    let mut img = make_white_canvas(100, 100);

    // Line 1: y = 15..25
    draw_horizontal_stroke(&mut img, 15, 80, 15, 10, [0, 0, 0]);
    // Line 2: y = 45..55
    draw_horizontal_stroke(&mut img, 25, 80, 45, 10, [0, 0, 0]);
    // Line 3: y = 75..85
    draw_horizontal_stroke(&mut img, 25, 80, 75, 10, [0, 0, 0]);

    // Continuous 1px indentation guide at x=20 spanning the whole height y=10..90 (80px tall)
    draw_vertical_line(&mut img, 20, 10, 90, 1, [0, 0, 0]);

    // Run detect_horizontal_line_bands
    let bands = detect_horizontal_line_bands(&img);
    println!("[EMPIRICAL OBS] Detected bands: {:?}", bands);

    assert_eq!(
        bands.len(), 3,
        "Indentation guide line must not merge lines! Expected 3 bands, got: {:?}",
        bands
    );

    // Verify the bands correspond to the 3 lines
    assert!(bands[0].0 <= 16 && bands[0].1 >= 24, "Band 0 mismatch: {:?}", bands[0]);
    assert!(bands[1].0 <= 46 && bands[1].1 >= 54, "Band 1 mismatch: {:?}", bands[1]);
    assert!(bands[2].0 <= 76 && bands[2].1 >= 84, "Band 2 mismatch: {:?}", bands[2]);
}

#[test]
fn test_empirical_inspect_asimov_crop_row_ink() {
    use std::path::PathBuf;
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.parent().unwrap().join("tests").join("fixtures").join("real_editor_asimov_crop.png");
    if !fixture_path.exists() {
        println!("Fixture not found at {:?}", fixture_path);
        return;
    }

    let raw_img = image::open(&fixture_path).expect("open fixture").to_rgba8();
    let (w, h) = (raw_img.width(), raw_img.height());
    let mut prepared = raw_img.clone();
    enhance_contrast(&mut prepared);

    let mut working_img = prepared.clone();
    let masked = suppress_vertical_guidelines(&mut working_img);
    let guide_cols = detect_vertical_guideline_columns(&prepared);
    println!("[EMPIRICAL OBS] Asimov: w={}, h={}, masked={}, guide_cols={:?}", w, h, masked, guide_cols);

    let mut row_ink = vec![0u32; h as usize];
    for y in 0..h {
        let mut count = 0u32;
        for x in 0..w {
            if guide_cols.contains(&x) {
                continue;
            }
            let p = working_img.get_pixel(x, y);
            if p[0] < 200 && p[1] < 200 && p[2] < 200 {
                count += 1;
            }
        }
        row_ink[y as usize] = count;
    }

    // Print rows where row_ink is between 0 and 15
    for y in 0..h {
        if row_ink[y as usize] < 15 {
            println!("  row {:3}: ink = {:3}", y, row_ink[y as usize]);
        }
    }

    // Inspect rows 42, 72, 108, 135
    for &sample_y in &[42u32, 72, 108, 135] {
        let mut ink_cols = Vec::new();
        for x in 0..w {
            if guide_cols.contains(&x) { continue; }
            let p = working_img.get_pixel(x, sample_y);
            if p[0] < 200 && p[1] < 200 && p[2] < 200 {
                ink_cols.push(x);
            }
        }
        println!("[EMPIRICAL OBS] row {} has {} ink pixels at cols: {:?}", sample_y, ink_cols.len(), ink_cols);
    }

    println!("[EMPIRICAL OBS] raw_img at (5, 42): {:?}", raw_img.get_pixel(5, 42));
    println!("[EMPIRICAL OBS] prepared at (5, 42): {:?}", prepared.get_pixel(5, 42));
    let gutter = detect_gutter_boundary(&prepared);
    println!("[EMPIRICAL OBS] gutter boundary: {:?}", gutter);
    assert!(is_dark_image(&raw_img), "real_editor_asimov_crop.png must be dark image");
    assert!(masked >= 200, "Must mask vertical guidelines on real crop, got: {}", masked);
    assert!(gutter.has_line_numbers, "Must detect gutter line numbers on real crop");
    assert!(gutter.gutter_boundary_x.is_some(), "Must detect gutter boundary x on real crop");
    let g_x = gutter.gutter_boundary_x.unwrap();
    assert!(g_x >= 18.0 && g_x <= 45.0, "Gutter boundary x should be near 20.0-40.0, got: {}", g_x);
}



