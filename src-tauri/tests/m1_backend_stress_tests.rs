//! M1 Backend & Translation Adversarial Challenger Stress Harness
//!
//! Empirically stress-tests:
//! 1. System Tray icon assets (decoding, RGBA8 conversion, Tauri Image construction, corrupt input resilience).
//! 2. Translation normalization boundary & adversarial test matrix.
//! 3. Google Translate dual-format JSON response parser resilience against adversarial payloads.
//! 4. End-to-end translation command edge cases.

use desklens_lib::translation::{
    normalize_source_lang, normalize_target_lang, translate_on_screen, url_encode,
    TranslationBoxInput,
};

// =============================================================================
// 1. TRAY ICON ASSETS & TAURI IMAGE STRESS TESTS
// =============================================================================

#[test]
fn test_stress_tray_icon_assets_128_and_32_decoding() {
    // 128x128.png
    let raw_128 = include_bytes!("../icons/128x128.png");
    assert!(!raw_128.is_empty(), "128x128.png asset must not be empty");
    let img_128 = image::load_from_memory(raw_128)
        .expect("128x128.png must decode successfully from memory");
    assert_eq!(img_128.width(), 128, "128x128 width must be exactly 128");
    assert_eq!(img_128.height(), 128, "128x128 height must be exactly 128");

    let rgba_128 = img_128.to_rgba8();
    assert_eq!(
        rgba_128.as_raw().len(),
        128 * 128 * 4,
        "128x128 RGBA8 buffer must have exactly 65,536 bytes"
    );

    // 32x32.png
    let raw_32 = include_bytes!("../icons/32x32.png");
    assert!(!raw_32.is_empty(), "32x32.png asset must not be empty");
    let img_32 = image::load_from_memory(raw_32)
        .expect("32x32.png must decode successfully from memory");
    assert_eq!(img_32.width(), 32, "32x32 width must be exactly 32");
    assert_eq!(img_32.height(), 32, "32x32 height must be exactly 32");

    let rgba_32 = img_32.to_rgba8();
    assert_eq!(
        rgba_32.as_raw().len(),
        32 * 32 * 4,
        "32x32 RGBA8 buffer must have exactly 4,096 bytes"
    );
}

#[test]
fn test_stress_tauri_image_construction() {
    // Test constructing tauri::image::Image from decoded 128x128 asset
    let raw_128 = include_bytes!("../icons/128x128.png");
    let img_128 = image::load_from_memory(raw_128).unwrap();
    let rgba_128 = img_128.to_rgba8();
    let (w128, h128) = (rgba_128.width(), rgba_128.height());
    let tauri_img_128 = tauri::image::Image::new_owned(rgba_128.into_raw(), w128, h128);
    assert_eq!(tauri_img_128.width(), 128);
    assert_eq!(tauri_img_128.height(), 128);
    assert_eq!(tauri_img_128.rgba().len(), 128 * 128 * 4);

    // Test constructing tauri::image::Image from decoded 32x32 asset
    let raw_32 = include_bytes!("../icons/32x32.png");
    let img_32 = image::load_from_memory(raw_32).unwrap();
    let rgba_32 = img_32.to_rgba8();
    let (w32, h32) = (rgba_32.width(), rgba_32.height());
    let tauri_img_32 = tauri::image::Image::new_owned(rgba_32.into_raw(), w32, h32);
    assert_eq!(tauri_img_32.width(), 32);
    assert_eq!(tauri_img_32.height(), 32);
    assert_eq!(tauri_img_32.rgba().len(), 32 * 32 * 4);
}

#[test]
fn test_stress_tray_icon_corrupt_data() {
    // 1. Truncated PNG header
    let truncated_png = b"\x89PNG\r\n\x1a\n";
    assert!(image::load_from_memory(truncated_png).is_err());

    // 2. All zeros
    let zeros = vec![0u8; 1024];
    assert!(image::load_from_memory(&zeros).is_err());

    // 3. Random noise
    let garbage = b"THIS IS NOT A VALID PNG IMAGE AT ALL 1234567890!@#$%^&*()";
    assert!(image::load_from_memory(garbage).is_err());

    // 4. Empty slice
    assert!(image::load_from_memory(&[]).is_err());
}

