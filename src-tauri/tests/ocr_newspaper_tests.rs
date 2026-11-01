use desklens_lib::ocr::{
    coalesce_and_structure_lines, parse_tesseract_tsv_scaled, OcrLine, OcrWord,
};

#[test]
fn test_newspaper_low_confidence_hallucination_filtered() {
    // Simulates paper crease/shadow hallucinations in newspaper images:
    // Low-confidence blur text (< 35.0) and weak short tokens (< 45.0) must be discarded.
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t1000\t800\t-1\t\n\
2\t1\t1\t0\t0\t0\t10\t10\t900\t700\t-1\t\n\
3\t1\t1\t1\t0\t0\t10\t10\t900\t700\t-1\t\n\
4\t1\t1\t1\t1\t0\t10\t10\t500\t30\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t120\t25\t88.5\tNEWSPAPER\n\
5\t1\t1\t1\t1\t2\t140\t10\t80\t25\t91.0\tHEADLINE\n\
5\t1\t1\t1\t1\t3\t230\t10\t40\t25\t22.4\tblurred\n\
5\t1\t1\t1\t1\t4\t280\t10\t15\t25\t40.0\tto\n\
5\t1\t1\t1\t1\t5\t305\t10\t15\t25\t65.0\ton\n\
5\t1\t1\t1\t1\t6\t330\t10\t60\t25\t34.1\tghost\n";

    let result = parse_tesseract_tsv_scaled(tsv, 1.0);

    // "blurred" (conf 22.4 < 35.0) and "ghost" (conf 34.1 < 35.0) must be dropped.
    // "to" (2 chars, conf 40.0 < 45.0) must be dropped.
    // "on" (2 chars, conf 65.0 >= 45.0) must be kept.
    // "NEWSPAPER" and "HEADLINE" must be kept.
    assert_eq!(result.word_count, 3);
    assert!(result.full_text.contains("NEWSPAPER"));
    assert!(result.full_text.contains("HEADLINE"));
    assert!(result.full_text.contains("on"));
    assert!(!result.full_text.contains("blurred"));
    assert!(!result.full_text.contains("ghost"));
    assert!(!result.full_text.contains("to"));
}

#[test]
fn test_newspaper_vertical_margin_artifact_rejection() {
    // Real newspaper defect: vertical margin fold/border artifacts detected as tall narrow boxes.
    // Examples from live testing: "RT," with width=17, height=100; "ME" with width=50, height=188.
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t1000\t800\t-1\t\n\
2\t1\t1\t0\t0\t0\t0\t0\t1000\t800\t-1\t\n\
3\t1\t1\t1\t0\t0\t0\t0\t1000\t800\t-1\t\n\
4\t1\t1\t1\t1\t0\t1\t50\t17\t100\t-1\t\n\
5\t1\t1\t1\t1\t1\t1\t50\t17\t100\t78.0\tRT,\n\
4\t1\t1\t1\t2\t0\t950\t40\t50\t188\t-1\t\n\
5\t1\t1\t1\t2\t1\t950\t40\t50\t188\t82.0\tME\n\
4\t1\t1\t1\t3\t0\t100\t100\t400\t30\t-1\t\n\
5\t1\t1\t1\t3\t1\t100\t100\t150\t28\t95.0\tEDITORIAL\n\
5\t1\t1\t1\t3\t2\t260\t100\t120\t28\t93.0\tSECTION\n";

    let result = parse_tesseract_tsv_scaled(tsv, 1.0);

    // RT, (aspect ratio 100/17 = 5.88 > 2.8, len <= 3) must be filtered.
    // ME (aspect ratio 188/50 = 3.76 > 2.8, len <= 3) must be filtered.
    // EDITORIAL SECTION must be retained.
    assert_eq!(result.word_count, 2);
    assert!(!result.full_text.contains("RT,"));
    assert!(!result.full_text.contains("ME"));
    assert!(result.full_text.contains("EDITORIAL SECTION"));
}

