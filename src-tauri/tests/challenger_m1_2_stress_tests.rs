//! Challenger M1.2 Adversarial Stress & Verification Test Suite
//!
//! Empirically challenges:
//! 1. IPC Serialization: TranslationResponse JSON round-trip, detected_source_lang preservation,
//!    null/missing field handling, and legacy array format behavior.
//! 2. Fallback & Network Timeout: offline network condition, bounded execution time, thread pool stability.
//! 3. Boundary & Extreme Inputs: empty, zero-sized, negative coords, emojis, multi-byte UTF-8,
//!    code indentation non-merging, and 100+ line batch scaling.

use desklens_lib::ocr::{group_bounding_boxes_into_paragraphs, BoundingBox};
use desklens_lib::translation::{
    cluster_translation_boxes, translate_on_screen, TranslationBoxInput, TranslationBoxOutput,
    TranslationResponse,
};
use std::time::Instant;

// =============================================================================
// 1. IPC SERIALIZATION & BOUNDARY RESILIENCE
// =============================================================================

#[test]
fn test_ipc_serialization_roundtrip_with_detected_lang() {
    let original = TranslationResponse {
        boxes: vec![TranslationBoxOutput {
            text: "Lembrando que não se preocupe!".to_string(),
            x: 21.0,
            y: 14.0,
            width: 969.0,
            height: 69.0,
            translated_text: "Remember that do not worry!".to_string(),
            is_paragraph: Some(true),
            detected_source_lang: Some("pt".to_string()),
        }],
        detected_source_lang: Some("pt".to_string()),
    };

    // Serialize to JSON string as Tauri IPC does
    let json_str = serde_json::to_string(&original).expect("Serialization must succeed");
    println!("[IPC OBS] Serialized JSON: {}", json_str);

    // Verify payload contract
    assert!(json_str.contains("\"detected_source_lang\":\"pt\""));
    assert!(json_str.contains("\"is_paragraph\":true"));
    assert!(json_str.contains("\"translated_text\":"));

    // Deserialize back from JSON
    let deserialized: TranslationResponse =
        serde_json::from_str(&json_str).expect("Deserialization must succeed");

    assert_eq!(
        deserialized.detected_source_lang,
        Some("pt".to_string()),
        "detected_source_lang must NOT be dropped across IPC boundary"
    );
    assert_eq!(deserialized.boxes.len(), 1);
    assert_eq!(deserialized.boxes[0].text, original.boxes[0].text);
    assert_eq!(
        deserialized.boxes[0].translated_text,
        original.boxes[0].translated_text
    );
    assert_eq!(deserialized.boxes[0].is_paragraph, Some(true));
}

#[test]
fn test_ipc_serialization_roundtrip_without_detected_lang() {
    let original = TranslationResponse {
        boxes: vec![TranslationBoxOutput {
            text: "Hello".to_string(),
            x: 10.0,
            y: 10.0,
            width: 50.0,
            height: 20.0,
            translated_text: "Olá".to_string(),
            is_paragraph: None,
            detected_source_lang: None,
        }],
        detected_source_lang: None,
    };

    let json_str = serde_json::to_string(&original).unwrap();
    println!("[IPC OBS] Serialized None JSON: {}", json_str);

    // When detected_source_lang is None, it should be omitted or null per serde attributes
    assert!(!json_str.contains("detected_source_lang"));
    assert!(!json_str.contains("is_paragraph"));

    let deserialized: TranslationResponse = serde_json::from_str(&json_str).unwrap();
    assert_eq!(deserialized.detected_source_lang, None);
    assert_eq!(deserialized.boxes[0].is_paragraph, None);
}

