use desklens_lib::ocr::{
    compute_words_bounding_box, parse_tesseract_tsv, OcrWord,
};
use desklens_lib::translation::{
    normalize_target_lang, translate_on_screen, TranslationBoxInput,
};

// =============================================================================
// ADVERSARIAL TEST SUITE 1: OCR BOUNDING BOX & TSV PARSER
// =============================================================================

#[test]
fn test_adv_compute_words_bounding_box_edge_cases() {
    // 1. Empty words slice
    assert_eq!(
        compute_words_bounding_box(&[]),
        (0.0, 0.0, 0.0, 0.0),
        "Empty words must return (0, 0, 0, 0)"
    );

    // 2. Single word
    let single = vec![OcrWord {
        text: "Solo".to_string(),
        x: 42.0,
        y: 84.0,
        width: 100.0,
        height: 20.0,
    }];
    assert_eq!(
        compute_words_bounding_box(&single),
        (42.0, 84.0, 100.0, 20.0)
    );

    // 3. Negative coordinates (e.g. offscreen or relative coordinate shifts)
    let negative = vec![
        OcrWord {
            text: "Left".to_string(),
            x: -20.0,
            y: -10.0,
            width: 30.0,
            height: 15.0,
        },
        OcrWord {
            text: "Right".to_string(),
            x: 20.0,
            y: 5.0,
            width: 40.0,
            height: 25.0,
        },
    ];
    let (nx, ny, nw, nh) = compute_words_bounding_box(&negative);
    assert_eq!(nx, -20.0);
    assert_eq!(ny, -10.0);
    assert_eq!(nw, 80.0); // max_x = 20 + 40 = 60; 60 - (-20) = 80
    assert_eq!(nh, 40.0); // max_y = 5 + 25 = 30; 30 - (-10) = 40

    // 4. Zero-width and zero-height words
    let zero_dims = vec![
        OcrWord {
            text: "Dot1".to_string(),
            x: 50.0,
            y: 50.0,
            width: 0.0,
            height: 0.0,
        },
        OcrWord {
            text: "Dot2".to_string(),
            x: 100.0,
            y: 80.0,
            width: 0.0,
            height: 0.0,
        },
    ];
    let (zx, zy, zw, zh) = compute_words_bounding_box(&zero_dims);
    assert_eq!(zx, 50.0);
    assert_eq!(zy, 50.0);
    assert_eq!(zw, 50.0);
    assert_eq!(zh, 30.0);

    // 5. Completely overlapping identical words
    let overlapping = vec![
        OcrWord {
            text: "Same".to_string(),
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 20.0,
        },
        OcrWord {
            text: "Same".to_string(),
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 20.0,
        },
    ];
    assert_eq!(
        compute_words_bounding_box(&overlapping),
        (10.0, 10.0, 50.0, 20.0)
    );
}

#[test]
fn test_adv_parse_tesseract_tsv_empty_and_whitespace() {
    // 1. Completely empty string
    let res = parse_tesseract_tsv("");
    assert_eq!(res.word_count, 0);
    assert!(res.lines.is_empty());
    assert!(res.boxes.is_empty());
    assert_eq!(res.full_text, "");

    // 2. Whitespace only, empty lines, and tabs
    let res2 = parse_tesseract_tsv("   \n\t\t\t\n\r\n   \n");
    assert_eq!(res2.word_count, 0);
    assert!(res2.lines.is_empty());
    assert!(res2.boxes.is_empty());
    assert_eq!(res2.full_text, "");

    // 3. Header-only TSV
    let header_only = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext";
    let res3 = parse_tesseract_tsv(header_only);
    assert_eq!(res3.word_count, 0);
    assert!(res3.lines.is_empty());
    assert!(res3.boxes.is_empty());

    // 4. Header with trailing blank lines
    let header_blanks = format!("{}\n\n\n\t\t\n", header_only);
    let res4 = parse_tesseract_tsv(&header_blanks);
    assert_eq!(res4.word_count, 0);
    assert!(res4.lines.is_empty());
}