#[test]
fn test_newspaper_isolated_noise_symbols_filtered() {
    // Crease lines, vertical separators, and print marks often produce single noise symbols.
    // Noise symbols below 75% confidence must be suppressed.
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t1000\t800\t-1\t\n\
2\t1\t1\t0\t0\t0\t10\t10\t900\t700\t-1\t\n\
3\t1\t1\t1\t0\t0\t10\t10\t900\t700\t-1\t\n\
4\t1\t1\t1\t1\t0\t10\t10\t500\t30\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t5\t20\t55.0\t|\n\
5\t1\t1\t1\t1\t2\t30\t10\t100\t20\t92.0\tArticle\n\
5\t1\t1\t1\t1\t3\t140\t10\t15\t5\t60.0\t—\n\
5\t1\t1\t1\t1\t4\t165\t10\t80\t20\t90.0\tSummary\n\
5\t1\t1\t1\t1\t5\t255\t10\t15\t5\t85.0\t-\n";

    let result = parse_tesseract_tsv_scaled(tsv, 1.0);

    // '|' with conf 55.0 < 75.0 must be dropped.
    // '—' with conf 60.0 < 75.0 must be dropped.
    // '-' with conf 85.0 >= 75.0 must be kept (valid punctuation/hyphen).
    // "Article" and "Summary" must be kept.
    assert!(!result.full_text.contains('|'));
    assert!(!result.full_text.contains('—'));
    assert!(result.full_text.contains("Article"));
    assert!(result.full_text.contains("Summary"));
    assert!(result.full_text.contains("-"));
}

#[test]
fn test_newspaper_sub_3px_sanity_filtering() {
    // Printer dots, scanner dust, and sub-3px noise boxes must be discarded.
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t1000\t800\t-1\t\n\
2\t1\t1\t0\t0\t0\t10\t10\t900\t700\t-1\t\n\
3\t1\t1\t1\t0\t0\t10\t10\t900\t700\t-1\t\n\
4\t1\t1\t1\t1\t0\t10\t10\t300\t30\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t2\t15\t90.0\t.\n\
5\t1\t1\t1\t1\t2\t20\t10\t15\t2\t90.0\t_\n\
5\t1\t1\t1\t1\t3\t50\t10\t60\t20\t94.0\tValid\n";

    let result = parse_tesseract_tsv_scaled(tsv, 1.0);

    assert_eq!(result.word_count, 1);
    assert_eq!(result.full_text.trim(), "Valid");
}

#[test]
fn test_newspaper_multi_column_gutter_preservation() {
    // Multi-column newspaper page:
    // Column 1 and Column 2 are separated by a 70px gutter.
    // They must NOT be merged into a single line across the gutter!
    let raw_lines = vec![
        OcrLine {
            text: "LOCAL CITY COUNCIL MEETS".to_string(),
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 18.0,
            words: vec![
                OcrWord { text: "LOCAL".to_string(), x: 50.0, y: 100.0, width: 45.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "CITY".to_string(), x: 100.0, y: 100.0, width: 35.0, height: 18.0, confidence: 95.0 },
                OcrWord { text: "COUNCIL".to_string(), x: 140.0, y: 100.0, width: 60.0, height: 18.0, confidence: 94.0 },
                OcrWord { text: "MEETS".to_string(), x: 205.0, y: 100.0, width: 45.0, height: 18.0, confidence: 96.0 },
            ],
        },
        OcrLine {
            text: "MARKET SURGES TO RECORD".to_string(),
            x: 330.0, // Gutter from col 1 right edge (250.0) is 80.0px
            y: 101.0,
            width: 210.0,
            height: 18.0,
            words: vec![
                OcrWord { text: "MARKET".to_string(), x: 330.0, y: 101.0, width: 55.0, height: 18.0, confidence: 93.0 },
                OcrWord { text: "SURGES".to_string(), x: 390.0, y: 101.0, width: 55.0, height: 18.0, confidence: 92.0 },
                OcrWord { text: "TO".to_string(), x: 450.0, y: 101.0, width: 20.0, height: 18.0, confidence: 96.0 },
                OcrWord { text: "RECORD".to_string(), x: 475.0, y: 101.0, width: 65.0, height: 18.0, confidence: 95.0 },
            ],
        },
    ];

    let (lines, full_text, _) = coalesce_and_structure_lines(raw_lines);

    // Must remain 2 separate lines!
    assert_eq!(lines.len(), 2, "Column 1 and Column 2 must not be merged across gutter");
    assert!(lines[0].text.contains("LOCAL CITY COUNCIL MEETS"));
    assert!(lines[1].text.contains("MARKET SURGES TO RECORD"));
    assert!(full_text.contains("LOCAL CITY COUNCIL MEETS"));
    assert!(full_text.contains("MARKET SURGES TO RECORD"));
}

