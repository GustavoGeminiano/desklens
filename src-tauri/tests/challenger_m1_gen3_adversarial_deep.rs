//! Deep Adversarial Stress Test Suite for Milestone 1 Gen 3
//! Targeting: Wide crops, dense vertical lines, sparse delimiters, border artifacts, and low-contrast themes.

use desklens_lib::ocr::*;
use image::{Rgba, RgbaImage};

// ============================================================================
// 1. ADVERSARIAL STRESS: ULTRA-WIDE CROPS & SPARSE DELIMITERS
// ============================================================================

#[test]
fn test_stress_wide_crop_sparse_delimiters_1920_and_3840() {
    let widths = [1920u32, 2560u32, 3840u32];

    for &w in &widths {
        let h = 200u32;
        let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

        // Line 0: "def calculate_total(items):" at y=20..36 (height 16px)
        for y in 20..36 {
            for x in 100..450 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        // Line 1: Indented body "    subtotal = sum(items)" at y=56..72 (height 16px)
        for y in 56..72 {
            for x in 180..600 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        // Line 2: Sparse delimiter "    }" at y=92..108 (height 16px)
        // Monospace closing brace '}' is only 4px wide! (x=180..184)
        for y in 92..108 {
            for x in 180..184 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        // Line 3: Sparse delimiter ";" at y=128..144 (height 16px)
        // Semicolon is only 3px wide! (x=100..103)
        for y in 128..144 {
            for x in 100..103 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        // Line 4: "return subtotal" at y=164..180 (height 16px)
        for y in 164..180 {
            for x in 100..320 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        let bands = detect_horizontal_line_bands(&img);
        println!(
            "[WIDE CROP OBS] Width {}px: detected {} bands (expected 5)",
            w,
            bands.len()
        );
        for (i, (y0, y1)) in bands.iter().enumerate() {
            println!("  Band {}: y={}..{} (h={}px)", i, y0, y1, y1 - y0);
        }

        assert_eq!(
            bands.len(),
            5,
            "Wide crop {}px must detect all 5 lines including sparse single-character delimiter lines '}}' and ';'",
            w
        );

        // Verify Band 2 corresponds to '}'
        assert!(
            bands[2].0 <= 94 && bands[2].1 >= 106,
            "Band 2 must tightly bound line 2 (92..108), got: {:?}",
            bands[2]
        );

        // Verify Band 3 corresponds to ';'
        assert!(
            bands[3].0 <= 130 && bands[3].1 >= 142,
            "Band 3 must tightly bound line 3 (128..144), got: {:?}",
            bands[3]
        );
    }
}

// ============================================================================
// 2. ADVERSARIAL STRESS: DENSE VERTICAL GUIDE LINES & INDENTATION
// ============================================================================

#[test]
fn test_stress_dense_vertical_guide_lines_and_indentation() {
    let (w, h) = (400u32, 260u32);
    // Dark background #1E1E1E (30, 30, 30)
    let mut img = RgbaImage::from_pixel(w, h, Rgba([30, 30, 30, 255]));

    // 6 text lines (height 15px each, pitch 38px) with realistic character glyph spacing
    for line_idx in 0..6 {
        let y_top = 15 + line_idx * 38;
        let indent_x = 40 + line_idx * 20; // Staircase indentation
        for y in y_top..(y_top + 15) {
            for x in indent_x..350 {
                // Realistic monospace font: 8px char width, 2px stroke, 6px advance
                if (x % 9) < 5 {
                    img.put_pixel(x, y, Rgba([220, 220, 220, 255]));
                }
            }
        }
    }

    // Dense guide lines across indentation levels:
    // Guide 1 (x=20): solid 1px gutter guide
    for y in 0..h {
        img.put_pixel(20, y, Rgba([70, 70, 70, 255]));
    }
    // Guide 2 (x=40): solid 1px indent guide level 1
    for y in 0..h {
        img.put_pixel(40, y, Rgba([75, 75, 75, 255]));
    }
    // Guide 3 (x=60): solid 2px indent guide level 2
    for y in 0..h {
        img.put_pixel(60, y, Rgba([80, 80, 80, 255]));
        img.put_pixel(61, y, Rgba([80, 80, 80, 255]));
    }
    // Guide 4 (x=80): solid 1px indent guide level 3
    for y in 0..h {
        img.put_pixel(80, y, Rgba([75, 75, 75, 255]));
    }
    // Guide 5 (x=100): solid 1px indent guide level 4
    for y in 0..h {
        img.put_pixel(100, y, Rgba([75, 75, 75, 255]));
    }
    // Guide 6 (x=120): solid 1px indent guide level 5
    for y in 0..h {
        img.put_pixel(120, y, Rgba([75, 75, 75, 255]));
    }

    let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
    assert_eq!(
        (bg_r as u8, bg_g as u8, bg_b as u8),
        (30, 30, 30),
        "Must correctly identify dark editor background despite 6 vertical guides"
    );

    let mut prep = img.clone();
    enhance_contrast(&mut prep);

    let bands = detect_horizontal_line_bands(&prep);
    println!(
        "[DENSE GUIDES OBS] Detected {} bands with 6 dense vertical guides:",
        bands.len()
    );
    for (i, (y0, y1)) in bands.iter().enumerate() {
        println!("  Band {}: y={}..{} (h={}px)", i, y0, y1, y1 - y0);
    }
    assert_eq!(
        bands.len(),
        6,
        "Must cleanly segment all 6 lines without bridging across any of the 6 dense guide lines"
    );
}

// ============================================================================
// 3. ADVERSARIAL STRESS: SELECTION BORDER RECTANGLE OF VARIOUS THICKNESS & COLORS
// ============================================================================

#[test]
fn test_stress_selection_border_detection_and_background_isolation() {
    let border_colors = [
        Rgba([59, 130, 246, 255]),  // Blue #3B82F6 (standard Windows selection)
        Rgba([255, 255, 255, 255]), // White #FFFFFF
        Rgba([34, 197, 94, 255]),   // Green #22C55E
        Rgba([249, 115, 22, 255]),  // Orange #F97316
    ];

    for thickness in 1..=3u32 {
        for &border_color in &border_colors {
            let (w, h) = (200u32, 120u32);
            // Dark editor background #1E1E1E (30, 30, 30)
            let mut img = RgbaImage::from_pixel(w, h, Rgba([30, 30, 30, 255]));

            // Draw outer border of given thickness
            for b in 0..thickness {
                for x in 0..w {
                    img.put_pixel(x, b, border_color);
                    img.put_pixel(x, h - 1 - b, border_color);
                }
                for y in 0..h {
                    img.put_pixel(b, y, border_color);
                    img.put_pixel(w - 1 - b, y, border_color);
                }
            }

            let detected = detect_solid_border_thickness(&img);
            assert_eq!(
                detected,
                thickness,
                "Failed to detect border thickness {} for color {:?}",
                thickness,
                border_color
            );

            let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
            assert_eq!(
                (bg_r as u8, bg_g as u8, bg_b as u8),
                (30, 30, 30),
                "Background estimation must not be contaminated by {}-pixel {:?} border",
                thickness,
                border_color
            );
        }
    }
}

// ============================================================================
// 4. ADVERSARIAL STRESS: LOW CONTRAST COLOR DIFFERENCE THRESHOLDS & INK PROFILE
// ============================================================================

#[test]
fn test_stress_low_contrast_threshold_boundary() {
    let (w, h) = (200u32, 100u32);
    // Dark background #1A1A1A (26, 26, 26)
    let mut img = RgbaImage::from_pixel(w, h, Rgba([26, 26, 26, 255]));

    // Line 1: Salmon/peach syntax keyword (66, 26, 26) -> chromatic diff = |66 - 26| = 40.0
    // Notice: Due to ch_diff * 0.55 damping in enhance_contrast, diff = 22.0, mapping to transition ink (157)
    for y in 20..35 {
        for x in 20..180 {
            img.put_pixel(x, y, Rgba([66, 26, 26, 255]));
        }
    }

    // Line 2: Green syntax keyword (26, 96, 26) -> chromatic diff = |96 - 26| = 70.0
    // With 0.587 green luma, diff = 41.0 > 38.0, mapping to solid black ink (0)
    for y in 55..70 {
        for x in 20..180 {
            img.put_pixel(x, y, Rgba([26, 96, 26, 255]));
        }
    }

    let (bg_r, bg_g, bg_b) = estimate_background_color(&img);
    assert_eq!((bg_r as u8, bg_g as u8, bg_b as u8), (26, 26, 26));

    enhance_contrast(&mut img);

    // Line 1 transition ink is 157 (which is < 160 ink threshold in detect_horizontal_line_bands)
    let p_line1 = img.get_pixel(30, 25)[0];
    assert!(
        p_line1 < 160,
        "Line 1 transition ink ({}) must be below the 160 ink threshold",
        p_line1
    );

    // Line 2 green keyword binarizes to solid black ink (0)
    assert_eq!(img.get_pixel(30, 60)[0], 0, "Line 2 must be solid black ink");

    let bands = detect_horizontal_line_bands(&img);
    assert_eq!(
        bands.len(),
        2,
        "Must segment both low-contrast lines (got {:?})",
        bands
    );
}

// ============================================================================
// 5. ADVERSARIAL STRESS: DIGIT HEAD DILATION AND ASCENDER PRESERVATION
// ============================================================================

#[test]
fn test_stress_ascender_digit_head_preservation() {
    // Simulate numeral '0' and '2' where the top 2 rows have only 3px ink (curved top of numeral)
    let (w, h) = (200u32, 80u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // Line 0: Numeral head at y=15..17 (3px ink per row: x=50..53)
    for y in 15..17 {
        for x in 50..53 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    // Main body at y=17..31 (30px ink per row: x=40..70)
    for y in 17..31 {
        for x in 40..70 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }

    let bands = detect_horizontal_line_bands(&img);
    assert_eq!(bands.len(), 1, "Must detect exactly 1 line band");
    let (y0, y1) = bands[0];
    println!("[ASCENDER OBS] Detected band: y={}..{} (expected top <= 15)", y0, y1);
    assert!(
        y0 <= 15,
        "Band top ({}) must include the numeral head at y=15, not clipping it!",
        y0
    );
    assert!(
        y1 >= 31,
        "Band bottom ({}) must include the numeral base at y=31",
        y1
    );
}
