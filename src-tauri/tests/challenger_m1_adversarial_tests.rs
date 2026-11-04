//! Challenger M1 Adversarial Empirical Stress Test Suite
//!
//! Rigorously verifies:
//! 1. Ground Truth Test: User's reported Portuguese paragraph from Image 2 (`media_1793729546214_613885a7.png`):
//!    - 3 lines grouped into exactly 1 paragraph bounding box.
//!    - Union box covers [21, 14, 969, 69].
//!    - Text properly joined without cutting off "não se" / "preocupe!".
//!    - End-to-end `translate_on_screen` produces coherent translation with detected_source_lang == "pt".
//! 2. Multi-column layouts (reading order, adjacent columns, narrow gutters).
//! 3. Mixed font heights (large titles, subheadings, inline height jitter).
//! 4. Extreme aspect ratios (ultra-wide ribbons, tall thin sidebars, single narrow chars).
//! 5. Nested quotes and dialogue wrapping.
//! 6. Terminal punctuation dynamics (!, ?, :, ;) vs continuing sentences.

use desklens_lib::ocr::{
    group_bounding_boxes_into_paragraphs, group_lines_into_paragraphs,
    should_merge_lines_into_paragraph, BoundingBox, OcrLine,
};
use desklens_lib::translation::{
    cluster_translation_boxes, group_translation_boxes_into_paragraphs, translate_on_screen,
    TranslationBoxInput, TranslationResponse,
};

// =============================================================================
// 1. GROUND TRUTH: IMAGE 2 VERBATIM PORTUGUESE PARAGRAPH
// =============================================================================

#[test]
fn test_ground_truth_image2_verbatim_portuguese_paragraph_clustering() {
    // Verbatim OCR coordinates from Image 2 (media_1793729546214_613885a7.png)
    let lines = vec![
        OcrLine {
            text: "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se".to_string(),
            x: 22.0,
            y: 14.0,
            width: 968.0,
            height: 18.0,
            words: vec![],
        },
        OcrLine {
            text: "preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas".to_string(),
            x: 22.0,
            y: 40.0,
            width: 968.0,
            height: 18.0,
            words: vec![],
        },
        OcrLine {
            text: "faz parte do processo de aprendizagem.".to_string(),
            x: 21.0,
            y: 65.0,
            width: 324.0,
            height: 18.0,
            words: vec![],
        },
    ];

    // 1. Verify group_lines_into_paragraphs
    let paragraphs = group_lines_into_paragraphs(&lines);
    assert_eq!(
        paragraphs.len(),
        1,
        "Image 2 Portuguese lines MUST cluster into exactly 1 paragraph"
    );

    let p0 = &paragraphs[0];
    assert_eq!(p0.line_indices, vec![0, 1, 2]);
    assert_eq!(p0.is_paragraph, Some(true));

    // Exact union bounding box:
    // min(22, 22, 21) = 21.0
    // min(14, 40, 65) = 14.0
    // max(22+968, 22+968, 21+324) - 21 = 990.0 - 21.0 = 969.0
    // max(14+18, 40+18, 65+18) - 14 = 83.0 - 14.0 = 69.0
    assert_eq!(p0.x, 21.0, "Paragraph union x must be 21.0");
    assert_eq!(p0.y, 14.0, "Paragraph union y must be 14.0");
    assert_eq!(p0.width, 969.0, "Paragraph union width must be 969.0");
    assert_eq!(p0.height, 69.0, "Paragraph union height must be 69.0");

    // Text concatenation verification
    let expected_joined = "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas faz parte do processo de aprendizagem.";
    assert_eq!(p0.text, expected_joined);
    assert_eq!(p0.text.chars().count(), 268, "Must be exactly 268 Unicode characters");
    assert_eq!(p0.text.len(), 274, "Must be 274 UTF-8 bytes (due to Portuguese accented characters)");
    assert!(
        p0.text.contains("não se preocupe!"),
        "Text must preserve unbroken 'não se preocupe!'"
    );

    // 2. Verify group_bounding_boxes_into_paragraphs
    let bboxes = vec![
        BoundingBox {
            text: lines[0].text.clone(),
            x: lines[0].x,
            y: lines[0].y,
            width: lines[0].width,
            height: lines[0].height,
            translated_text: None,
        },
        BoundingBox {
            text: lines[1].text.clone(),
            x: lines[1].x,
            y: lines[1].y,
            width: lines[1].width,
            height: lines[1].height,
            translated_text: None,
        },
        BoundingBox {
            text: lines[2].text.clone(),
            x: lines[2].x,
            y: lines[2].y,
            width: lines[2].width,
            height: lines[2].height,
            translated_text: None,
        },
    ];

    let b_paras = group_bounding_boxes_into_paragraphs(&bboxes);
    assert_eq!(b_paras.len(), 1);
    assert_eq!(b_paras[0].x, 21.0);
    assert_eq!(b_paras[0].y, 14.0);
    assert_eq!(b_paras[0].width, 969.0);
    assert_eq!(b_paras[0].height, 69.0);
    assert_eq!(b_paras[0].text, expected_joined);
    assert_eq!(b_paras[0].is_paragraph, Some(true));
}