#[test]
fn test_newspaper_intra_column_fragments_coalesced() {
    // Within the same newspaper column, OCR may split a line horizontally.
    // For example, due to spacing or punctuation: "BREAKING NEWS:" and "CIVIC CENTER OPENS"
    // with a normal word gap (15px). These must be coalesced into a single line.
    let raw_lines = vec![
        OcrLine {
            text: "BREAKING NEWS:".to_string(),
            x: 50.0,
            y: 100.0,
            width: 120.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "BREAKING".to_string(), x: 50.0, y: 100.0, width: 70.0, height: 20.0, confidence: 95.0 },
                OcrWord { text: "NEWS:".to_string(), x: 125.0, y: 100.0, width: 45.0, height: 20.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "CIVIC CENTER OPENS".to_string(),
            x: 185.0, // Gap is 185.0 - (50.0 + 120.0 = 170.0) = 15.0px <= max_allowed_gap (70px)
            y: 100.5,
            width: 140.0,
            height: 20.0,
            words: vec![
                OcrWord { text: "CIVIC".to_string(), x: 185.0, y: 100.5, width: 40.0, height: 20.0, confidence: 94.0 },
                OcrWord { text: "CENTER".to_string(), x: 230.0, y: 100.5, width: 50.0, height: 20.0, confidence: 93.0 },
                OcrWord { text: "OPENS".to_string(), x: 285.0, y: 100.5, width: 40.0, height: 20.0, confidence: 96.0 },
            ],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);

    assert_eq!(lines.len(), 1, "Split fragments within same column must coalesce");
    assert_eq!(lines[0].words.len(), 5);
    assert!(lines[0].text.contains("BREAKING NEWS:"));
    assert!(lines[0].text.contains("CIVIC CENTER OPENS"));
}

#[test]
fn test_newspaper_three_column_layout_separation() {
    // Realistic 3-column newspaper article:
    // Col 1: x in [40..200]
    // Col 2: x in [260..420] (60px gutter)
    // Col 3: x in [480..640] (60px gutter)
    // All 3 columns share the same vertical baseline.
    let raw_lines = vec![
        OcrLine {
            text: "POLITICS TODAY".to_string(),
            x: 40.0,
            y: 80.0,
            width: 140.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "POLITICS".to_string(), x: 40.0, y: 80.0, width: 70.0, height: 16.0, confidence: 96.0 },
                OcrWord { text: "TODAY".to_string(), x: 115.0, y: 80.0, width: 50.0, height: 16.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "BUSINESS DESK".to_string(),
            x: 260.0,
            y: 80.0,
            width: 130.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "BUSINESS".to_string(), x: 260.0, y: 80.0, width: 70.0, height: 16.0, confidence: 94.0 },
                OcrWord { text: "DESK".to_string(), x: 335.0, y: 80.0, width: 45.0, height: 16.0, confidence: 95.0 },
            ],
        },
        OcrLine {
            text: "SPORTS ROUNDUP".to_string(),
            x: 480.0,
            y: 80.0,
            width: 140.0,
            height: 16.0,
            words: vec![
                OcrWord { text: "SPORTS".to_string(), x: 480.0, y: 80.0, width: 60.0, height: 16.0, confidence: 95.0 },
                OcrWord { text: "ROUNDUP".to_string(), x: 545.0, y: 80.0, width: 75.0, height: 16.0, confidence: 94.0 },
            ],
        },
    ];

    let (lines, _, _) = coalesce_and_structure_lines(raw_lines);

    assert_eq!(lines.len(), 3, "All 3 newspaper columns must remain separate");
    assert_eq!(lines[0].text.trim(), "POLITICS TODAY");
    assert_eq!(lines[1].text.trim(), "BUSINESS DESK");
    assert_eq!(lines[2].text.trim(), "SPORTS ROUNDUP");
}

