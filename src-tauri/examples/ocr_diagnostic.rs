//! Standalone diagnostic; also compiled by tools/ocr-diagnostic's minimal manifest.
#[allow(dead_code)]
#[path = "../src/ocr.rs"]
mod ocr;

use serde_json::{json, Value};
use std::{path::PathBuf, time::Instant};

fn variant(name: &str, result: ocr::OcrResult, elapsed_ms: f64, preprocessing_ms: f64, tsv: Option<String>) -> Value {
    let mut result = serde_json::to_value(result).expect("serializable OCR result");
    // WinRT does not expose recognition confidence. Application defaults must not
    // masquerade as engine measurements in comparisons.
    if cfg!(target_os = "windows") {
        if let Some(lines) = result["lines"].as_array_mut() {
            for line in lines {
                if let Some(words) = line["words"].as_array_mut() {
                    for word in words { word["confidence"] = Value::Null; }
                }
            }
        }
    }
    let mut out = json!({
        "name": name,
        "engine": if cfg!(target_os = "linux") { "tesseract" } else { "windows_ocr" },
        "elapsed_ms": elapsed_ms,
        "preprocessing_ms": preprocessing_ms,
        "confidence_scale": if cfg!(target_os = "windows") { "unavailable" } else { "0_100" },
        "confidence_origin": if cfg!(target_os = "windows") { "unavailable" } else if name == "current_pipeline" { "engine_and_application_heuristics" } else { "engine" },
        "result": result,
    });
    if let Some(tsv) = tsv { out["raw_tsv"] = Value::String(tsv); }
    out
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = Some(args.next().ok_or("Missing --input value")?.into()),
            "--output" => output = Some(args.next().ok_or("Missing --output value")?.into()),
            "--help" | "-h" => {
                println!("ocr_diagnostic --input IMAGE.png --output REPORT.json");
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    let input = input.ok_or("Required --input IMAGE.png")?;
    let output = output.ok_or("Required --output REPORT.json")?;
    if output.exists() && std::fs::canonicalize(&input).ok() == std::fs::canonicalize(&output).ok() {
        return Err("Output must differ from input image".into());
    }
    let image = image::open(&input).map_err(|e| format!("Read image: {e}"))?.to_rgba8();
    let start = Instant::now();
    let (raw, tsv) = ocr::extract_text_raw_for_diagnostic(&image, 1.0)?;
    let original = variant("original_raw", raw, start.elapsed().as_secs_f64() * 1000.0, 0.0, tsv);

    let start = Instant::now();
    let prep = ocr::preprocess_image_with_metadata(&image);
    let preprocessing_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let (raw, tsv) = ocr::extract_text_raw_for_diagnostic(&prep.processed_image, 1.0 / prep.scale)?;
    let preprocessed = variant("preprocessed_raw", raw, preprocessing_ms + start.elapsed().as_secs_f64() * 1000.0, preprocessing_ms, tsv);

    let start = Instant::now();
    let current = ocr::extract_text_native(&image)?;
    let current = variant("current_pipeline", current, start.elapsed().as_secs_f64() * 1000.0, 0.0, None);
    let report = json!({
        "schema_version": 1,
        "platform": std::env::consts::OS,
        "coordinates": "original_image_pixels",
        "image": { "path": input, "width": image.width(), "height": image.height() },
        "variants": [original, preprocessed, current],
        "timing_notes": "Sequential single runs; elapsed_ms includes preprocessing and engine process startup. current_pipeline preprocessing is included in elapsed_ms but not measured separately. No warmup or model download timing exclusion.",
    });
    let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(output, bytes).map_err(|e| format!("Write report: {e}"))?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ocr_diagnostic: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod diagnostic_tests {
    #[test]
    fn raw_parser_preserves_low_confidence_operators_spelling_and_scale() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\t1\t1\t1\t10\t20\t2\t8\t12\t=\n5\t1\t1\t1\t1\t2\t14\t20\t30\t8\t20\tcheck_runbers\n";
        let result = super::ocr::parse_tesseract_tsv_raw_for_diagnostic(tsv, 0.5);
        assert_eq!(result.full_text, "= check_runbers");
        assert_eq!(result.word_count, 2);
        assert_eq!(result.lines[0].words[0].confidence, 12.0);
        assert_eq!(result.lines[0].words[0].x, 5.0);
        assert_eq!(result.lines[0].words[0].width, 1.0);
        assert!(result.boxes.is_empty());
    }

    #[test]
    fn raw_parser_rejects_malformed_geometry_not_low_confidence() {
        let tsv = "header\n5\t1\t1\t1\t1\t1\tNaN\t0\t3\t8\t10\ti\n5\t1\t1\t1\t1\t2\t2\t0\t3\t8\t0\ti\n";
        let result = super::ocr::parse_tesseract_tsv_raw_for_diagnostic(tsv, 1.0);
        assert_eq!(result.full_text, "i");
        assert_eq!(result.word_count, 1);
    }
}