#[test]
fn test_ipc_deserialization_from_explicit_null_and_legacy_object() {
    // Test JSON with explicit null
    let json_with_null = r#"{"boxes":[],"detected_source_lang":null}"#;
    let res: Result<TranslationResponse, _> = serde_json::from_str(json_with_null);
    assert!(res.is_ok(), "Explicit null must deserialize to None");
    assert_eq!(res.unwrap().detected_source_lang, None);

    // Test JSON missing detected_source_lang entirely
    let json_missing_field = r#"{"boxes":[{"text":"a","x":0.0,"y":0.0,"width":10.0,"height":10.0,"translated_text":"b"}]}"#;
    let res2: Result<TranslationResponse, _> = serde_json::from_str(json_missing_field);
    assert!(res2.is_ok(), "Missing detected_source_lang must default to None");
    let resp2 = res2.unwrap();
    assert_eq!(resp2.detected_source_lang, None);
    assert_eq!(resp2.boxes.len(), 1);
    assert_eq!(resp2.boxes[0].translated_text, "b");
}

#[test]
fn test_ipc_deserialization_legacy_array_behavior() {
    // Empirical probe: Can TranslationResponse directly deserialize a legacy array `[{...}]`?
    let legacy_array_json = r#"[{"text":"test","x":0.0,"y":0.0,"width":10.0,"height":10.0,"translated_text":"teste"}]"#;
    let res: Result<TranslationResponse, _> = serde_json::from_str(legacy_array_json);

    println!(
        "[OBSERVATION] Does TranslationResponse directly deserialize legacy array? {}",
        res.is_ok()
    );
    // TranslationResponse is a struct { boxes, detected_source_lang }, so serde_json
    // cannot deserialize a JSON array into it without a custom untagged/visitor deserializer.
    // However, Vec<TranslationBoxOutput> CAN deserialize it:
    let vec_res: Result<Vec<TranslationBoxOutput>, _> = serde_json::from_str(legacy_array_json);
    assert!(vec_res.is_ok(), "Vec<TranslationBoxOutput> can deserialize legacy array");
    let converted: TranslationResponse = vec_res.unwrap().into();
    assert_eq!(converted.boxes.len(), 1);
    assert_eq!(converted.detected_source_lang, None);
}

// =============================================================================
// 2. TIMEOUT BOUNDING & NETWORK OFFLINE BEHAVIOR
// =============================================================================

#[test]
fn test_offline_network_bounded_timeout_stress() {
    // Point proxies to a closed port on 127.0.0.1 to simulate hard offline / connection refused
    std::env::set_var("HTTP_PROXY", "http://127.0.0.1:54321");
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:54321");
    std::env::set_var("ALL_PROXY", "http://127.0.0.1:54321");

    tauri::async_runtime::block_on(async {
        let input_boxes = vec![
            TranslationBoxInput {
                text: "Primeira linha do parágrafo offline,".to_string(),
                x: 20.0,
                y: 30.0,
                width: 200.0,
                height: 20.0,
            },
            TranslationBoxInput {
                text: "segunda linha do mesmo parágrafo.".to_string(),
                x: 20.0,
                y: 55.0,
                width: 210.0,
                height: 20.0,
            },
        ];

        let start = Instant::now();
        let res = translate_on_screen(
            input_boxes,
            Some("en".to_string()),
            Some("pt".to_string()),
        )
        .await;
        let elapsed = start.elapsed();

        // Immediately clean up proxy environment variables
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("ALL_PROXY");

        println!(
            "⏱️ [EMPIRICAL OFFLINE TIMING] Offline translate_on_screen elapsed: {:.2}s ({:?})",
            elapsed.as_secs_f64(),
            elapsed
        );

        assert!(
            res.is_ok(),
            "Offline translate_on_screen must not panic or return Err"
        );
        let resp = res.unwrap();

        // 1. Must cluster the 2 wrapped lines into 1 cohesive paragraph
        assert_eq!(
            resp.len(),
            1,
            "Offline lines should still cluster into a single paragraph"
        );
        let box0 = &resp[0];
        assert_eq!(box0.x, 20.0);
        assert_eq!(box0.y, 30.0);
        assert_eq!(box0.width, 210.0);
        assert_eq!(box0.height, 45.0); // 55+20 - 30 = 45.0
        assert_eq!(box0.is_paragraph, Some(true));

        // 2. Under total network failure, translated_text must fall back gracefully to the clustered original text
        assert_eq!(
            box0.translated_text,
            "Primeira linha do parágrafo offline, segunda linha do mesmo parágrafo."
        );

        assert!(
            elapsed.as_secs_f64() <= 3.0,
            "Offline translation under connection refusal must complete within 3.0s, took {:.2}s",
            elapsed.as_secs_f64()
        );
    });
}