#[test]
fn test_newspaper_full_page_tsv_realistic_simulation() {
    // End-to-end simulation of a scanned newspaper page:
    // - Headline across top
    // - Two articles side-by-side with gutter
    // - Left edge paper fold artifact: "RT," (17x100)
    // - Crease shadow hallucination: "asdfgh" (conf 19.0)
    // - Isolated ink mark: "|" (conf 45.0)
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t1200\t1600\t-1\t\n\
2\t1\t1\t0\t0\t0\t0\t0\t1200\t1600\t-1\t\n\
3\t1\t1\t1\t0\t0\t0\t0\t1200\t1600\t-1\t\n\
4\t1\t1\t1\t1\t0\t2\t10\t17\t100\t-1\t\n\
5\t1\t1\t1\t1\t1\t2\t10\t17\t100\t75.0\tRT,\n\
4\t1\t1\t1\t2\t0\t100\t50\t800\t40\t-1\t\n\
5\t1\t1\t1\t2\t1\t100\t50\t350\t38\t98.0\tTHE\n\
5\t1\t1\t1\t2\t2\t470\t50\t430\t38\t97.0\tCHRONICLE\n\
4\t1\t1\t1\t3\t0\t100\t120\t350\t20\t-1\t\n\
5\t1\t1\t1\t3\t1\t100\t120\t100\t18\t93.0\tEconomy\n\
5\t1\t1\t1\t3\t2\t210\t120\t80\t18\t92.0\tShows\n\
5\t1\t1\t1\t3\t3\t300\t120\t80\t18\t95.0\tGrowth\n\
4\t1\t1\t1\t4\t0\t550\t120\t350\t20\t-1\t\n\
5\t1\t1\t1\t4\t1\t550\t120\t110\t18\t94.0\tCommunity\n\
5\t1\t1\t1\t4\t2\t670\t120\t60\t18\t91.0\tBuilds\n\
5\t1\t1\t1\t4\t3\t740\t120\t50\t18\t96.0\tPark\n\
4\t1\t1\t1\t5\t0\t480\t125\t8\t22\t-1\t\n\
5\t1\t1\t1\t5\t1\t480\t125\t8\t22\t45.0\t|\n\
4\t1\t1\t1\t6\t0\t100\t200\t200\t20\t-1\t\n\
5\t1\t1\t1\t6\t1\t100\t200\t60\t16\t19.0\tasdfgh\n";

    let result = parse_tesseract_tsv_scaled(tsv, 1.0);

    // Verify all margin artifacts, blur hallucinations, and noise symbols are purged
    assert!(!result.full_text.contains("RT,"));
    assert!(!result.full_text.contains("asdfgh"));
    assert!(!result.full_text.contains('|'));

    // Verify real text was preserved
    assert!(result.full_text.contains("THE CHRONICLE"));
    assert!(result.full_text.contains("Economy Shows Growth"));
    assert!(result.full_text.contains("Community Builds Park"));

    // Verify confidence field exists and is populated
    for line in &result.lines {
        for word in &line.words {
            assert!(word.confidence > 0.0, "Word confidence must be populated: {:?}", word.text);
        }
    }
}

#[test]
fn test_newspaper_narrow_characters_preserved_in_sentence() {
    // Standard sentence containing narrow typographical characters with aspect ratio > 2.8:
    // 'I' (w=5, h=20, ratio=4.0)
    // '1' (w=5, h=20, ratio=4.0)
    // '!' (w=4, h=20, ratio=5.0)
    // These valid single characters must NEVER be dropped.
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t10\t10\t300\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t5\t20\t92.0\tI\n\
5\t1\t1\t1\t1\t2\t25\t10\t30\t20\t95.0\tsee\n\
5\t1\t1\t1\t1\t3\t65\t10\t5\t20\t90.0\t1\n\
5\t1\t1\t1\t1\t4\t80\t10\t40\t20\t93.0\titem\n\
5\t1\t1\t1\t1\t5\t130\t10\t5\t20\t91.0\t!\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.word_count, 5, "All 5 words including 'I', '1', and '!' must be retained");
    assert!(res.full_text.contains("I"), "Must contain 'I'");
    assert!(res.full_text.contains("see"), "Must contain 'see'");
    assert!(res.full_text.contains("1"), "Must contain '1'");
    assert!(res.full_text.contains("item"), "Must contain 'item'");
    assert!(res.full_text.contains("!"), "Must contain '!'");
}

