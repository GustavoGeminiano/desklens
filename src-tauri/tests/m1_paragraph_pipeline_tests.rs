//! Milestone 1: Backend Paragraph Grouping & Contextual Translation Pipeline Tests
//!
//! Validates:
//! 1. Image 2 ground truth reproduction: wrapped Portuguese dialogue lines merge into 1 paragraph.
//! 2. Union bounding box computation: [min_x, min_y, max_x+w-min_x, max_y+h-min_y].
//! 3. Dehyphenation across line wraps.
//! 4. Separation of distinct UI items: buttons, code snippets, large vertical gaps.
//! 5. `translate_on_screen` returning `TranslationResponse` with `detected_source_lang` ("pt").
//! 6. TranslationResponse Deref / Index / PartialEq backward compatibility.

use desklens_lib::ocr::{
    group_bounding_boxes_into_paragraphs, group_lines_into_paragraphs, join_paragraph_lines,
    BoundingBox, OcrLine,
};
use desklens_lib::translation::{
    translate_on_screen, TranslationBoxInput, TranslationResponse,
};

#[test]
fn test_m1_image2_portuguese_dialogue_reproduction() {
    // Exact reproduction of the Portuguese text in user's reported Image 2:
    // Line 1: "Mas eu não estou preocupada com isso,"
    // Line 2: "afinal de contas temos todo o tempo"
    // Line 3: "do mundo para resolver isso."
    let lines = vec![
        OcrLine {
            text: "Mas eu não estou preocupada com isso,".to_string(),
            x: 50.0,
            y: 100.0,
            width: 250.0,
            height: 20.0,
            words: vec![],
        },
        OcrLine {
            text: "afinal de contas temos todo o tempo".to_string(),
            x: 50.0,
            y: 125.0, // pitch = 25px
            width: 240.0,
            height: 20.0,
            words: vec![],
        },
        OcrLine {
            text: "do mundo para resolver isso.".to_string(),
            x: 50.0,
            y: 150.0, // pitch = 25px
            width: 200.0,
            height: 20.0,
            words: vec![],
        },
    ];

    let paragraphs = group_lines_into_paragraphs(&lines);
    assert_eq!(
        paragraphs.len(),
        1,
        "Wrapped 3-line dialogue must cluster into exactly 1 paragraph"
    );
    assert_eq!(paragraphs[0].line_indices, vec![0, 1, 2]);
    assert_eq!(
        paragraphs[0].text,
        "Mas eu não estou preocupada com isso, afinal de contas temos todo o tempo do mundo para resolver isso."
    );
    assert_eq!(paragraphs[0].x, 50.0);
    assert_eq!(paragraphs[0].y, 100.0);
    assert_eq!(paragraphs[0].width, 250.0);
    assert_eq!(paragraphs[0].height, 70.0); // 150 + 20 - 100 = 70.0
}

#[test]
fn test_m1_dehyphenation_across_wrapped_lines() {
    let lines = vec![
        OcrLine {
            text: "Estamos desen-".to_string(),
            x: 40.0,
            y: 50.0,
            width: 200.0,
            height: 18.0,
            words: vec![],
        },
        OcrLine {
            text: "volvendo uma aplicação.".to_string(),
            x: 40.0,
            y: 72.0,
            width: 190.0,
            height: 18.0,
            words: vec![],
        },
    ];

    let paragraphs = group_lines_into_paragraphs(&lines);
    assert_eq!(paragraphs.len(), 1);
    assert_eq!(paragraphs[0].text, "Estamos desenvolvendo uma aplicação.");
    assert_eq!(paragraphs[0].x, 40.0);
    assert_eq!(paragraphs[0].y, 50.0);
    assert_eq!(paragraphs[0].width, 200.0);
    assert_eq!(paragraphs[0].height, 40.0); // 72 + 18 - 50 = 40.0
}