#[test]
fn test_stress_tray_fallback_logic_reproduction() {
    // Replicates exact fallback logic from src-tauri/src/lib.rs:1067-1073:
    // let tray_icon = app.default_window_icon().cloned().unwrap_or_else(|| {
    //     let raw_bytes = include_bytes!("../icons/128x128.png");
    //     let img = image::load_from_memory(raw_bytes).expect("Valid 128x128 tray icon asset");
    //     let rgba = img.to_rgba8();
    //     let (w, h) = (rgba.width(), rgba.height());
    //     tauri::image::Image::new_owned(rgba.into_raw(), w, h)
    // });
    let fallback = || -> tauri::image::Image<'static> {
        let raw_bytes = include_bytes!("../icons/128x128.png");
        let img = image::load_from_memory(raw_bytes).expect("Valid 128x128 tray icon asset");
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        tauri::image::Image::new_owned(rgba.into_raw(), w, h)
    };

    let tray_icon = fallback();
    assert_eq!(tray_icon.width(), 128);
    assert_eq!(tray_icon.height(), 128);
    assert_eq!(tray_icon.rgba().len(), 128 * 128 * 4);
}

// =============================================================================
// 2. TRANSLATION NORMALIZATION ADVERSARIAL MATRIX
// =============================================================================

#[test]
fn test_stress_normalize_target_lang_matrix() {
    // Nil / Empty / Whitespace -> default "pt"
    assert_eq!(normalize_target_lang(None), "pt");
    assert_eq!(normalize_target_lang(Some("")), "pt");
    assert_eq!(normalize_target_lang(Some("   ")), "pt");
    assert_eq!(normalize_target_lang(Some("\t\n\r")), "pt");
    assert_eq!(normalize_target_lang(Some("  \t  \n  ")), "pt");

    // Standard & Regional codes
    assert_eq!(normalize_target_lang(Some("pt")), "pt");
    assert_eq!(normalize_target_lang(Some("PT")), "pt");
    assert_eq!(normalize_target_lang(Some("pt-BR")), "pt");
    assert_eq!(normalize_target_lang(Some("PT-BR")), "pt");
    assert_eq!(normalize_target_lang(Some("pt_BR")), "pt");
    assert_eq!(normalize_target_lang(Some("pt-PT")), "pt");
    assert_eq!(normalize_target_lang(Some("en")), "en");
    assert_eq!(normalize_target_lang(Some("EN")), "en");
    assert_eq!(normalize_target_lang(Some("en-US")), "en");
    assert_eq!(normalize_target_lang(Some("EN-GB")), "en");
    assert_eq!(normalize_target_lang(Some("es")), "es");
    assert_eq!(normalize_target_lang(Some("ES")), "es");
    assert_eq!(normalize_target_lang(Some("es-MX")), "es");
    assert_eq!(normalize_target_lang(Some("fr")), "fr");
    assert_eq!(normalize_target_lang(Some("FR-CA")), "fr");
    assert_eq!(normalize_target_lang(Some("de")), "de");
    assert_eq!(normalize_target_lang(Some("ja")), "ja");
    assert_eq!(normalize_target_lang(Some("zh")), "zh");
    assert_eq!(normalize_target_lang(Some("zh-CN")), "zh");

    // Padding & whitespace trimming
    assert_eq!(normalize_target_lang(Some("  en  ")), "en");
    assert_eq!(normalize_target_lang(Some("  pt-br \n")), "pt");

    // Unsupported / arbitrary codes (extracts first 2 chars)
    assert_eq!(normalize_target_lang(Some("klingon")), "kl");
    assert_eq!(normalize_target_lang(Some("esperanto")), "es"); // starts with es -> "es"
    assert_eq!(normalize_target_lang(Some("xx")), "xx");

    // Numeric & Punctuation
    assert_eq!(normalize_target_lang(Some("123")), "12");
    assert_eq!(normalize_target_lang(Some("!@#$%^")), "!@");
    assert_eq!(normalize_target_lang(Some("a")), "a"); // single char preserved

    // Unicode multi-byte (ensure chars().take(2) does NOT panic on multibyte UTF-8)
    assert_eq!(normalize_target_lang(Some("日本語")), "日本");
    assert_eq!(normalize_target_lang(Some("العربية")), "ال");
    assert_eq!(normalize_target_lang(Some("💡")), "💡");
    assert_eq!(normalize_target_lang(Some("💡🔥")), "💡🔥");

    // Very long string
    let long_str = "a".repeat(10_000);
    assert_eq!(normalize_target_lang(Some(&long_str)), "aa");
}