// =============================================================================
// 3. BOUNDARY INPUTS: EMPTY, ZERO-SIZED, EMOJIS, CODE INDENTATION
// =============================================================================

#[test]
fn test_stress_empty_and_zero_sized_inputs() {
    tauri::async_runtime::block_on(async {
        // Zero-sized and negative coordinates
        let zero_boxes = vec![
            TranslationBoxInput {
                text: "Zero Box".to_string(),
                x: -100.0,
                y: -50.0,
                width: 0.0,
                height: 0.0,
            },
            TranslationBoxInput {
                text: "".to_string(),
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 20.0,
            },
            TranslationBoxInput {
                text: "   ".to_string(),
                x: 10.0,
                y: 10.0,
                width: 10.0,
                height: 10.0,
            },
        ];

        let res = translate_on_screen(zero_boxes, Some("en".to_string()), None).await;
        assert!(res.is_ok(), "Zero/empty boxes must not panic");
        let resp = res.unwrap();
        assert_eq!(resp.len(), 3);
        assert_eq!(resp[0].x, -100.0);
        assert_eq!(resp[0].width, 0.0);
    });
}

#[test]
fn test_stress_emojis_and_multibyte_utf8() {
    tauri::async_runtime::block_on(async {
        // Lines with internal emojis
        let emoji_boxes = vec![
            TranslationBoxInput {
                text: "🎉 Parabéns! Você ganhou um prêmio.".to_string(),
                x: 10.0,
                y: 10.0,
                width: 300.0,
                height: 25.0,
            },
            TranslationBoxInput {
                text: "💡 Dica rápida: use atalhos de teclado.".to_string(),
                x: 10.0,
                y: 40.0,
                width: 320.0,
                height: 25.0,
            },
        ];

        let res = translate_on_screen(
            emoji_boxes,
            Some("en".to_string()),
            Some("auto".to_string()),
        )
        .await;

        assert!(res.is_ok(), "Emoji boxes must translate without panic");
        let resp = res.unwrap();
        // Since line 1 fills >70% of column width with >4 words and tight vertical pitch (5px gap),
        // the paragraph clustering algorithm considers it an intra-paragraph sentence and clusters them:
        assert_eq!(resp.len(), 1, "Lines filling column width within paragraph pitch correctly coalesce into 1 paragraph");
        assert!(!resp[0].translated_text.is_empty());
        assert_eq!(resp[0].is_paragraph, Some(true));
    });
}

#[test]
fn test_stress_trailing_emoji_punctuation_bypass_discovery() {
    // EMPIRICAL VULNERABILITY PROBE:
    // When a line ends with an emoji following punctuation (e.g. "Parabéns! 🚀"),
    // `t_curr.ends_with(c)` checks if the LAST char is in ['!', '?', ':', ';'].
    // Since the last char is '🚀', the terminal punctuation check fails!
    let boxes = vec![
        TranslationBoxInput {
            text: "Parabéns! 🚀".to_string(),
            x: 10.0,
            y: 10.0,
            width: 200.0,
            height: 20.0,
        },
        TranslationBoxInput {
            text: "Próxima dica importante. 💡".to_string(),
            x: 10.0,
            y: 35.0,
            width: 220.0,
            height: 20.0,
        },
    ];

    let (clustered, is_para) = cluster_translation_boxes(&boxes);
    println!(
        "[EMPIRICAL OBSERVATION] Trailing emoji caused merge?: {}",
        clustered.len() == 1
    );
    assert_eq!(
        clustered.len(),
        2,
        "Separate punctuated items with trailing emojis must NOT merge"
    );
    assert_eq!(is_para, vec![false, false]);
}

