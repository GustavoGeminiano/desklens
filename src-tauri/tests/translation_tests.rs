use desklens_lib::ocr::parse_tesseract_tsv;
use desklens_lib::translation::{
    normalize_target_lang, translate_on_screen, url_encode, TranslationBoxInput,
};

#[test]
fn test_language_normalization() {
    assert_eq!(normalize_target_lang(None), "pt");
    assert_eq!(normalize_target_lang(Some("")), "pt");
    assert_eq!(normalize_target_lang(Some("pt-BR")), "pt");
    assert_eq!(normalize_target_lang(Some("PT-PT")), "pt");
    assert_eq!(normalize_target_lang(Some("en-US")), "en");
    assert_eq!(normalize_target_lang(Some("es-ES")), "es");
    assert_eq!(normalize_target_lang(Some("de")), "de");
}

#[test]
fn test_url_encoding_special_chars() {
    assert_eq!(url_encode("Hello World!"), "Hello%20World%21");
    assert_eq!(url_encode("foo&bar=baz"), "foo%26bar%3Dbaz");
    assert_eq!(url_encode("100% real"), "100%25%20real");
}

#[test]
fn test_translate_empty_input() {
    tauri::async_runtime::block_on(async {
        let result = translate_on_screen(vec![], Some("pt".to_string())).await;
        assert!(result.is_ok());
        let outputs = result.unwrap();
        assert!(outputs.is_empty());
    });
}

#[test]
fn test_translate_bounding_box_preservation_and_fallback() {
    tauri::async_runtime::block_on(async {
        let input_boxes = vec![
            TranslationBoxInput {
                text: "DeskLens Visual Search".to_string(),
                x: 100.0,
                y: 150.0,
                width: 200.0,
                height: 30.0,
            },
            TranslationBoxInput {
                text: "Copy Text".to_string(),
                x: 100.0,
                y: 190.0,
                width: 120.0,
                height: 25.0,
            },
        ];

        let result = translate_on_screen(input_boxes.clone(), Some("pt-BR".to_string())).await;
        assert!(result.is_ok(), "translate_on_screen must succeed without panicking");
        let outputs = result.unwrap();

        assert_eq!(outputs.len(), 2);
        // Verify coordinates are strictly preserved
        assert_eq!(outputs[0].x, 100.0);
        assert_eq!(outputs[0].y, 150.0);
        assert_eq!(outputs[0].width, 200.0);
        assert_eq!(outputs[0].height, 30.0);
        assert_eq!(outputs[0].text, "DeskLens Visual Search");
        assert!(!outputs[0].translated_text.is_empty());

        assert_eq!(outputs[1].x, 100.0);
        assert_eq!(outputs[1].y, 190.0);
        assert_eq!(outputs[1].width, 120.0);
        assert_eq!(outputs[1].height, 25.0);
        assert_eq!(outputs[1].text, "Copy Text");
        assert!(!outputs[1].translated_text.is_empty());

        println!("Output 0 translation: '{}'", outputs[0].translated_text);
        println!("Output 1 translation: '{}'", outputs[1].translated_text);
    });
}

#[test]
fn test_ocr_to_translation_pipeline_contract() {
    // Verify that TSV parsed boxes directly feed into translation inputs
    let mock_tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t1920\t1080\t-1\t\n\
4\t1\t1\t1\t1\t0\t50\t60\t150\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t50\t60\t70\t20\t95\tSettings\n\
5\t1\t1\t1\t1\t2\t125\t60\t75\t20\t96\tPanel\n";

    let ocr_res = parse_tesseract_tsv(mock_tsv);
    assert_eq!(ocr_res.boxes.len(), 1);

    let trans_inputs: Vec<TranslationBoxInput> = ocr_res
        .boxes
        .into_iter()
        .map(|b| TranslationBoxInput {
            text: b.text,
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        })
        .collect();

    assert_eq!(trans_inputs.len(), 1);
    assert_eq!(trans_inputs[0].text, "Settings Panel");
    assert_eq!(trans_inputs[0].x, 50.0);
    assert_eq!(trans_inputs[0].y, 60.0);
    assert_eq!(trans_inputs[0].width, 150.0);
    assert_eq!(trans_inputs[0].height, 20.0);
}