#[test]
fn test_stress_normalize_source_lang_matrix() {
    // Nil / Empty / Whitespace -> default "auto"
    assert_eq!(normalize_source_lang(None), "auto");
    assert_eq!(normalize_source_lang(Some("")), "auto");
    assert_eq!(normalize_source_lang(Some("   ")), "auto");
    assert_eq!(normalize_source_lang(Some("\t\r\n")), "auto");

    // "auto" variants
    assert_eq!(normalize_source_lang(Some("auto")), "auto");
    assert_eq!(normalize_source_lang(Some("AUTO")), "auto");
    assert_eq!(normalize_source_lang(Some("  auto  ")), "auto");

    // Standard & Regional codes
    assert_eq!(normalize_source_lang(Some("en")), "en");
    assert_eq!(normalize_source_lang(Some("EN")), "en");
    assert_eq!(normalize_source_lang(Some("en-US")), "en");
    assert_eq!(normalize_source_lang(Some("EN-GB")), "en");
    assert_eq!(normalize_source_lang(Some("pt")), "pt");
    assert_eq!(normalize_source_lang(Some("PT")), "pt");
    assert_eq!(normalize_source_lang(Some("pt-BR")), "pt");
    assert_eq!(normalize_source_lang(Some("es")), "es");
    assert_eq!(normalize_source_lang(Some("ES")), "es");
    assert_eq!(normalize_source_lang(Some("es-ES")), "es");
    assert_eq!(normalize_source_lang(Some("fr")), "fr");
    assert_eq!(normalize_source_lang(Some("de")), "de");
    assert_eq!(normalize_source_lang(Some("ja")), "ja");
    assert_eq!(normalize_source_lang(Some("zh")), "zh");

    // Padding & whitespace trimming
    assert_eq!(normalize_source_lang(Some("  en-US  ")), "en");
    assert_eq!(normalize_source_lang(Some("  es  ")), "es");

    // Unsupported & arbitrary codes
    assert_eq!(normalize_source_lang(Some("klingon")), "kl");
    assert_eq!(normalize_source_lang(Some("xx")), "xx");

    // Numeric & Punctuation
    assert_eq!(normalize_source_lang(Some("123")), "12");
    assert_eq!(normalize_source_lang(Some("!@#$%^")), "!@");
    assert_eq!(normalize_source_lang(Some("a")), "a");

    // Unicode multi-byte (no UTF-8 slicing panics)
    assert_eq!(normalize_source_lang(Some("日本語")), "日本");
    assert_eq!(normalize_source_lang(Some("العربية")), "ال");
    assert_eq!(normalize_source_lang(Some("💡")), "💡");
    assert_eq!(normalize_source_lang(Some("💡🔥")), "💡🔥");

    // Very long string
    let long_str = "x".repeat(10_000);
    assert_eq!(normalize_source_lang(Some(&long_str)), "xx");
}

#[test]
fn test_stress_url_encoder_adversarial() {
    assert_eq!(url_encode(""), "");
    assert_eq!(url_encode("abcXYZ123-_.~"), "abcXYZ123-_.~");
    assert_eq!(url_encode(" "), "%20");
    assert_eq!(url_encode("   "), "%20%20%20");
    assert_eq!(url_encode("\t\n"), "%09%0A");
    assert_eq!(url_encode("Hello World!"), "Hello%20World%21");
    assert_eq!(url_encode("a+b=c&d?e#f"), "a%2Bb%3Dc%26d%3Fe%23f");
    assert_eq!(url_encode("100%"), "100%25");
    // UTF-8 multibyte encoding
    assert_eq!(url_encode("Olá"), "Ol%C3%A1");
    assert_eq!(url_encode("日本語"), "%E6%97%A5%E6%9C%AC%E8%AA%9E");
}

// =============================================================================
// 3. DUAL-FORMAT JSON PARSER ADVERSARIAL HARNESS
// =============================================================================