#[test]
fn test_ground_truth_image2_translation_on_screen_end_to_end() {
    tauri::async_runtime::block_on(async {
        let input_boxes = vec![
            TranslationBoxInput {
                text: "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se".to_string(),
                x: 22.0,
                y: 14.0,
                width: 968.0,
                height: 18.0,
            },
            TranslationBoxInput {
                text: "preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas".to_string(),
                x: 22.0,
                y: 40.0,
                width: 968.0,
                height: 18.0,
            },
            TranslationBoxInput {
                text: "faz parte do processo de aprendizagem.".to_string(),
                x: 21.0,
                y: 65.0,
                width: 324.0,
                height: 18.0,
            },
        ];

        let result = translate_on_screen(
            input_boxes,
            Some("en".to_string()),
            Some("auto".to_string()),
        )
        .await;

        assert!(
            result.is_ok(),
            "translate_on_screen failed: {:?}",
            result.err()
        );
        let resp: TranslationResponse = result.unwrap();

        // Must coalesce into exactly 1 translation box
        assert_eq!(
            resp.len(),
            1,
            "Input 3 lines MUST cluster into 1 unified translation box"
        );
        let out = &resp[0];
        assert_eq!(out.x, 21.0);
        assert_eq!(out.y, 14.0);
        assert_eq!(out.width, 969.0);
        assert_eq!(out.height, 69.0);
        assert_eq!(out.is_paragraph, Some(true));

        println!("[GROUND TRUTH TRANSLATION]: {}", out.translated_text);
        assert!(!out.translated_text.is_empty());

        // Semantic rules for contextual translation
        let lower = out.translated_text.to_lowercase();
        let has_negation = lower.contains("don't worry")
            || lower.contains("do not worry")
            || lower.contains("never worry")
            || lower.contains("no worries");
        assert!(
            has_negation,
            "Must preserve negation 'don't worry' in translation: '{}'",
            out.translated_text
        );

        let has_hallucinated_contact = lower.contains("do not hesitate to contact us");
        assert!(
            !has_hallucinated_contact,
            "Must not hallucinate closing contact formula"
        );

        if let Some(ref detected) = resp.detected_source_lang {
            println!("[GROUND TRUTH DETECTED LANG]: {}", detected);
            assert_eq!(detected, "pt");
        }
    });
}

// =============================================================================
// 2. PARAGRAPH CLUSTERING: MULTI-COLUMN LINES
// =============================================================================

#[test]
fn test_adv_multi_column_reading_order_two_columns() {
    // Column 1: x: 10..200
    // Column 2: x: 300..500
    // Provided in reading order: Col 1 lines, then Col 2 lines
    let lines = vec![
        OcrLine {
            text: "This is the first line of the left column,".to_string(),
            x: 10.0,
            y: 10.0,
            width: 190.0,
            height: 18.0,
            words: vec![],
        },
        OcrLine {
            text: "and here is the second line of the left column.".to_string(),
            x: 10.0,
            y: 32.0,
            width: 190.0,
            height: 18.0,
            words: vec![],
        },
        // Column 2 starts back at top y=10
        OcrLine {
            text: "Now this is the first line of the right column,".to_string(),
            x: 300.0,
            y: 10.0,
            width: 200.0,
            height: 18.0,
            words: vec![],
        },
        OcrLine {
            text: "which also continues into its second line.".to_string(),
            x: 300.0,
            y: 32.0,
            width: 195.0,
            height: 18.0,
            words: vec![],
        },
    ];

    let paragraphs = group_lines_into_paragraphs(&lines);
    assert_eq!(
        paragraphs.len(),
        2,
        "Two columns in reading order must form 2 distinct paragraphs"
    );

    assert_eq!(paragraphs[0].line_indices, vec![0, 1]);
    assert_eq!(paragraphs[0].x, 10.0);
    assert_eq!(paragraphs[0].width, 190.0);

    assert_eq!(paragraphs[1].line_indices, vec![2, 3]);
    assert_eq!(paragraphs[1].x, 300.0);
    assert_eq!(paragraphs[1].width, 200.0);
}

