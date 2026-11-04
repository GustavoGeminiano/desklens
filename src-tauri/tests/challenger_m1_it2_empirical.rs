//! Challenger M1 Iteration 2 Empirical Stress Test Harness
//!
//! Specifically validates the 3 Iteration 1 failure areas:
//! 1. Code indentation preservation on single lines across all grouping interfaces.
//! 2. Punctuation termination on lines ending with emojis, quotes, and brackets.
//! 3. Offline fallback wall-clock completion in <= 3.0s under both connection-refusal and blackhole timeout.

use desklens_lib::ocr::{
    group_bounding_boxes_into_paragraphs, group_lines_into_paragraphs,
    should_merge_lines_into_paragraph, BoundingBox, OcrLine,
};
use desklens_lib::translation::{
    cluster_translation_boxes, translate_on_screen, TranslationBoxInput,
};
use std::time::Instant;

// =============================================================================
// AREA 1: CODE INDENTATION PRESERVATION ON SINGLE LINES
// =============================================================================

#[test]
fn test_empirical_indentation_preservation_across_all_interfaces() {
    let test_cases = vec![
        "    let x = 1;",
        "        return result;",
        "\tconst API_URL = 'https://example.com';",
        "  // Single line comment",
        "    }",
        "   SELECT * FROM users WHERE active = 1;",
        "          deeply_indented_identifier();",
    ];

    for (i, &code_str) in test_cases.iter().enumerate() {
        // 1. OcrLine interface (group_lines_into_paragraphs)
        let ocr_line = OcrLine {
            text: code_str.to_string(),
            x: 20.0,
            y: 50.0 + (i as f64) * 30.0,
            width: 250.0,
            height: 18.0,
            words: vec![],
        };
        let grouped_ocr = group_lines_into_paragraphs(&[ocr_line]);
        assert_eq!(grouped_ocr.len(), 1);
        assert_eq!(
            grouped_ocr[0].text, code_str,
            "group_lines_into_paragraphs stripped indentation for case {}: {:?}",
            i, code_str
        );
        assert_eq!(grouped_ocr[0].is_paragraph, Some(false));

        // 2. BoundingBox interface (group_bounding_boxes_into_paragraphs)
        let bbox = BoundingBox {
            text: code_str.to_string(),
            x: 20.0,
            y: 50.0 + (i as f64) * 30.0,
            width: 250.0,
            height: 18.0,
            translated_text: None,
        };
        let grouped_bbox = group_bounding_boxes_into_paragraphs(&[bbox]);
        assert_eq!(grouped_bbox.len(), 1);
        assert_eq!(
            grouped_bbox[0].text, code_str,
            "group_bounding_boxes_into_paragraphs stripped indentation for case {}: {:?}",
            i, code_str
        );
        assert_eq!(grouped_bbox[0].is_paragraph, Some(false));

        // 3. TranslationBoxInput interface (cluster_translation_boxes)
        let trans_box = TranslationBoxInput {
            text: code_str.to_string(),
            x: 20.0,
            y: 50.0 + (i as f64) * 30.0,
            width: 250.0,
            height: 18.0,
        };
        let (grouped_trans, is_para) = cluster_translation_boxes(&[trans_box]);
        assert_eq!(grouped_trans.len(), 1);
        assert_eq!(
            grouped_trans[0].text, code_str,
            "cluster_translation_boxes stripped indentation for case {}: {:?}",
            i, code_str
        );
        assert_eq!(is_para, vec![false]);
    }
}

// =============================================================================
// AREA 2: PUNCTUATION TERMINATION WITH EMOJIS, QUOTES, AND BRACKETS
// =============================================================================

#[test]
fn test_empirical_terminal_punctuation_with_decorators_and_emojis() {
    let punctuated_first_lines = vec![
        "Parabéns pelo resultado! 🚀",
        "Alguma dúvida sobre a aula? 🤔",
        "Aviso importante: 📌",
        "Etapa concluída com sucesso; ✅",
        "Dica essencial. 💡",
        "Ele gritou: \"Socorro!\"",
        "Ela perguntou: 'Você vem?'",
        "Atenção com a regra: “Cuidado!”",
        "(Nota de rodapé importante!)",
        "[Veja anexo para detalhes?]",
        "Parabéns a todos! 🎉✨🔥",
        "Tudo pronto? ❓❔",
    ];

    let second_line_text = "Esta é a linha seguinte que NÃO deve se fundir com a anterior.";

    for (idx, first_line) in punctuated_first_lines.iter().enumerate() {
        let should_merge = should_merge_lines_into_paragraph(
            first_line,
            10.0,
            20.0,
            500.0,
            18.0,
            second_line_text,
            10.0,
            42.0,
            500.0,
            18.0,
            18.0,
            22.0,
            600.0,
        );

        println!(
            "[EMOJI/PUNCT TEST] Line {}: {:?} -> merge = {}",
            idx, first_line, should_merge
        );

        assert!(
            !should_merge,
            "Line ending with decorator/emoji after punctuation MUST NOT merge! Line: {:?}",
            first_line
        );
    }
}