/// Exact reproduction of Google Translate chunk parsing logic in src-tauri/src/translation.rs:131-164
fn parse_google_translate_response_oracle(
    body: &str,
    texts: &[String],
) -> Result<Vec<String>, String> {
    let json: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| format!("JSON parse failed: {e} - body: {body}"))?;

    let array = json
        .as_array()
        .ok_or_else(|| "Google Translate response is not an array".to_string())?;

    let mut translated = Vec::with_capacity(texts.len());
    for (idx, item) in array.iter().enumerate() {
        if let Some(pair) = item.as_array() {
            if let Some(trans_str) = pair.first().and_then(|v| v.as_str()) {
                if trans_str.trim().is_empty() {
                    translated.push(texts.get(idx).cloned().unwrap_or_default());
                } else {
                    translated.push(trans_str.to_string());
                }
                continue;
            }
        } else if let Some(trans_str) = item.as_str() {
            if trans_str.trim().is_empty() {
                translated.push(texts.get(idx).cloned().unwrap_or_default());
            } else {
                translated.push(trans_str.to_string());
            }
            continue;
        }
        translated.push(texts.get(idx).cloned().unwrap_or_default());
    }

    while translated.len() < texts.len() {
        translated.push(texts[translated.len()].clone());
    }

    Ok(translated)
}

#[test]
fn test_stress_dual_format_json_parser_standard_cases() {
    let inputs = vec!["Hello".to_string(), "World".to_string()];

    // Case 1: Auto-mode (pairs with detected language: [["Olá", "en"], ["Mundo", "en"]])
    let json_pairs = r#"[["Olá", "en"], ["Mundo", "en"]]"#;
    let res1 = parse_google_translate_response_oracle(json_pairs, &inputs).unwrap();
    assert_eq!(res1, vec!["Olá", "Mundo"]);

    // Case 2: Explicit mode (flat strings: ["Olá", "Mundo"])
    let json_flat = r#"["Olá", "Mundo"]"#;
    let res2 = parse_google_translate_response_oracle(json_flat, &inputs).unwrap();
    assert_eq!(res2, vec!["Olá", "Mundo"]);

    // Case 3: Mixed format ([["Olá", "en"], "Mundo"])
    let json_mixed = r#"[["Olá", "en"], "Mundo"]"#;
    let res3 = parse_google_translate_response_oracle(json_mixed, &inputs).unwrap();
    assert_eq!(res3, vec!["Olá", "Mundo"]);
}

#[test]
fn test_stress_dual_format_json_parser_empty_and_null_adversarial() {
    let inputs = vec![
        "Text 1".to_string(),
        "Text 2".to_string(),
        "Text 3".to_string(),
    ];

    // Adversarial 1: Empty array []
    let json_empty = "[]";
    let res_empty = parse_google_translate_response_oracle(json_empty, &inputs).unwrap();
    assert_eq!(res_empty, inputs, "Empty response array must fallback to original inputs");

    // Adversarial 2: Null elements inside array
    let json_nulls = r#"[null, ["Translated 2", "en"], null]"#;
    let res_nulls = parse_google_translate_response_oracle(json_nulls, &inputs).unwrap();
    assert_eq!(res_nulls, vec!["Text 1", "Translated 2", "Text 3"]);

    // Adversarial 3: Empty string / whitespace translations
    let json_whitespace = r#"[["", "en"], ["   ", "en"], "   "]"#;
    let res_whitespace = parse_google_translate_response_oracle(json_whitespace, &inputs).unwrap();
    assert_eq!(res_whitespace, inputs, "Empty or whitespace translations must fallback to inputs");

    // Adversarial 4: Truncated response (fewer items than inputs)
    let json_truncated = r#"[["Only First", "en"]]"#;
    let res_truncated = parse_google_translate_response_oracle(json_truncated, &inputs).unwrap();
    assert_eq!(res_truncated, vec!["Only First", "Text 2", "Text 3"]);

    // Adversarial 5: Oversized response (more items than inputs)
    let json_oversized = r#"["T1", "T2", "T3", "T4", "T5"]"#;
    let res_oversized = parse_google_translate_response_oracle(json_oversized, &inputs).unwrap();
    assert_eq!(res_oversized.len(), 5);
    assert_eq!(&res_oversized[0..3], &["T1", "T2", "T3"]);
}