#[test]
fn test_adv_parse_tesseract_tsv_malformed_and_fuzz() {
    let malformed_tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
not_a_number\n\
5\t1\tshort\tline\n\
4\t1\t1\t1\t1\t0\tabc\tdef\tghi\tjkl\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t20\t30\t15\t95\tValidWord\n\
garbage line without any tabs whatsoever\n\
\t\t\t\t\t\t\t\t\t\t\t\n\
5\t1\t1\t1\t1\t2\t45\t20\t40\t15\t98\tAnotherValid\n\
5\t1\t1\t1\t1\t3\t100\t20\t50\t15\t90\t   \n"; // empty word text

    let res = parse_tesseract_tsv(malformed_tsv);
    // Should parse only the 2 valid words, skipping malformed, short, and empty lines
    assert_eq!(res.word_count, 2);
    assert_eq!(res.lines.len(), 1);
    assert_eq!(res.lines[0].text, "ValidWord AnotherValid");
    assert_eq!(res.boxes.len(), 1);
    assert_eq!(res.boxes[0].text, "ValidWord AnotherValid");
}

#[test]
fn test_adv_parse_tesseract_tsv_missing_level_4() {
    // Missing Level 4: TSV contains ONLY Level 5 words, no Level 4 line record
    let tsv_no_lvl4 = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t50\t100\t80\t30\t95\tFirst\n\
5\t1\t1\t1\t1\t2\t140\t95\t90\t40\t92\tSecond\n";

    let res = parse_tesseract_tsv(tsv_no_lvl4);
    assert_eq!(res.word_count, 2);
    assert_eq!(res.lines.len(), 1);
    assert_eq!(res.lines[0].text, "First Second");

    // Must compute fallback union bounding box:
    // min_x = 50, min_y = 95
    // max_x = 140 + 90 = 230 -> width = 230 - 50 = 180
    // max_y = 95 + 40 = 135 (vs 100+30=130) -> height = 135 - 95 = 40
    assert_eq!(res.lines[0].x, 50.0);
    assert_eq!(res.lines[0].y, 95.0);
    assert_eq!(res.lines[0].width, 180.0);
    assert_eq!(res.lines[0].height, 40.0);

    assert_eq!(res.boxes.len(), 1);
    assert_eq!(res.boxes[0].x, 50.0);
    assert_eq!(res.boxes[0].y, 95.0);
    assert_eq!(res.boxes[0].width, 180.0);
    assert_eq!(res.boxes[0].height, 40.0);
}

#[test]
fn test_adv_parse_tesseract_tsv_zero_and_negative_dimensions() {
    // Level 4 exists but has zero width and height (w=0, h=0)
    // Should trigger fallback to union of Level 5 words
    let tsv_zero_l4 = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t0\t0\t0\t0\t-1\t\n\
5\t1\t1\t1\t1\t1\t30\t40\t50\t20\t95\tAlpha\n\
5\t1\t1\t1\t1\t2\t90\t40\t60\t20\t95\tBeta\n";

    let res = parse_tesseract_tsv(tsv_zero_l4);
    assert_eq!(res.lines.len(), 1);
    assert_eq!(res.lines[0].x, 30.0);
    assert_eq!(res.lines[0].y, 40.0);
    assert_eq!(res.lines[0].width, 120.0); // 90+60-30 = 120
    assert_eq!(res.lines[0].height, 20.0);
}