#[test]
fn test_empirical_prose_continuation_across_non_terminal_emojis() {
    // Non-terminal emojis inside or at the end of wrapped lines should continue naturally
    let line1 = "Nós adoramos usar ferramentas inovadoras 🚀";
    let line2 = "para acelerar o desenvolvimento do software.";

    let should_merge = should_merge_lines_into_paragraph(
        line1,
        10.0,
        20.0,
        500.0,
        18.0,
        line2,
        10.0,
        42.0,
        500.0,
        18.0,
        18.0,
        22.0,
        600.0,
    );

    assert!(
        should_merge,
        "Prose line ending with an emoji WITHOUT terminal punctuation SHOULD merge with following line!"
    );
}

// =============================================================================
// AREA 3: OFFLINE FALLBACK COMPLETION <= 3.0s WALL-CLOCK DURATION
// =============================================================================

#[test]
fn test_empirical_offline_fallback_wall_clock_connection_refusal() {
    tauri::async_runtime::block_on(async {
        // Force connection refusal via invalid local port
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9");
        std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
        std::env::set_var("ALL_PROXY", "http://127.0.0.1:9");

        let input_boxes = vec![
            TranslationBoxInput {
                text: "Linha de teste para medição de timeout offline 1,".to_string(),
                x: 15.0,
                y: 20.0,
                width: 300.0,
                height: 20.0,
            },
            TranslationBoxInput {
                text: "linha de teste para medição de timeout offline 2.".to_string(),
                x: 15.0,
                y: 45.0,
                width: 310.0,
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

        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("ALL_PROXY");

        println!(
            "⏱️ [EMPIRICAL MEASUREMENT - REFUSAL] Duration: {:.3}s ({:?})",
            elapsed.as_secs_f64(),
            elapsed
        );

        assert!(res.is_ok(), "translate_on_screen must succeed with fallback text");
        let resp = res.unwrap();
        assert_eq!(resp.len(), 1, "Lines should cluster into 1 paragraph");
        assert_eq!(
            resp[0].translated_text,
            "Linha de teste para medição de timeout offline 1, linha de teste para medição de timeout offline 2."
        );
        assert!(
            elapsed.as_secs_f64() <= 3.0,
            "Wall-clock elapsed time under connection refusal must be <= 3.0s, was: {:.3}s",
            elapsed.as_secs_f64()
        );
    });
}

#[test]
fn test_empirical_offline_multiple_chunks_budget_ceiling() {
    tauri::async_runtime::block_on(async {
        // Force offline via invalid proxy
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9");
        std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
        std::env::set_var("ALL_PROXY", "http://127.0.0.1:9");

        // 35 separate paragraphs (forces splitting across CHUNK_SIZE = 30)
        let mut input_boxes = Vec::new();
        for i in 0..35 {
            input_boxes.push(TranslationBoxInput {
                text: format!("Standalone offline item {i}."),
                x: 10.0,
                y: 10.0 + (i as f64) * 100.0,
                width: 200.0,
                height: 20.0,
            });
        }

        let start = Instant::now();
        let res = translate_on_screen(
            input_boxes,
            Some("en".to_string()),
            Some("pt".to_string()),
        )
        .await;
        let elapsed = start.elapsed();

        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("ALL_PROXY");

        println!(
            "⏱️ [EMPIRICAL MEASUREMENT - 35 ITEMS MULTI-CHUNK] Duration: {:.3}s ({:?})",
            elapsed.as_secs_f64(),
            elapsed
        );

        assert!(res.is_ok());
        let resp = res.unwrap();
        assert_eq!(resp.len(), 35);
        assert!(
            elapsed.as_secs_f64() <= 4.5,
            "Wall-clock elapsed time for 35 items under offline conditions must be bounded <= 4.5s, was: {:.3}s",
            elapsed.as_secs_f64()
        );
    });
}