#[test]
fn test_stress_dual_format_json_parser_malformed_and_deep_types() {
    let inputs = vec!["Fallback A".to_string(), "Fallback B".to_string()];

    // Adversarial 6: Malformed JSON syntax
    let malformed_syntax = "[{\"unclosed\":";
    assert!(parse_google_translate_response_oracle(malformed_syntax, &inputs).is_err());

    // Adversarial 7: Root is not an array (Object, Number, String, Boolean, Null)
    assert!(parse_google_translate_response_oracle("{\"error\": \"bad request\"}", &inputs).is_err());
    assert!(parse_google_translate_response_oracle("12345", &inputs).is_err());
    assert!(parse_google_translate_response_oracle("\"plain string\"", &inputs).is_err());
    assert!(parse_google_translate_response_oracle("true", &inputs).is_err());
    assert!(parse_google_translate_response_oracle("null", &inputs).is_err());

    // Adversarial 8: Array containing unexpected types (nested empty arrays, objects, booleans, numbers)
    let weird_elements = r#"[[], {}, 42, false, [null, "en"], [123, "en"]]"#;
    let res_weird = parse_google_translate_response_oracle(weird_elements, &inputs).unwrap();
    // Index 0: [] -> pair.first() is None -> fallback inputs[0]
    // Index 1: {} -> fallback inputs[1]
    // Index 2: 42 -> fallback unwrap_or_default ("")
    // Index 3: false -> fallback unwrap_or_default ("")
    // Index 4: [null, "en"] -> trans_str is None -> fallback unwrap_or_default ("")
    // Index 5: [123, "en"] -> trans_str is None -> fallback unwrap_or_default ("")
    assert_eq!(res_weird[0], "Fallback A");
    assert_eq!(res_weird[1], "Fallback B");
}

#[test]
fn test_stress_dual_format_json_parser_escaped_strings_and_newlines() {
    let inputs = vec!["Original Quote".to_string(), "Original Multiline".to_string()];

    // String containing escaped quotes, newlines, tabs, and unicode escapes
    let json_escaped = r#"[
        "\"Quoted Text\" with \\backslashes\\ and \u0026 ampersand",
        "Line 1\nLine 2\r\nLine 3\tTabbed"
    ]"#;

    let res = parse_google_translate_response_oracle(json_escaped, &inputs).unwrap();
    assert_eq!(res.len(), 2);
    assert_eq!(res[0], "\"Quoted Text\" with \\backslashes\\ and & ampersand");
    assert_eq!(res[1], "Line 1\nLine 2\r\nLine 3\tTabbed");
}

// =============================================================================
// 4. END-TO-END TRANSLATE_ON_SCREEN BOUNDARY STRESS
// =============================================================================

#[test]
fn test_stress_translate_on_screen_empty_and_zero_dims() {
    tauri::async_runtime::block_on(async {
        // 1. Empty boxes
        let res_empty = translate_on_screen(vec![], Some("pt".to_string()), Some("auto".to_string())).await;
        assert!(res_empty.is_ok());
        assert_eq!(res_empty.unwrap(), vec![]);

        // 2. Boxes with empty text
        let empty_text_boxes = vec![
            TranslationBoxInput {
                text: "".to_string(),
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            TranslationBoxInput {
                text: "   ".to_string(),
                x: 10.0,
                y: 20.0,
                width: 0.0,
                height: 0.0,
            },
        ];

        let res_empty_text = translate_on_screen(
            empty_text_boxes.clone(),
            Some("es".to_string()),
            Some("en".to_string()),
        )
        .await;
        assert!(res_empty_text.is_ok());
        let out = res_empty_text.unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].x, 0.0);
        assert_eq!(out[0].y, 0.0);
        assert_eq!(out[0].width, 100.0);
        assert_eq!(out[0].height, 50.0);
        assert_eq!(out[1].x, 10.0);
        assert_eq!(out[1].y, 20.0);
        assert_eq!(out[1].width, 0.0);
        assert_eq!(out[1].height, 0.0);
    });
}

#[test]
fn test_stress_translate_on_screen_adversarial_languages() {
    tauri::async_runtime::block_on(async {
        let input_boxes = vec![
            TranslationBoxInput {
                text: "Save File".to_string(),
                x: 15.0,
                y: 25.0,
                width: 80.0,
                height: 20.0,
            },
        ];

        // Extreme/unusual language parameters should never panic or fail the invoke:
        // 1. None, None
        let r1 = translate_on_screen(input_boxes.clone(), None, None).await;
        assert!(r1.is_ok(), "translate_on_screen with None, None must succeed");

        // 2. Empty strings
        let r2 = translate_on_screen(input_boxes.clone(), Some("".to_string()), Some("".to_string())).await;
        assert!(r2.is_ok(), "translate_on_screen with empty strings must succeed");

        // 3. Punctuation / Emoji languages
        let r3 = translate_on_screen(input_boxes.clone(), Some("💡".to_string()), Some("!@#".to_string())).await;
        assert!(r3.is_ok(), "translate_on_screen with emoji/symbol languages must succeed without panic");
    });
}