#[test]
fn test_m1_join_paragraph_lines_helper() {
    let joined = join_paragraph_lines(&["Esta é uma frase de-", "monstrativa para teste."]);
    assert_eq!(joined, "Esta é uma frase demonstrativa para teste.");

    let joined_space = join_paragraph_lines(&["Primeira parte da linha,", "segunda parte da linha."]);
    assert_eq!(joined_space, "Primeira parte da linha, segunda parte da linha.");
}

#[test]
fn test_m1_non_merge_for_buttons_and_code() {
    // Buttons (Title case, UI elements) should NOT be merged
    let btn1 = BoundingBox {
        text: "Save Changes".to_string(),
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 25.0,
        translated_text: None,
    };
    let btn2 = BoundingBox {
        text: "Discard Draft".to_string(),
        x: 10.0,
        y: 50.0,
        width: 100.0,
        height: 25.0,
        translated_text: None,
    };
    let paras = group_bounding_boxes_into_paragraphs(&[btn1, btn2]);
    assert_eq!(paras.len(), 2, "Title-cased buttons must remain separate");

    // Code lines ending with ; or { should NOT be merged
    let code1 = BoundingBox {
        text: "fn main() {".to_string(),
        x: 10.0,
        y: 100.0,
        width: 120.0,
        height: 20.0,
        translated_text: None,
    };
    let code2 = BoundingBox {
        text: "    println!(\"hello\");".to_string(),
        x: 10.0,
        y: 125.0,
        width: 180.0,
        height: 20.0,
        translated_text: None,
    };
    let paras_code = group_bounding_boxes_into_paragraphs(&[code1, code2]);
    assert_eq!(paras_code.len(), 2, "Code lines must remain separate");
}

#[test]
fn test_m1_translation_pipeline_clustering_and_detected_lang() {
    tauri::async_runtime::block_on(async {
        let input_boxes = vec![
            TranslationBoxInput {
                text: "Mas eu não estou preocupada com isso,".to_string(),
                x: 50.0,
                y: 100.0,
                width: 250.0,
                height: 20.0,
            },
            TranslationBoxInput {
                text: "afinal de contas temos todo o tempo".to_string(),
                x: 50.0,
                y: 125.0,
                width: 240.0,
                height: 20.0,
            },
            TranslationBoxInput {
                text: "do mundo para resolver isso.".to_string(),
                x: 50.0,
                y: 150.0,
                width: 200.0,
                height: 20.0,
            },
        ];

        let res = translate_on_screen(
            input_boxes,
            Some("en".to_string()),
            Some("auto".to_string()),
        )
        .await;

        assert!(res.is_ok(), "translate_on_screen should succeed: {:?}", res.err());
        let response: TranslationResponse = res.unwrap();

        // 3 wrapped lines must be clustered into 1 translation box!
        assert_eq!(response.len(), 1, "Clustering must produce 1 unified translation box");
        let box0 = &response[0];
        assert_eq!(box0.x, 50.0);
        assert_eq!(box0.y, 100.0);
        assert_eq!(box0.width, 250.0);
        assert_eq!(box0.height, 70.0);
        assert_eq!(box0.is_paragraph, Some(true));

        println!("Translated paragraph: '{}'", box0.translated_text);
        assert!(!box0.translated_text.is_empty());
        // Verify translation does NOT fracture or invert
        let lower = box0.translated_text.to_lowercase();
        assert!(
            lower.contains("worry") || lower.contains("worried") || lower.contains("care"),
            "Translated text should convey meaning: '{}'",
            box0.translated_text
        );

        // Check detected language
        if let Some(ref detected) = response.detected_source_lang {
            println!("Detected source lang: {}", detected);
            assert_eq!(detected, "pt");
        }
    });
}

#[test]
fn test_m1_translation_response_deref_and_compat() {
    tauri::async_runtime::block_on(async {
        let res = translate_on_screen(vec![], Some("en".to_string()), None).await;
        assert!(res.is_ok());
        let response = res.unwrap();

        // Deref to slice
        assert!(response.is_empty());
        assert_eq!(response.len(), 0);

        // PartialEq with Vec<TranslationBoxOutput>
        assert_eq!(response, vec![]);
    });
}