#[test]
fn test_adv_parse_tesseract_tsv_non_ascii_emojis_special_chars() {
    let tsv_unicode = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t10\t10\t300\t30\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t80\t30\t95\tConfiguração\n\
5\t1\t1\t1\t1\t2\t95\t10\t90\t30\t94\tAtenção!\n\
5\t1\t1\t1\t1\t3\t190\t10\t50\t30\t93\t🚀🔥\n\
4\t1\t1\t1\t2\t0\t10\t50\t350\t30\t-1\t\n\
5\t1\t1\t1\t2\t1\t10\t50\t120\t30\t90\t<div id=\"lens\">\n\
5\t1\t1\t1\t2\t2\t135\t50\t80\t30\t91\t100% & $50\n";

    let res = parse_tesseract_tsv(tsv_unicode);
    assert_eq!(res.lines.len(), 2);
    assert_eq!(res.lines[0].text, "Configuração Atenção! 🚀🔥");
    assert_eq!(res.lines[1].text, "<div id=\"lens\"> 100% & $50");
    assert_eq!(res.word_count, 5);
    assert_eq!(res.boxes.len(), 2);
    assert_eq!(res.boxes[0].text, "Configuração Atenção! 🚀🔥");
    assert_eq!(res.boxes[1].text, "<div id=\"lens\"> 100% & $50");
}

#[test]
fn test_adv_parse_tesseract_tsv_out_of_order_blocks() {
    // Scrambled block and line numbers: Block 2 appears before Block 1
    let tsv_scrambled = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t2\t1\t1\t0\t10\t100\t100\t20\t-1\t\n\
5\t1\t2\t1\t1\t1\t10\t100\t100\t20\t90\tSecondBlockLine\n\
4\t1\t1\t1\t1\t0\t10\t20\t100\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t20\t100\t20\t95\tFirstBlockLine\n";

    let res = parse_tesseract_tsv(tsv_scrambled);
    assert_eq!(res.lines.len(), 2);
    // BTreeMap key is (block_num, par_num, line_num) -> (1, 1, 1) should precede (2, 1, 1)
    assert_eq!(res.lines[0].text, "FirstBlockLine");
    assert_eq!(res.lines[1].text, "SecondBlockLine");
}

#[test]
fn test_adv_parse_tesseract_tsv_scale_stress() {
    // Stress test: generate 1000 lines of TSV to verify parsing throughput and memory
    let mut big_tsv = String::from("level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n");
    for i in 1..=500 {
        big_tsv.push_str(&format!("4\t1\t1\t1\t{i}\t0\t10\t{y}\t400\t20\t-1\t\n", y = i * 25));
        big_tsv.push_str(&format!("5\t1\t1\t1\t{i}\t1\t10\t{y}\t150\t20\t95\tLineWordA_{i}\n", y = i * 25));
        big_tsv.push_str(&format!("5\t1\t1\t1\t{i}\t2\t170\t{y}\t180\t20\t95\tLineWordB_{i}\n", y = i * 25));
    }

    let start = std::time::Instant::now();
    let res = parse_tesseract_tsv(&big_tsv);
    let elapsed = start.elapsed();

    assert_eq!(res.lines.len(), 500);
    assert_eq!(res.boxes.len(), 500);
    assert_eq!(res.word_count, 1000);
    assert!(
        elapsed.as_millis() < 100,
        "Parsing 1500 TSV rows took {}ms, should be <100ms",
        elapsed.as_millis()
    );
}

// =============================================================================
// ADVERSARIAL TEST SUITE 2: TRANSLATION LOGIC & NORMALIZATION
// =============================================================================

#[test]
fn test_adv_language_normalization_standard() {
    assert_eq!(normalize_target_lang(None), "pt");
    assert_eq!(normalize_target_lang(Some("")), "pt");
    assert_eq!(normalize_target_lang(Some("   ")), "pt");
    assert_eq!(normalize_target_lang(Some("pt-BR")), "pt");
    assert_eq!(normalize_target_lang(Some("pt_BR")), "pt");
    assert_eq!(normalize_target_lang(Some("PT")), "pt");
    assert_eq!(normalize_target_lang(Some("PT-PT")), "pt");
    assert_eq!(normalize_target_lang(Some("es")), "es");
    assert_eq!(normalize_target_lang(Some("es-AR")), "es");
    assert_eq!(normalize_target_lang(Some("fr")), "fr");
    assert_eq!(normalize_target_lang(Some("en-US")), "en");
    assert_eq!(normalize_target_lang(Some("unknown")), "un");
    assert_eq!(normalize_target_lang(Some("x")), "x");
}