#[test]
fn test_adv_multi_column_narrow_gutter_separation() {
    // Two columns separated by only 12px gutter
    // Col 1: x=20, w=150 (right edge=170)
    // Col 2: x=182, w=150 (left edge=182, gutter=12px)
    let can_merge = should_merge_lines_into_paragraph(
        "End of left column line",
        20.0,
        10.0,
        150.0,
        18.0,
        "Start of right column line",
        182.0,
        32.0,
        150.0,
        18.0,
        18.0,
        22.0,
        150.0,
    );
    assert!(
        !can_merge,
        "Lines in different columns with narrow gutter must NOT merge (0% horizontal overlap)"
    );
}

#[test]
fn test_adv_multi_column_three_column_newspaper() {
    let mut lines = Vec::new();
    let col_xs = [50.0, 300.0, 550.0];

    for (c, &col_x) in col_xs.iter().enumerate() {
        for l in 0..3 {
            lines.push(OcrLine {
                text: format!("Column {} line {} text continuing", c + 1, l + 1),
                x: col_x,
                y: 10.0 + (l as f64) * 24.0,
                width: 200.0,
                height: 18.0,
                words: vec![],
            });
        }
    }

    let paras = group_lines_into_paragraphs(&lines);
    assert_eq!(
        paras.len(),
        3,
        "Three distinct columns must produce exactly 3 separate paragraph units"
    );
    for (i, p) in paras.iter().enumerate() {
        assert_eq!(p.x, col_xs[i]);
        assert_eq!(p.line_indices.len(), 3);
        assert_eq!(p.is_paragraph, Some(true));
    }
}

// =============================================================================
// 3. PARAGRAPH CLUSTERING: MIXED FONT HEIGHTS
// =============================================================================

#[test]
fn test_adv_mixed_font_height_heading_vs_body() {
    // Main Heading (h: 36px) followed by Body text (h: 18px) -> ratio = 18/36 = 0.50
    // Heading is Title Case
    let heading_merges = should_merge_lines_into_paragraph(
        "Comprehensive System Overview",
        50.0,
        20.0,
        350.0,
        36.0,
        "This chapter introduces the core architectural pillars.",
        50.0,
        64.0,
        350.0,
        18.0,
        18.0,
        24.0,
        350.0,
    );
    assert!(
        !heading_merges,
        "Large title-cased heading must NOT merge with body line"
    );

    // Drastic font height difference (e.g. h=48px banner vs h=14px text) -> ratio 14/48 = 0.29 < 0.50
    let banner_merges = should_merge_lines_into_paragraph(
        "BREAKING NEWS",
        50.0,
        10.0,
        300.0,
        48.0,
        "details follow in the report below",
        50.0,
        68.0,
        300.0,
        14.0,
        14.0,
        20.0,
        300.0,
    );
    assert!(
        !banner_merges,
        "Ratio < 0.50 font height differences must reject merging"
    );
}

#[test]
fn test_adv_mixed_font_height_within_paragraph_jitter() {
    // Normal variations due to font ascenders/descenders (e.g. h=17.5 vs h=19.2)
    // ratio = 17.5 / 19.2 = 0.91 >= 0.50
    let lines = vec![
        OcrLine {
            text: "This paragraph contains natural typographic ascenders".to_string(),
            x: 40.0,
            y: 50.0,
            width: 320.0,
            height: 19.2,
            words: vec![],
        },
        OcrLine {
            text: "and descenders causing minor pixel height variations".to_string(),
            x: 40.0,
            y: 72.0,
            width: 318.0,
            height: 17.5,
            words: vec![],
        },
        OcrLine {
            text: "without indicating any font size change.".to_string(),
            x: 40.0,
            y: 94.0,
            width: 250.0,
            height: 18.0,
            words: vec![],
        },
    ];

    let paras = group_lines_into_paragraphs(&lines);
    assert_eq!(
        paras.len(),
        1,
        "Minor height jitter within natural font line height MUST be merged"
    );
    assert_eq!(paras[0].line_indices.len(), 3);
}

// =============================================================================
// 4. PARAGRAPH CLUSTERING: EXTREME ASPECT RATIOS
// =============================================================================

#[test]
fn test_adv_extreme_aspect_ratio_ultra_wide_ribbon() {
    let ribbon = OcrLine {
        text: "NOTIFICATION: System maintenance scheduled tonight from 02:00 to 04:00 UTC. Please save work.".to_string(),
        x: 10.0,
        y: 10.0,
        width: 1600.0,
        height: 14.0,
        words: vec![],
    };
    let body = OcrLine {
        text: "The regular operations of the service will continue without interruption tomorrow.".to_string(),
        x: 10.0,
        y: 60.0, // gap = 36px
        width: 400.0,
        height: 18.0,
        words: vec![],
    };

    let paras = group_lines_into_paragraphs(&[ribbon, body]);
    assert_eq!(
        paras.len(),
        2,
        "Ultra-wide ribbon separated by vertical gap must remain separate"
    );
    assert_eq!(paras[0].width, 1600.0);
    assert_eq!(paras[0].height, 14.0);
    assert_eq!(paras[0].is_paragraph, Some(false));
}