#[test]
fn test_newspaper_code_syntax_and_brackets_preserved() {
    // Code syntax with narrow symbols: `|x| [0]`
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t10\t10\t200\t20\t-1\t\n\
5\t1\t1\t1\t1\t1\t10\t10\t4\t20\t95.0\t|\n\
5\t1\t1\t1\t1\t2\t20\t10\t10\t20\t95.0\tx\n\
5\t1\t1\t1\t1\t3\t35\t10\t4\t20\t95.0\t|\n\
5\t1\t1\t1\t1\t4\t50\t10\t6\t20\t95.0\t[\n\
5\t1\t1\t1\t1\t5\t60\t10\t10\t20\t95.0\t0\n\
5\t1\t1\t1\t1\t6\t75\t10\t6\t20\t95.0\t]\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert_eq!(res.word_count, 6, "All 6 syntax tokens must be preserved");
    assert!(res.full_text.contains("|"));
    assert!(res.full_text.contains("x"));
    assert!(res.full_text.contains("["));
    assert!(res.full_text.contains("0"));
    assert!(res.full_text.contains("]"));
}

#[test]
fn test_newspaper_headline_gutter_separation_40px_and_above() {
    // Large headline font (height=36px) across varying gutters.
    // Gutters of 40px, 50px, 80px, 120px must NEVER be merged into a single line.
    for &gutter in &[40.0, 50.0, 80.0, 100.0, 120.0, 130.0] {
        let col1_x = 50.0;
        let col1_w = 200.0;
        let col2_x = col1_x + col1_w + gutter;
        let col2_w = 200.0;

        let raw_lines = vec![
            OcrLine {
                text: "TOP NEWS".to_string(),
                x: col1_x,
                y: 50.0,
                width: col1_w,
                height: 36.0,
                words: vec![
                    OcrWord { text: "TOP".to_string(), x: col1_x, y: 50.0, width: 80.0, height: 36.0, confidence: 96.0 },
                    OcrWord { text: "NEWS".to_string(), x: col1_x + 95.0, y: 50.0, width: 105.0, height: 36.0, confidence: 96.0 },
                ],
            },
            OcrLine {
                text: "GOOD NEWS".to_string(),
                x: col2_x,
                y: 50.0,
                width: col2_w,
                height: 36.0,
                words: vec![
                    OcrWord { text: "GOOD".to_string(), x: col2_x, y: 50.0, width: 90.0, height: 36.0, confidence: 95.0 },
                    OcrWord { text: "NEWS".to_string(), x: col2_x + 100.0, y: 50.0, width: 100.0, height: 36.0, confidence: 95.0 },
                ],
            },
        ];

        let (lines, _, _) = coalesce_and_structure_lines(raw_lines);
        assert_eq!(
            lines.len(),
            2,
            "Headlines across gutter {}px must remain 2 separate lines, not merge",
            gutter
        );
    }
}

#[test]
fn test_newspaper_nan_and_inf_confidence_discarded() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t10\t10\t80\t20\tNaN\tNanWord\n\
5\t1\t1\t1\t1\t2\t100\t10\t80\t20\tinf\tInfWord\n\
5\t1\t1\t1\t1\t3\t200\t10\t80\t20\t-inf\tNegInfWord\n\
5\t1\t1\t1\t1\t4\t300\t10\t80\t20\t-1\tNegOneWord\n\
5\t1\t1\t1\t1\t5\t400\t10\t80\t20\t90.0\tNormalWord\n";

    let res = parse_tesseract_tsv_scaled(tsv, 1.0);
    assert!(!res.full_text.contains("NanWord"), "NaN word must be discarded");
    assert!(!res.full_text.contains("InfWord"), "Inf word must be discarded");
    assert!(!res.full_text.contains("NegInfWord"), "NegInf word must be discarded");
    assert!(!res.full_text.contains("NegOneWord"), "-1 word must be discarded");
    assert!(res.full_text.contains("NormalWord"), "Normal word must be retained");
    assert_eq!(res.word_count, 1);
}