#[test]
fn test_adv_language_normalization_adversarial_utf8() {
    // Standard 2-character ASCII codes must succeed
    let test_inputs = vec!["pt-br", "PORTUGUÊS", "DE", "JA", "RU"];
    for input in test_inputs {
        let normalized = normalize_target_lang(Some(input));
        assert!(!normalized.is_empty());
    }

    // EMPIRICAL VULNERABILITY INVESTIGATION:
    // Testing behavior on 3-byte and 4-byte multi-byte UTF-8 inputs (Chinese, Japanese Kanji, Emoji).
    // In Rust, lower[..2] slices by BYTE index. For a 3-byte character, byte index 2 is inside
    // the codepoint, causing a panic: "end byte index 2 is not a char boundary".
    let panic_result_chinese = std::panic::catch_unwind(|| {
        normalize_target_lang(Some("中文"))
    });
    let panic_result_emoji = std::panic::catch_unwind(|| {
        normalize_target_lang(Some("🚀"))
    });

    println!(
        "🔍 [Adversarial Vulnerability Probe] '中文' panicked? {}",
        panic_result_chinese.is_err()
    );
    println!(
        "🔍 [Adversarial Vulnerability Probe] '🚀' panicked? {}",
        panic_result_emoji.is_err()
    );

    // Multi-byte UTF-8 inputs must execute safely without panicking:
    assert!(
        panic_result_chinese.is_ok(),
        "normalize_target_lang must not panic on multi-byte UTF-8 input '中文'"
    );
    assert_eq!(panic_result_chinese.unwrap(), "中文");

    assert!(
        panic_result_emoji.is_ok(),
        "normalize_target_lang must not panic on 4-byte emoji input '🚀'"
    );
    assert_eq!(panic_result_emoji.unwrap(), "🚀");
}

#[test]
fn test_adv_translate_empty_boxes() {
    tauri::async_runtime::block_on(async {
        let res = translate_on_screen(vec![], Some("pt-BR".to_string()), None).await;
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![]);
    });
}

#[test]
fn test_adv_translate_large_batch_chunking() {
    // SPRINT R2 CHALLENGE: Batch size > 50 items (testing multi-chunk batching: 30 + 30 + 5)
    const TOTAL_ITEMS: usize = 65;

    tauri::async_runtime::block_on(async {
        let mut input_boxes = Vec::with_capacity(TOTAL_ITEMS);
        for i in 0..TOTAL_ITEMS {
            input_boxes.push(TranslationBoxInput {
                text: format!("DeskLens Item {i}"),
                x: 10.0 + (i as f64) * 5.0,
                y: 20.0 + (i as f64) * 20.0,
                width: 150.0,
                height: 18.0,
            });
        }

        let start = std::time::Instant::now();
        let res = translate_on_screen(
            input_boxes.clone(),
            Some("pt-BR".to_string()),
            Some("en".to_string()),
        )
        .await;
        let elapsed = start.elapsed();

        assert!(res.is_ok(), "Batch translation must succeed: {:?}", res.err());
        let outputs = res.unwrap();

        assert_eq!(
            outputs.len(),
            TOTAL_ITEMS,
            "Must return exactly {TOTAL_ITEMS} outputs matching input count"
        );

        // Verify strictly that every single coordinate and text position is preserved
        for (i, (inp, out)) in input_boxes.iter().zip(outputs.iter()).enumerate() {
            assert_eq!(out.x, inp.x, "Coordinate x mismatch at item {i}");
            assert_eq!(out.y, inp.y, "Coordinate y mismatch at item {i}");
            assert_eq!(out.width, inp.width, "Width mismatch at item {i}");
            assert_eq!(out.height, inp.height, "Height mismatch at item {i}");
            assert_eq!(out.text, inp.text, "Text mismatch at item {i}");
            assert!(
                !out.translated_text.is_empty(),
                "translated_text at item {i} must not be empty"
            );
        }

        println!(
            "✅ [Adversarial] Translated {TOTAL_ITEMS} items across 3 chunks in {:.2}s",
            elapsed.as_secs_f64()
        );
    });
}