#[test]
fn test_stress_code_indentation_non_merging() {
    // In code snippets, indentation and code syntax must NEVER merge lines into paragraphs!
    let code_boxes = vec![
        BoundingBox {
            text: "function calculateSubtotal(items) {".to_string(),
            x: 20.0,
            y: 100.0,
            width: 280.0,
            height: 20.0,
            translated_text: None,
        },
        BoundingBox {
            text: "    let total = 0;".to_string(),
            x: 20.0,
            y: 125.0,
            width: 150.0,
            height: 20.0,
            translated_text: None,
        },
        BoundingBox {
            text: "    for (const item of items) {".to_string(),
            x: 20.0,
            y: 150.0,
            width: 240.0,
            height: 20.0,
            translated_text: None,
        },
        BoundingBox {
            text: "        total += item.price;".to_string(),
            x: 20.0,
            y: 175.0,
            width: 220.0,
            height: 20.0,
            translated_text: None,
        },
        BoundingBox {
            text: "    }".to_string(),
            x: 20.0,
            y: 200.0,
            width: 40.0,
            height: 20.0,
            translated_text: None,
        },
        BoundingBox {
            text: "    return total;".to_string(),
            x: 20.0,
            y: 225.0,
            width: 140.0,
            height: 20.0,
            translated_text: None,
        },
        BoundingBox {
            text: "}".to_string(),
            x: 20.0,
            y: 250.0,
            width: 20.0,
            height: 20.0,
            translated_text: None,
        },
    ];

    // In translation.rs cluster_translation_boxes:
    let trans_boxes: Vec<TranslationBoxInput> = code_boxes
        .iter()
        .map(|b| TranslationBoxInput {
            text: b.text.clone(),
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        })
        .collect();

    let (trans_clustered, trans_is_para) = cluster_translation_boxes(&trans_boxes);
    assert_eq!(
        trans_clustered.len(),
        7,
        "Code lines with syntax delimiters ({{, }}, ;) must NEVER merge in translation"
    );
    // Verify translation.rs preserves exact indentation:
    assert_eq!(trans_clustered[1].text, "    let total = 0;");
    assert_eq!(trans_is_para, vec![false; 7]);

    // In ocr.rs group_bounding_boxes_into_paragraphs:
    let ocr_grouped = group_bounding_boxes_into_paragraphs(&code_boxes);
    assert_eq!(
        ocr_grouped.len(),
        7,
        "Code lines must NEVER merge in ocr.rs grouping"
    );
    // Observation: ocr.rs calls join_paragraph_lines which trims leading whitespace:
    println!(
        "[EMPIRICAL OBSERVATION] ocr.rs text trimmed?: '{}'",
        ocr_grouped[1].text
    );
    assert_eq!(ocr_grouped[1].text, "    let total = 0;");
}

// =============================================================================
// 4. LARGE SCALE STRESS: 120+ LINES CHUNKING & THREAD HARNESS
// =============================================================================

#[test]
fn test_stress_120_plus_lines_batch_scaling_and_thread_pool() {
    const TOTAL_LINES: usize = 120;

    tauri::async_runtime::block_on(async {
        let mut large_inputs = Vec::with_capacity(TOTAL_LINES);
        for i in 0..TOTAL_LINES {
            large_inputs.push(TranslationBoxInput {
                text: format!("Standalone Line Number {i}."),
                x: 10.0,
                y: 10.0 + (i as f64) * 25.0,
                width: 200.0,
                height: 18.0,
            });
        }

        let start = Instant::now();
        let res = translate_on_screen(
            large_inputs.clone(),
            Some("pt".to_string()),
            Some("en".to_string()),
        )
        .await;
        let elapsed = start.elapsed();

        println!(
            "🚀 [SCALE OBS] Translated {} lines in {:.2}s",
            TOTAL_LINES,
            elapsed.as_secs_f64()
        );

        assert!(
            res.is_ok(),
            "120-line batch translation must succeed: {:?}",
            res.err()
        );
        let resp = res.unwrap();

        // Because every line ended with '.' and is an isolated sentence item, they should not merge:
        assert_eq!(
            resp.len(),
            TOTAL_LINES,
            "All 120 lines must be preserved without loss"
        );

        for (i, item) in resp.iter().enumerate() {
            assert_eq!(item.x, large_inputs[i].x);
            assert_eq!(item.y, large_inputs[i].y);
            assert!(!item.translated_text.is_empty());
        }
    });
}