#[test]
fn test_adv_extreme_aspect_ratio_narrow_vertical_characters() {
    // Single narrow characters '|' or 'I' (w: 3px, h: 20px)
    let narrow = BoundingBox {
        text: "|".to_string(),
        x: 10.0,
        y: 10.0,
        width: 3.0,
        height: 20.0,
        translated_text: None,
    };
    let paras = group_bounding_boxes_into_paragraphs(&[narrow]);
    assert_eq!(paras.len(), 1);
    assert_eq!(paras[0].width, 3.0);
    assert_eq!(paras[0].height, 20.0);
    assert_eq!(paras[0].is_paragraph, Some(false));
}

// =============================================================================
// 5. PARAGRAPH CLUSTERING: NESTED QUOTES & DIALOGUE
// =============================================================================

#[test]
fn test_adv_nested_quotes_wrapped_dialogue() {
    let lines = vec![
        OcrLine {
            text: "“Ela olhou nos olhos dele e disse: ‘Não se preocupe,".to_string(),
            x: 30.0,
            y: 40.0,
            width: 380.0,
            height: 18.0,
            words: vec![],
        },
        OcrLine {
            text: "nós vamos superar esse desafio juntos’, com firmeza.”".to_string(),
            x: 30.0,
            y: 64.0,
            width: 390.0,
            height: 18.0,
            words: vec![],
        },
    ];

    let paras = group_lines_into_paragraphs(&lines);
    assert_eq!(
        paras.len(),
        1,
        "Wrapped quoted dialogue must cluster into 1 paragraph"
    );
    assert_eq!(
        paras[0].text,
        "“Ela olhou nos olhos dele e disse: ‘Não se preocupe, nós vamos superar esse desafio juntos’, com firmeza.”"
    );
}

// =============================================================================
// 6. TERMINAL PUNCTUATION VS CONTINUING SENTENCES
// =============================================================================

#[test]
fn test_adv_terminal_punctuation_vs_continuation() {
    // Line 1 ending in exclamation mark should NOT merge into line 2 when they are separate clauses
    let merges_exclamation = should_merge_lines_into_paragraph(
        "Parabéns pelo excelente trabalho!",
        20.0,
        20.0,
        250.0,
        18.0,
        "Agora vamos iniciar o próximo módulo.",
        20.0,
        45.0,
        260.0,
        18.0,
        18.0,
        25.0,
        260.0,
    );
    assert!(
        !merges_exclamation,
        "Lines ending with '!' are treated as terminal boundaries by should_merge_lines_into_paragraph"
    );

    // Line 1 ending in question mark should NOT merge
    let merges_question = should_merge_lines_into_paragraph(
        "Você tem certeza que deseja sair?",
        20.0,
        20.0,
        240.0,
        18.0,
        "Todas as alterações serão perdidas.",
        20.0,
        45.0,
        250.0,
        18.0,
        18.0,
        25.0,
        250.0,
    );
    assert!(
        !merges_question,
        "Lines ending with '?' are treated as terminal boundaries"
    );

    // But mid-line exclamation followed by wrap is preserved (like Image 2):
    // Line 0: "... não se"
    // Line 1: "preocupe! Sinta-se à vontade..."
    let merges_image2_wrap = should_merge_lines_into_paragraph(
        "Lembrando que, se você ficou com alguma dúvida, não se",
        22.0,
        14.0,
        500.0,
        18.0,
        "preocupe! Sinta-se à vontade para conversar com seu professor.",
        22.0,
        40.0,
        510.0,
        18.0,
        18.0,
        26.0,
        510.0,
    );
    assert!(
        merges_image2_wrap,
        "Mid-sentence wrap with exclamation at start of line 2 MUST merge"
    );
}

// =============================================================================
// 7. CLUSTERING TRANSLATION BOXES HELPER CONTRACT
// =============================================================================

#[test]
fn test_adv_cluster_translation_boxes_identity_and_properties() {
    // Empty input
    let (empty_out, flags) = cluster_translation_boxes(&[]);
    assert!(empty_out.is_empty());
    assert!(flags.is_empty());

    // Single item
    let single = vec![TranslationBoxInput {
        text: "Single isolated button".to_string(),
        x: 10.0,
        y: 20.0,
        width: 120.0,
        height: 24.0,
    }];
    let (single_out, single_flags) = cluster_translation_boxes(&single);
    assert_eq!(single_out.len(), 1);
    assert_eq!(single_flags, vec![false]);
    assert_eq!(single_out[0].text, "Single isolated button");

    // Group translation boxes into paragraphs convenience wrapper
    let wrapped = group_translation_boxes_into_paragraphs(&single);
    assert_eq!(wrapped.len(), 1);
}