#[test]
fn test_adv_translate_special_characters_and_html() {
    tauri::async_runtime::block_on(async {
        let input_boxes = vec![
            TranslationBoxInput {
                text: "<button class=\"btn\">Click Here & Now</button>".to_string(),
                x: 10.0,
                y: 10.0,
                width: 250.0,
                height: 35.0,
            },
            TranslationBoxInput {
                text: "100% Satisfaction Guaranteed @ $9.99!".to_string(),
                x: 10.0,
                y: 50.0,
                width: 300.0,
                height: 30.0,
            },
            TranslationBoxInput {
                text: "DeskLens 🚀 Screen Translation 🔍".to_string(),
                x: 10.0,
                y: 90.0,
                width: 280.0,
                height: 30.0,
            },
        ];

        let res = translate_on_screen(input_boxes.clone(), Some("pt".to_string()), None).await;
        assert!(res.is_ok());
        let outputs = res.unwrap();
        assert_eq!(outputs.len(), 3);

        for (i, out) in outputs.iter().enumerate() {
            assert_eq!(out.x, input_boxes[i].x);
            assert_eq!(out.y, input_boxes[i].y);
            assert!(!out.translated_text.is_empty());
            println!("Special chars [{i}] translated to: '{}'", out.translated_text);
        }
    });
}

#[test]
fn test_adv_translate_network_failure_fallback() {
    // SPRINT R2 CHALLENGE: Network failure / timeout simulation.
    // Verify that translate_on_screen NEVER crashes, panics, or returns Err when network fails.
    // Instead, it must return Ok(boxes) with translated_text populated with fallback (original text).

    // We can simulate an unreachable network condition by pointing HTTP proxies to a closed port on loopback:
    std::env::set_var("HTTP_PROXY", "http://127.0.0.1:54321");
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:54321");
    std::env::set_var("ALL_PROXY", "http://127.0.0.1:54321");

    tauri::async_runtime::block_on(async {
        let input_boxes = vec![
            TranslationBoxInput {
                text: "Offline Text Line 1".to_string(),
                x: 25.0,
                y: 50.0,
                width: 180.0,
                height: 25.0,
            },
            TranslationBoxInput {
                text: "Offline Text Line 2".to_string(),
                x: 25.0,
                y: 80.0,
                width: 200.0,
                height: 25.0,
            },
        ];

        let res = translate_on_screen(
            input_boxes.clone(),
            Some("pt".to_string()),
            Some("auto".to_string()),
        )
        .await;

        // Clean up environment variables immediately
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("ALL_PROXY");

        assert!(
            res.is_ok(),
            "translate_on_screen MUST NOT fail or panic on network error: {:?}",
            res.err()
        );
        let outputs = res.unwrap();
        assert_eq!(outputs.len(), 2);

        // Under total network failure, translated_text must fall back to original text
        assert_eq!(outputs[0].text, "Offline Text Line 1");
        assert_eq!(outputs[0].translated_text, "Offline Text Line 1");
        assert_eq!(outputs[0].x, 25.0);
        assert_eq!(outputs[0].y, 50.0);

        assert_eq!(outputs[1].text, "Offline Text Line 2");
        assert_eq!(outputs[1].translated_text, "Offline Text Line 2");
        assert_eq!(outputs[1].x, 25.0);
        assert_eq!(outputs[1].y, 80.0);

        println!("✅ [Adversarial] Offline network fallback verified: translated_text preserved as original text.");
    });
}
