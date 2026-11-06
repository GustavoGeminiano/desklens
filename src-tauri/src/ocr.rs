use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub confidence: f32,
}

impl Default for OcrWord {
    fn default() -> Self {
        Self {
            text: String::new(),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            confidence: 100.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OcrLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub words: Vec<OcrWord>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ParagraphBox {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub line_indices: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub translated_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub is_paragraph: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BoundingBox {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub translated_text: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OcrResult {
    pub full_text: String,
    pub lines: Vec<OcrLine>,
    pub word_count: usize,
    #[serde(default)]
    pub boxes: Vec<BoundingBox>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paragraphs: Vec<ParagraphBox>,
}

/// Computes the bounding box enclosing a slice of words (union fallback).
pub fn compute_words_bounding_box(words: &[OcrWord]) -> (f64, f64, f64, f64) {
    if words.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let min_x = words.iter().map(|w| w.x).fold(f64::INFINITY, f64::min);
    let min_y = words.iter().map(|w| w.y).fold(f64::INFINITY, f64::min);
    let max_x = words.iter().map(|w| w.x + w.width).fold(f64::NEG_INFINITY, f64::max);
    let max_y = words.iter().map(|w| w.y + w.height).fold(f64::NEG_INFINITY, f64::max);

    (min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
}


/// Detects whether an image has a predominantly dark background (e.g. dark mode editor or terminal).
pub fn is_dark_image(image: &image::RgbaImage) -> bool {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return false;
    }

    let step = ((w * h) / 10000).max(1) as usize;
    let mut total_luma: u64 = 0;
    let mut count: u64 = 0;

    let raw = image.as_raw();
    let num_pixels = (w * h) as usize;
    for i in (0..num_pixels).step_by(step) {
        let idx = i * 4;
        if idx + 3 < raw.len() {
            let a = raw[idx + 3];
            if a > 32 {
                let r = raw[idx] as u32;
                let g = raw[idx + 1] as u32;
                let b = raw[idx + 2] as u32;
                let luma = (r * 299 + g * 587 + b * 114) / 1000;
                total_luma += luma as u64;
                count += 1;
            }
        }
    }

    if count == 0 {
        return false;
    }
    (total_luma / count) < 128
}

/// Inverts RGB channels of an image to turn dark background into light background for high-fidelity OCR.
pub fn invert_image(image: &image::RgbaImage) -> image::RgbaImage {
    let mut inverted = image.clone();
    for pixel in inverted.pixels_mut() {
        pixel[0] = 255 - pixel[0];
        pixel[1] = 255 - pixel[1];
        pixel[2] = 255 - pixel[2];
    }
    inverted
}

/// Enhances contrast of an image by stretching the dynamic range of pixel luminances.
/// Scales luminance proportionally across RGB channels to preserve chromaticity and syntax colors,
/// while mapping background to clean white.
pub fn enhance_contrast(image: &mut image::RgbaImage) {
    let mut min_luma = 255u8;
    let mut max_luma = 0u8;
    for pixel in image.pixels() {
        if pixel[3] > 0 {
            let luma = ((pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000) as u8;
            min_luma = min_luma.min(luma);
            max_luma = max_luma.max(luma);
        }
    }

    if max_luma > min_luma + 20 {
        let range = (max_luma - min_luma) as f32;
        for pixel in image.pixels_mut() {
            if pixel[3] > 0 {
                let luma = ((pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000) as f32;
                if luma >= (max_luma as f32 - 6.0) {
                    pixel[0] = 255;
                    pixel[1] = 255;
                    pixel[2] = 255;
                } else {
                    let norm = (luma - min_luma as f32).max(0.0) / range;
                    let target_luma = norm.powf(1.15) * 255.0;
                    if luma > 0.0 {
                        let k = (target_luma / luma).min(1.0);
                        pixel[0] = ((pixel[0] as f32 * k).round()).clamp(0.0, 255.0) as u8;
                        pixel[1] = ((pixel[1] as f32 * k).round()).clamp(0.0, 255.0) as u8;
                        pixel[2] = ((pixel[2] as f32 * k).round()).clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
}

/// Preprocesses screenshot crop for optimal code OCR precision:
/// - Inverts dark theme images to black-on-white.
/// - Enhances contrast across color spectrum while preserving syntax highlight chromaticity.
/// - Upscales small screen text (font height 8-16px) by 2x using crisp Nearest-neighbor interpolation.
/// - Restricts dimensions to fit within WinRT OCR max size (2500px).
pub fn preprocess_for_ocr(image: &image::RgbaImage) -> (image::RgbaImage, f64) {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return (image.clone(), 1.0);
    }

    let mut prepared = if is_dark_image(image) {
        invert_image(image)
    } else {
        image.clone()
    };

    enhance_contrast(&mut prepared);

    const MAX_OCR_DIM: f64 = 2500.0;
    let scale = if (w as f64 * 2.0) <= MAX_OCR_DIM && (h as f64 * 2.0) <= MAX_OCR_DIM && (w < 1600 || h < 1600) {
        2.0
    } else if (w as f64) > MAX_OCR_DIM || (h as f64) > MAX_OCR_DIM {
        let factor = MAX_OCR_DIM / (w.max(h) as f64);
        (factor * 100.0).floor() / 100.0
    } else {
        1.0
    };

    if (scale - 1.0).abs() > 0.001 {
        let new_w = ((w as f64) * scale).round() as u32;
        let new_h = ((h as f64) * scale).round() as u32;
        let resized = image::imageops::resize(&prepared, new_w, new_h, image::imageops::FilterType::Lanczos3);
        (resized, scale)
    } else {
        (prepared, 1.0)
    }
}

/// Checks if any word in words_a has a significant horizontal overlap with any word in words_b.
/// Two distinct lines sharing the same column will have overlapping words, while split fragments of
/// the same visual line occupy non-overlapping horizontal segments or fit into word gaps.
pub fn words_collide_horizontally(words_a: &[OcrWord], words_b: &[OcrWord]) -> bool {
    for wa in words_a {
        for wb in words_b {
            let left = wa.x.max(wb.x);
            let right = (wa.x + wa.width).min(wb.x + wb.width);
            let h_overlap = (right - left).max(0.0);
            let min_w = wa.width.min(wb.width);
            if min_w > 0.0 {
                let ratio = h_overlap / min_w;
                if ratio >= 0.35 || (ratio >= 0.20 && h_overlap >= 4.0) {
                    return true;
                }
            }
        }
    }
    false
}

/// Normalizes typographic and fullwidth substitutions from OCR models into standard programming language syntax.
pub fn normalize_code_punctuation(text: &str) -> String {
    // 1. Unified mapping of all Fullwidth ASCII characters (U+FF01 '！' to U+FF5E '～')
    // and fullwidth ideographic space (U+3000)
    let mut s = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\u{3000}' {
            s.push(' ');
        } else if ('\u{FF01}'..='\u{FF5E}').contains(&c) {
            if let Some(ascii_c) = char::from_u32(c as u32 - 0xFEE0) {
                s.push(ascii_c);
            } else {
                s.push(c);
            }
        } else {
            s.push(c);
        }
    }

    // 2. Typographic double quotes to standard ASCII double quote
    s = s.replace(
        ['\u{201C}', '\u{201D}', '\u{201E}', '\u{201F}', '\u{2033}', '\u{2035}', '\u{00AB}', '\u{00BB}'],
        "\"",
    );

    // 3. Typographic single quotes / accents / primes to ASCII single quote
    s = s.replace(
        ['\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}', '\u{00B4}', '\u{2032}'],
        "'",
    );

    // 4. Typographic dashes / minus / en-dash / em-dash to hyphen-minus
    s = s.replace(
        ['\u{2014}', '\u{2013}', '\u{2212}', '\u{2015}'],
        "-",
    );

    // 5. Code font ligatures, arrows, and comparisons to programming syntax
    s = s.replace('\u{2261}', "===");
    s = s.replace('\u{2262}', "!==");
    s = s.replace('\u{2260}', "!=");
    s = s.replace('\u{2264}', "<=");
    s = s.replace('\u{2265}', ">=");
    s = s.replace(['\u{2192}', '\u{27F6}'], "->");
    s = s.replace(['\u{21D2}', '\u{27F9}'], "=>");
    s = s.replace('\u{2190}', "<-");
    s = s.replace('\u{2026}', "...");
    s = s.replace('\u{00D7}', "*");
    s = s.replace('\u{00F7}', "/");

    // 6. Non-breaking spaces and zero-width formatters
    s = s.replace(['\u{00A0}', '\u{2007}', '\u{202F}'], " ");
    s = s.replace(['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'], "");

    // 7. Simplify OCR quotation mark doubling and mismatched quote artifacts
    s = s.replace("'\"", "'").replace("\"'", "'");
    if s.ends_with("l')")
        || s.ends_with("'l)")
        || s.ends_with("l\")")
        || s.ends_with("\"l)")
        || s.ends_with("1')")
        || s.ends_with("'1)")
    {
        s = format!("{}\")", &s[..s.len() - 3]);
    }
    if s.starts_with('"') && (s.ends_with("':") || s.ends_with("',") || s.ends_with("')")) {
        s = format!("'{}", &s[1..]);
    }

    // 8. Programming keywords and OCR letter substitution normalizations
    if s == "eIse:" || s == "eise:" || s == "Ise:" || s == "élse:" || s == "Else:" {
        s = "else:".to_string();
    } else if s == "eIse" || s == "eise" || s == "élse" {
        s = "else".to_string();
    } else if s == "faIse" || s == "faise" {
        s = "false".to_string();
    } else if s == "nuII" || s == "nuii" {
        s = "null".to_string();
    } else if s == "whiIe" {
        s = "while".to_string();
    } else if s == "whiIe:" {
        s = "while:".to_string();
    } else if s == "cIass" {
        s = "class".to_string();
    }

    s
}

/// Returns true if the token is a recognized programming operator, symbol, or punctuation.
pub fn is_code_op(s: &str) -> bool {
    matches!(
        s.trim(),
        "===" | "!==" | ">>>"
            | "==" | "!=" | "<=" | ">=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&&" | "||"
            | "->" | "=>" | "::" | ".." | "<<" | ">>" | "++" | "--" | "**" | "//"
            | "=" | "+" | "-" | "*" | "/" | "%" | "<" | ">" | "!" | "&" | "|" | "^"
            | "~" | ":" | ";" | "?" | "." | "," | "(" | ")" | "[" | "]" | "{" | "}"
    )
}

/// Returns true if the token is a recognized programming keyword (ignoring trailing syntax delimiters like ':', ';', ',').
pub fn is_code_keyword(s: &str) -> bool {
    let clean = s.trim().trim_end_matches([':', ';', ',']);
    matches!(
        clean.to_ascii_lowercase().as_str(),
        "if" | "in" | "is" | "fn" | "as" | "do" | "to" | "go" | "or" | "for" | "def" | "let"
            | "var" | "val" | "try" | "mut" | "ref" | "use" | "pub" | "mod" | "new" | "get"
            | "set" | "else" | "elif" | "case" | "loop" | "end" | "nil" | "not" | "and"
            | "pass" | "with" | "from" | "return" | "class" | "import" | "while" | "yield"
            | "await" | "async" | "break" | "continue" | "struct" | "enum" | "impl" | "type"
            | "trait" | "const" | "static" | "where" | "super" | "self" | "crate" | "except"
            | "finally" | "match"
    )
}

/// Segments composite or glued code tokens into fine-grained tokens with tightly wrapped, non-overlapping bounding boxes.
pub fn segment_code_tokens(words: Vec<OcrWord>) -> Vec<OcrWord> {
    let mut intermediate: Vec<OcrWord> = Vec::with_capacity(words.len() * 2);

    // 1. Coalesce adjacent keyword fragment splits like 'e' + 'Ise:' -> 'else:'
    let mut i = 0;
    while i < words.len() {
        if i + 1 < words.len() {
            let w1_text = words[i].text.to_lowercase();
            let w2_text = words[i + 1].text.to_lowercase();
            if (w1_text == "e"
                && (w2_text == "ise:"
                    || w2_text == "lse:"
                    || w2_text == "ise"
                    || w2_text == "lse"
                    || w2_text == "else:"
                    || w2_text == "else"))
                || (w1_text == "el" && (w2_text == "se:" || w2_text == "se"))
            {
                let gap = words[i + 1].x - (words[i].x + words[i].width);
                if gap <= 15.0 {
                    let combined_w = (words[i + 1].x + words[i + 1].width) - words[i].x;
                    intermediate.push(OcrWord {
                        text: if w2_text.ends_with(':') { "else:".to_string() } else { "else".to_string() },
                        x: words[i].x,
                        y: words[i].y.min(words[i + 1].y),
                        width: combined_w,
                        height: words[i].height.max(words[i + 1].height),
                        confidence: (words[i].confidence + words[i + 1].confidence) * 0.5,
                    });
                    i += 2;
                    continue;
                }
            }
        }
        intermediate.push(words[i].clone());
        i += 1;
    }

    // 2. Segment each word into tokens if glued
    let mut result = Vec::with_capacity(intermediate.len() * 2);
    for word in intermediate {
        let segments = segment_single_code_word(&word);
        result.extend(segments);
    }

    result
}

fn segment_single_code_word(word: &OcrWord) -> Vec<OcrWord> {
    let text = &word.text;
    if text.is_empty() {
        return vec![word.clone()];
    }

    // Do not split already recognized single or double character operators
    if is_code_op(text) {
        return vec![word.clone()];
    }

    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n <= 1 {
        return vec![word.clone()];
    }

    let mut split_points: Vec<usize> = Vec::new();

    // Case A: Keyword prefix followed by a single-character identifier or operator/symbol
    const KEYWORDS: &[&str] = &[
        "for", "if", "in", "is", "def", "let", "var", "fn", "not", "and", "or", "as", "do",
        "while", "else", "return", "class", "import", "from", "try", "except", "with", "yield",
        "await", "async",
    ];
    for &kw in KEYWORDS {
        if text.starts_with(kw) && text.len() > kw.len() {
            let kw_len = kw.chars().count();
            let tail: Vec<char> = chars[kw_len..].to_vec();
            // Single character identifier e.g. 'fori' -> 'for', 'i'
            if tail.len() == 1 && tail[0].is_alphabetic() {
                split_points.push(kw_len);
                break;
            }
            // Identifier char followed by operator: e.g. 'ifi%2' -> 'if', 'i'
            if tail.len() >= 2
                && tail[0].is_alphabetic()
                && matches!(
                    tail[1],
                    '(' | '[' | '{' | '%' | '=' | '!' | '<' | '>' | '&' | '|' | '+' | '-' | '*' | '/' | ':' | ';'
                )
            {
                split_points.push(kw_len);
                split_points.push(kw_len + 1);
                break;
            }
            // Followed by operator or parenthesis e.g. 'for(' or 'if('
            if let Some(&first_ch) = tail.first() {
                if matches!(first_ch, '(' | '[' | '{' | '%' | '=' | '!' | '<' | '>' | '&' | '|') {
                    split_points.push(kw_len);
                    break;
                }
            }
        }
    }

    // Case B: Function call glued to string literal: e.g. 'print(f"%i' -> split before '"'
    if let Some(quote_idx) = chars.iter().position(|&c| c == '"' || c == '\'') {
        if quote_idx > 0 && quote_idx < n {
            let prev = chars[quote_idx - 1];
            if prev == '(' || prev == '[' || prev == '{' || prev == 'f' || prev == 'r' || prev == 'b'
                || prev == '=' || prev == ':' || prev == ','
            {
                if !split_points.contains(&quote_idx) {
                    split_points.push(quote_idx);
                }
            }
        }
    }

    // Case B2: Opening delimiter glued at start of word (e.g. '(i', '(x', '[0')
    if (chars[0] == '(' || chars[0] == '[' || chars[0] == '{')
        && !chars.contains(&')')
        && !chars.contains(&']')
        && !chars.contains(&'}')
    {
        if !split_points.contains(&1) {
            split_points.push(1);
        }
    }

    // Case B3: Unmatched closing delimiter and statement delimiter (e.g. '0):', '1):', '):')
    // Function calls with argument lists like 'range(10):' contain alphabetic chars before '(' and are preserved.
    if !chars.contains(&'(') && !chars.contains(&'[') && !chars.contains(&'{') {
        for (ci, &c) in chars.iter().enumerate() {
            if c == ')' || c == ']' || c == '}' {
                if ci > 0 && chars[ci - 1].is_alphanumeric() && !split_points.contains(&ci) {
                    split_points.push(ci);
                }
                if ci + 1 < n
                    && (chars[ci + 1] == ':' || chars[ci + 1] == ';' || chars[ci + 1] == ',')
                    && !split_points.contains(&(ci + 1))
                {
                    split_points.push(ci + 1);
                }
            }
        }
    }

    // Case C: Embedded operators between alphanumerics (do not split operators inside string quotes)
    let mut in_quote: Option<char> = None;
    let mut idx = 0;
    while idx < n {
        let c = chars[idx];
        if in_quote.is_none() && (c == '"' || c == '\'') {
            in_quote = Some(c);
            idx += 1;
            continue;
        } else if in_quote == Some(c) {
            in_quote = None;
            idx += 1;
            continue;
        }

        if in_quote.is_some() {
            idx += 1;
            continue;
        }

        // 3-char operators: ===, !==, >>>
        if idx + 2 < n {
            let three_char: String = chars[idx..=idx + 2].iter().collect();
            if matches!(three_char.as_str(), "===" | "!==" | ">>>") {
                if idx > 0 && !split_points.contains(&idx) {
                    split_points.push(idx);
                }
                if idx + 3 < n && !split_points.contains(&(idx + 3)) {
                    split_points.push(idx + 3);
                }
                idx += 3;
                continue;
            }
        }

        // Multi-char operators: ==, !=, <=, >=, +=, -=, *=, /=, %=, &&, ||, ->, =>, ::, .., <<, >>, ++, --, **, //
        if idx + 1 < n {
            let two_char: String = chars[idx..=idx + 1].iter().collect();
            if matches!(
                two_char.as_str(),
                "==" | "!=" | "<=" | ">=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&&" | "||"
                    | "->" | "=>" | "::" | ".." | "<<" | ">>" | "++" | "--" | "**" | "//"
            ) {
                if idx > 0 && !split_points.contains(&idx) {
                    split_points.push(idx);
                }
                if idx + 2 < n && !split_points.contains(&(idx + 2)) {
                    split_points.push(idx + 2);
                }
                idx += 2;
                continue;
            }
        }

        // Single char operators: %, =, +, -, *, /, <, >, !, &, |, ^, ~, ?, ;, ,
        let c = chars[idx];
        if matches!(c, '%' | '=' | '+' | '-' | '*' | '/' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?' | ';' | ',') {
            if idx > 0 && !split_points.contains(&idx) {
                split_points.push(idx);
            }
            if idx + 1 < n && !split_points.contains(&(idx + 1)) {
                split_points.push(idx + 1);
            }
        } else if c == ':' && idx > 0 && chars[idx - 1].is_numeric() {
            // e.g. '0:' -> split between '0' and ':'
            if !split_points.contains(&idx) {
                split_points.push(idx);
            }
        }

        idx += 1;
    }

    if split_points.is_empty() {
        return vec![word.clone()];
    }

    split_at_multiple_offsets(word, &split_points)
}

fn split_at_multiple_offsets(word: &OcrWord, split_points: &[usize]) -> Vec<OcrWord> {
    let chars: Vec<char> = word.text.chars().collect();
    let total_chars = chars.len() as f64;
    let char_w = word.width / total_chars;

    let mut indices = vec![0];
    for &sp in split_points {
        if sp > 0 && sp < chars.len() && !indices.contains(&sp) {
            indices.push(sp);
        }
    }
    indices.sort_unstable();
    indices.push(chars.len());

    let mut result = Vec::new();
    for w in indices.windows(2) {
        let start = w[0];
        let end = w[1];
        let sub_text: String = chars[start..end].iter().collect();
        let sub_len = (end - start) as f64;
        let sub_x = word.x + char_w * start as f64;
        let sub_w = char_w * sub_len;
        result.push(OcrWord {
            text: sub_text,
            x: sub_x,
            y: word.y,
            width: sub_w,
            height: word.height,
            confidence: word.confidence,
        });
    }

    result
}

/// Coalesces raw OCR lines (resolving line-split artifacts), preserves code indentation,
/// reconstructs token spacing, detects blank lines, and builds structured full text and bounding boxes.
pub fn coalesce_and_structure_lines(mut raw_lines: Vec<OcrLine>) -> (Vec<OcrLine>, String, Vec<BoundingBox>) {
    // 1. Normalize punctuation on all words and segment glued code tokens
    for line in &mut raw_lines {
        for word in &mut line.words {
            word.text = normalize_code_punctuation(&word.text);
        }
        line.words.retain(|w| !w.text.trim().is_empty());
        line.words = segment_code_tokens(line.words.drain(..).collect());
    }
    raw_lines.retain(|l| !l.words.is_empty());

    if raw_lines.is_empty() {
        return (Vec::new(), String::new(), Vec::new());
    }

    // 2. Coalesce lines that are horizontally adjacent fragments of the same visual line
    // (resolving OCR horizontal split artifacts without falsely merging tightly-spaced consecutive lines or columns)
    let mut initial_total_w = 0.0;
    let mut initial_total_chars = 0;
    for line in &raw_lines {
        for word in &line.words {
            let count = word.text.chars().count();
            if count >= 2 && word.width > 0.0 {
                initial_total_w += word.width;
                initial_total_chars += count;
            }
        }
    }
    if initial_total_chars == 0 {
        for line in &raw_lines {
            for word in &line.words {
                let count = word.text.chars().count();
                if count >= 1 && word.width > 0.0 {
                    initial_total_w += word.width;
                    initial_total_chars += count;
                }
            }
        }
    }
    let char_w = if initial_total_chars > 0 {
        (initial_total_w / initial_total_chars as f64).max(3.0)
    } else {
        8.0
    };

    let mut lines: Vec<OcrLine> = raw_lines;
    loop {
        let mut merged_any = false;
        let mut i = 0;
        while i < lines.len() {
            let mut j = i + 1;
            while j < lines.len() {
                let min_h = lines[i].height.min(lines[j].height);
                let max_h = lines[i].height.max(lines[j].height);
                if min_h <= 0.0 {
                    j += 1;
                    continue;
                }

                let top = lines[i].y.max(lines[j].y);
                let bottom = (lines[i].y + lines[i].height).min(lines[j].y + lines[j].height);
                let v_overlap = (bottom - top).max(0.0);

                let center_1 = lines[i].y + lines[i].height * 0.5;
                let center_2 = lines[j].y + lines[j].height * 0.5;
                let center_diff = (center_1 - center_2).abs();

                // Optimization: prune non-overlapping lines immediately to prevent O(N^2) scaling slowdown
                if (v_overlap / min_h) < 0.50 || center_diff > (max_h * 0.35).max(min_h * 0.35) {
                    j += 1;
                    continue;
                }

                // Line fragment bounding box gap
                let line_a_right = lines[i].x + lines[i].width;
                let line_b_right = lines[j].x + lines[j].width;
                let line_gap = if lines[j].x >= line_a_right {
                    lines[j].x - line_a_right
                } else if lines[i].x >= line_b_right {
                    lines[i].x - line_b_right
                } else {
                    0.0
                };

                let mut word_gap = f64::INFINITY;
                for wa in &lines[i].words {
                    for wb in &lines[j].words {
                        let gap = if wb.x >= wa.x + wa.width {
                            wb.x - (wa.x + wa.width)
                        } else if wa.x >= wb.x + wb.width {
                            wa.x - (wb.x + wb.width)
                        } else {
                            0.0
                        };
                        if gap < word_gap {
                            word_gap = gap;
                        }
                    }
                }

                let horizontal_gap = line_gap.min(word_gap);
                // Cap max_allowed_gap at 35.0px for substantial multi-word column blocks so headlines/text
                // never merge across column gutters (>= 40px), while intra-line tokens/operators coalesce properly.
                let is_multi_word_column = lines[i].words.len() >= 2
                    && lines[j].words.len() >= 2
                    && lines[i].width >= 70.0
                    && lines[j].width >= 70.0;
                let max_allowed_gap = if is_multi_word_column {
                    (min_h * 1.5).max(char_w * 2.5).min(35.0)
                } else {
                    (min_h * 2.5).max(char_w * 4.5).min(75.0)
                };

                let is_same_line = horizontal_gap <= max_allowed_gap + 1e-4
                    && !words_collide_horizontally(&lines[i].words, &lines[j].words);

                if is_same_line {
                    let mut other_words = lines.remove(j).words;
                    lines[i].words.append(&mut other_words);
                    lines[i].words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
                    let (nx, ny, nw, nh) = compute_words_bounding_box(&lines[i].words);
                    lines[i].x = nx;
                    lines[i].y = ny;
                    lines[i].width = nw;
                    lines[i].height = nh;
                    merged_any = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        if !merged_any {
            break;
        }
    }

    for line in &mut lines {
        line.words.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Filter out isolated single-symbol ghost lines that collide horizontally with another line in the same vertical span
    const NOISE_CHARS: &[char] = &['|', '—', '_', '~', '^', '•', '·', '«', '»', '¢', '§', '°', '-', '='];
    let original_lines = lines.clone();
    lines.retain(|candidate| {
        if candidate.words.len() == 1 {
            let t = candidate.words[0].text.trim();
            if t.chars().count() == 1 && NOISE_CHARS.contains(&t.chars().next().unwrap()) {
                for other in &original_lines {
                    if candidate.y != other.y || candidate.x != other.x {
                        if words_collide_horizontally(&candidate.words, &other.words) {
                            let top = candidate.y.max(other.y);
                            let bottom = (candidate.y + candidate.height).min(other.y + other.height);
                            let v_overlap = (bottom - top).max(0.0);
                            let min_h = candidate.height.min(other.height);
                            if min_h > 0.0 && (v_overlap / min_h) >= 0.50 {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    });

    // 3. Sort lines top-to-bottom by y coordinate, breaking ties or near-ties by x coordinate (left-to-right)
    lines.sort_by(|a, b| {
        let diff_y = a.y - b.y;
        if diff_y.abs() > 4.0 {
            a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // 4. Estimate character width and median line height across code block
    let mut total_w = 0.0;
    let mut total_chars = 0;
    for line in &lines {
        for word in &line.words {
            let count = word.text.chars().count();
            if count >= 2 {
                total_w += word.width;
                total_chars += count;
            }
        }
    }
    if total_chars == 0 {
        for line in &lines {
            for word in &line.words {
                let count = word.text.chars().count();
                if count >= 1 && word.width > 0.0 {
                    total_w += word.width;
                    total_chars += count;
                }
            }
        }
    }
    let char_w = if total_chars > 0 {
        (total_w / total_chars as f64).max(3.0)
    } else {
        8.0
    };

    let mut heights: Vec<f64> = lines.iter().map(|l| l.height).filter(|&h| h > 0.0).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_line_h = if !heights.is_empty() {
        heights[heights.len() / 2].max(8.0)
    } else {
        18.0
    };

    // Line pitch estimate (vertical distance between consecutive lines)
    let mut pitches: Vec<f64> = Vec::new();
    for i in 1..lines.len() {
        let diff = lines[i].y - lines[i - 1].y;
        if diff > median_line_h * 0.8 && diff < median_line_h * 2.5 {
            pitches.push(diff);
        }
    }
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_pitch = if !pitches.is_empty() {
        pitches[pitches.len() / 2]
    } else {
        median_line_h * 1.3
    };

    // 5. Compute base left margin of the block
    let base_left_x = lines
        .iter()
        .filter_map(|l| l.words.first().map(|w| w.x))
        .fold(f64::INFINITY, f64::min);
    let base_left_x = if base_left_x.is_finite() { base_left_x } else { 0.0 };

    // Collect non-zero indentation offsets across lines
    let mut non_zero_indents: Vec<f64> = lines
        .iter()
        .filter_map(|l| l.words.first().map(|w| w.x - base_left_x))
        .filter(|&px| px >= char_w * 0.55)
        .collect();
    non_zero_indents.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Detect indentation model:
    // If we have indented lines, inspect the minimum indent offset to identify the base tab step
    let (unit_px, unit_spaces) = if let Some(&min_px) = non_zero_indents.first() {
        let est_chars = min_px / char_w;
        if est_chars >= 1.4 && est_chars <= 2.8 {
            (min_px, 2usize)
        } else if est_chars >= 3.2 && est_chars <= 5.0 {
            (min_px, 4usize)
        } else if est_chars > 5.0 && (est_chars / 4.0 - (est_chars / 4.0).round()).abs() <= 0.25 {
            let mult = (est_chars / 4.0).round().max(1.0);
            (min_px / mult, 4usize)
        } else if est_chars > 3.0 && (est_chars / 2.0 - (est_chars / 2.0).round()).abs() <= 0.25 {
            let mult = (est_chars / 2.0).round().max(1.0);
            (min_px / mult, 2usize)
        } else {
            (char_w, 1usize)
        }
    } else {
        (char_w, 1usize)
    };

    // 6. Format each line with leading indentation and token spacing
    for line in &mut lines {
        let first_word = &line.words[0];
        let indent_px = (first_word.x - base_left_x).max(0.0);
        let indent_spaces = if indent_px >= char_w * 0.55 {
            if unit_spaces > 1 && unit_px > 0.0 {
                let est_units = indent_px / unit_px;
                let nearest = est_units.round();
                if (est_units - nearest).abs() <= 0.28 {
                    (nearest.max(1.0) as usize) * unit_spaces
                } else {
                    (indent_px / char_w).round() as usize
                }
            } else {
                (indent_px / char_w).round() as usize
            }
        } else {
            0
        };

        let mut line_str = String::new();
        if indent_spaces > 0 {
            line_str.push_str(&" ".repeat(indent_spaces));
        }

        for (i, word) in line.words.iter().enumerate() {
            if i == 0 {
                line_str.push_str(&word.text);
            } else {
                let prev = &line.words[i - 1];
                let prev_end = prev.x + prev.width;
                let gap = (word.x - prev_end).max(0.0);

                if gap < char_w * 0.35 {
                    line_str.push_str(&word.text);
                } else if gap >= char_w * 1.75 {
                    let spaces = ((gap / char_w).round() as usize).max(2).min(8);
                    line_str.push_str(&" ".repeat(spaces));
                    line_str.push_str(&word.text);
                } else {
                    line_str.push(' ');
                    line_str.push_str(&word.text);
                }
            }
        }
        line.text = line_str;
    }

    // 7. Assemble full text preserving blank lines between blocks
    let mut full_text_lines = Vec::new();
    for i in 0..lines.len() {
        if i > 0 {
            let prev_y = lines[i - 1].y;
            let curr_y = lines[i].y;
            let y_diff = curr_y - prev_y;
            if y_diff >= median_pitch * 1.6 {
                let extra_lines = ((y_diff / median_pitch).round() as usize).saturating_sub(1).min(3);
                for _ in 0..extra_lines {
                    full_text_lines.push(String::new());
                }
            }
        }
        full_text_lines.push(lines[i].text.clone());
    }
    let full_text = full_text_lines.join("\n");

    // 8. Bounding boxes matching lines
    let boxes = lines
        .iter()
        .map(|l| BoundingBox {
            text: l.text.clone(),
            x: l.x,
            y: l.y,
            width: l.width,
            height: l.height,
            translated_text: None,
        })
        .collect();

    (lines, full_text, boxes)
}

/// Concatenates the lines belonging to a paragraph with spaces, properly joining words
/// across line breaks and dehyphenating words broken by end-of-line hyphens.
pub fn join_paragraph_lines(lines: &[&str]) -> String {
    let mut result = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if result.is_empty() {
            result.push_str(trimmed);
        } else {
            // Check if result ends with hyphen preceded by alphabetic char (word-break dehyphenation)
            let ends_with_hyphen = result.ends_with('-')
                && result[..result.len() - 1]
                    .chars()
                    .last()
                    .map(|c| c.is_alphabetic())
                    .unwrap_or(false)
                && trimmed.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false);

            if ends_with_hyphen {
                result.pop(); // remove trailing '-'
                result.push_str(trimmed);
            } else {
                result.push(' ');
                result.push_str(trimmed);
            }
        }
    }
    result
}

/// Evaluates whether two vertically adjacent lines belong to the same text paragraph.
pub fn should_merge_lines_into_paragraph(
    curr_text: &str,
    curr_x: f64,
    curr_y: f64,
    curr_w: f64,
    curr_h: f64,
    next_text: &str,
    next_x: f64,
    next_y: f64,
    next_w: f64,
    next_h: f64,
    median_line_h: f64,
    median_pitch: f64,
    max_column_w: f64,
) -> bool {
    let t_curr = curr_text.trim();
    let t_next = next_text.trim();

    if t_curr.is_empty() || t_next.is_empty() {
        return false;
    }

    // 1. Valid dimensions
    if curr_w <= 0.0 || curr_h <= 0.0 || next_w <= 0.0 || next_h <= 0.0 {
        return false;
    }

    // 2. Line height similarity: lines in the same paragraph share consistent font height
    let min_h = curr_h.min(next_h);
    let max_h = curr_h.max(next_h);
    if min_h / max_h < 0.50 {
        return false;
    }

    // 3. Vertical pitch & gap
    let gap_y = next_y - (curr_y + curr_h);
    let pitch_y = next_y - curr_y;
    // Disallow non-downward lines or lines with excessive vertical gap (e.g. blank line)
    if pitch_y <= 0.0 || pitch_y > median_pitch * 1.65 || gap_y > median_line_h * 1.15 {
        return false;
    }

    // 4. Horizontal column overlap: lines must be in the same column
    let left = curr_x.max(next_x);
    let right = (curr_x + curr_w).min(next_x + next_w);
    let h_overlap = (right - left).max(0.0);
    let min_w = curr_w.min(next_w);
    if min_w <= 0.0 || (h_overlap / min_w) < 0.35 {
        return false;
    }

    // Disallow continuous left-edge drift (e.g. staircase lines from adversarial tests)
    let left_diff = (curr_x - next_x).abs();
    let is_first_line_indent = curr_x > next_x && (curr_x - next_x) <= 40.0;
    if left_diff > 16.0 && !is_first_line_indent {
        return false;
    }

    // 5. Markup heuristics: HTML/XML tags are UI elements, not prose lines
    let has_markup = (t_curr.contains('<') && t_curr.contains('>'))
        || t_curr.ends_with('>')
        || (t_next.contains('<') && t_next.contains('>'))
        || t_next.starts_with('<');
    if has_markup {
        return false;
    }

    // 6. Code heuristics: if either line looks like code syntax, do not merge
    const CODE_SUFFIXES: &[char] = &['{', '}', ';'];
    if CODE_SUFFIXES.iter().any(|&c| t_curr.ends_with(c)) {
        return false;
    }
    const CODE_PREFIXES: &[&str] = &[
        "pub fn ", "fn ", "let ", "const ", "import ", "export ", "class ", "def ", "//", "/*", "*/",
    ];
    if CODE_PREFIXES.iter().any(|&p| t_curr.starts_with(p) || t_next.starts_with(p)) {
        return false;
    }

    // 7. Next line starts with bullet / list item / numbered item -> starts new item
    let starts_with_bullet = t_next.starts_with('•')
        || t_next.starts_with('*')
        || t_next.starts_with('-')
        || (t_next.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
            && t_next.chars().nth(1).map(|c| c == '.' || c == ')').unwrap_or(false));
    if starts_with_bullet {
        return false;
    }

    // 8. Numbered / list line ending check: lines ending in a digit are labels or list items
    if t_curr.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return false;
    }

    // 9. Title-case check: UI labels, button text, and headings have mostly capitalized words
    let words_curr: Vec<&str> = t_curr.split_whitespace().collect();
    let upper_count = words_curr
        .iter()
        .filter(|w| w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false))
        .count();
    if words_curr.len() >= 2 && (upper_count as f64 / words_curr.len() as f64) >= 0.70 {
        return false;
    }

    // 10. Check for line continuation:
    // A. Hyphenated word break: e.g. "desenvolvi-" + "mento"
    let ends_with_hyphen = t_curr.ends_with('-')
        && t_curr[..t_curr.len() - 1].chars().last().map(|c| c.is_alphabetic()).unwrap_or(false);
    if ends_with_hyphen {
        return true;
    }

    // B. Terminal punctuation: '!', '?', ':', ';' always terminate a phrase.
    // Strip trailing whitespace, closing quotes ('"', '\'', '”', '’'), closing brackets (')', ']'),
    // and trailing emojis/symbols before evaluating terminal punctuation.
    let is_trailing_decorator = |c: char| {
        c.is_whitespace()
            || c == '"'
            || c == '\''
            || c == ')'
            || c == ']'
            || c == '}'
            || (!c.is_alphanumeric() && !c.is_ascii_punctuation())
    };
    let t_clean = t_curr.trim_end_matches(is_trailing_decorator);

    if t_clean.is_empty() {
        return false;
    }

    const HARD_TERMINAL_PUNCT: &[char] = &['!', '?', ':', ';'];
    if HARD_TERMINAL_PUNCT.iter().any(|&c| t_clean.ends_with(c)) {
        return false;
    }

    if t_clean.ends_with('.') {
        // Allow continuation across '.' if the line has multiple words and fills the column
        if words_curr.len() >= 4 && max_column_w > 0.0 && curr_w >= max_column_w * 0.70 {
            return true;
        }
        return false;
    }

    // C. Non-terminal ending in natural prose: genuine wrapped line continuation!
    true
}

/// Groups lines into cohesive paragraph blocks using vertical pitch consistency,
/// horizontal column overlap, and natural textual continuation.
pub fn group_lines_into_paragraphs(lines: &[OcrLine]) -> Vec<ParagraphBox> {
    if lines.is_empty() {
        return Vec::new();
    }

    // Estimate median line height
    let mut heights: Vec<f64> = lines.iter().map(|l| l.height).filter(|&h| h > 0.0).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_line_h = if !heights.is_empty() {
        heights[heights.len() / 2].max(8.0)
    } else {
        18.0
    };

    // Line pitch estimate
    let mut pitches: Vec<f64> = Vec::new();
    for i in 1..lines.len() {
        let diff = lines[i].y - lines[i - 1].y;
        if diff > median_line_h * 0.8 && diff < median_line_h * 2.5 {
            pitches.push(diff);
        }
    }
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_pitch = if !pitches.is_empty() {
        pitches[pitches.len() / 2]
    } else {
        median_line_h * 1.3
    };

    let max_column_w = lines.iter().map(|l| l.width).fold(0.0f64, f64::max);

    let mut paragraphs: Vec<ParagraphBox> = Vec::new();
    let mut current_indices: Vec<usize> = vec![0];

    for j in 1..lines.len() {
        let last_idx = *current_indices.last().unwrap();
        let curr = &lines[last_idx];
        let next = &lines[j];

        if should_merge_lines_into_paragraph(
            &curr.text,
            curr.x,
            curr.y,
            curr.width,
            curr.height,
            &next.text,
            next.x,
            next.y,
            next.width,
            next.height,
            median_line_h,
            median_pitch,
            max_column_w,
        ) {
            current_indices.push(j);
        } else {
            paragraphs.push(build_paragraph_from_line_indices(lines, &current_indices));
            current_indices = vec![j];
        }
    }

    if !current_indices.is_empty() {
        paragraphs.push(build_paragraph_from_line_indices(lines, &current_indices));
    }

    paragraphs
}

fn build_paragraph_from_line_indices(lines: &[OcrLine], indices: &[usize]) -> ParagraphBox {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut line_texts = Vec::with_capacity(indices.len());

    for &idx in indices {
        let l = &lines[idx];
        min_x = min_x.min(l.x);
        min_y = min_y.min(l.y);
        max_x = max_x.max(l.x + l.width);
        max_y = max_y.max(l.y + l.height);
        line_texts.push(l.text.as_str());
    }

    let min_x = if min_x.is_finite() { min_x } else { 0.0 };
    let min_y = if min_y.is_finite() { min_y } else { 0.0 };
    let width = (max_x - min_x).max(0.0);
    let height = (max_y - min_y).max(0.0);
    let text = if indices.len() == 1 {
        lines[indices[0]].text.clone()
    } else {
        join_paragraph_lines(&line_texts)
    };
    let is_para = indices.len() > 1;

    ParagraphBox {
        text,
        x: min_x,
        y: min_y,
        width,
        height,
        line_indices: indices.to_vec(),
        translated_text: None,
        is_paragraph: Some(is_para),
    }
}

/// Groups bounding boxes into cohesive paragraph blocks.
pub fn group_bounding_boxes_into_paragraphs(boxes: &[BoundingBox]) -> Vec<ParagraphBox> {
    if boxes.is_empty() {
        return Vec::new();
    }

    let mut heights: Vec<f64> = boxes.iter().map(|b| b.height).filter(|&h| h > 0.0).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_line_h = if !heights.is_empty() {
        heights[heights.len() / 2].max(8.0)
    } else {
        18.0
    };

    let mut pitches: Vec<f64> = Vec::new();
    for i in 1..boxes.len() {
        let diff = boxes[i].y - boxes[i - 1].y;
        if diff > median_line_h * 0.8 && diff < median_line_h * 2.5 {
            pitches.push(diff);
        }
    }
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_pitch = if !pitches.is_empty() {
        pitches[pitches.len() / 2]
    } else {
        median_line_h * 1.3
    };

    let max_column_w = boxes.iter().map(|b| b.width).fold(0.0f64, f64::max);

    let mut paragraphs: Vec<ParagraphBox> = Vec::new();
    let mut current_indices: Vec<usize> = vec![0];

    for j in 1..boxes.len() {
        let last_idx = *current_indices.last().unwrap();
        let curr = &boxes[last_idx];
        let next = &boxes[j];

        if should_merge_lines_into_paragraph(
            &curr.text,
            curr.x,
            curr.y,
            curr.width,
            curr.height,
            &next.text,
            next.x,
            next.y,
            next.width,
            next.height,
            median_line_h,
            median_pitch,
            max_column_w,
        ) {
            current_indices.push(j);
        } else {
            paragraphs.push(build_paragraph_from_box_indices(boxes, &current_indices));
            current_indices = vec![j];
        }
    }

    if !current_indices.is_empty() {
        paragraphs.push(build_paragraph_from_box_indices(boxes, &current_indices));
    }

    paragraphs
}

fn build_paragraph_from_box_indices(boxes: &[BoundingBox], indices: &[usize]) -> ParagraphBox {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut line_texts = Vec::with_capacity(indices.len());

    for &idx in indices {
        let b = &boxes[idx];
        min_x = min_x.min(b.x);
        min_y = min_y.min(b.y);
        max_x = max_x.max(b.x + b.width);
        max_y = max_y.max(b.y + b.height);
        line_texts.push(b.text.as_str());
    }

    let min_x = if min_x.is_finite() { min_x } else { 0.0 };
    let min_y = if min_y.is_finite() { min_y } else { 0.0 };
    let width = (max_x - min_x).max(0.0);
    let height = (max_y - min_y).max(0.0);
    let text = if indices.len() == 1 {
        boxes[indices[0]].text.clone()
    } else {
        join_paragraph_lines(&line_texts)
    };
    let is_para = indices.len() > 1;

    ParagraphBox {
        text,
        x: min_x,
        y: min_y,
        width,
        height,
        line_indices: indices.to_vec(),
        translated_text: None,
        is_paragraph: Some(is_para),
    }
}

/// Parses Tesseract TSV output into a strongly typed OcrResult with line and word bounding boxes.
pub fn parse_tesseract_tsv(tsv_text: &str) -> OcrResult {
    parse_tesseract_tsv_scaled(tsv_text, 1.0)
}

/// Parses Tesseract TSV output with coordinate scaling to map upscaled OCR back to screen pixels.
pub fn parse_tesseract_tsv_scaled(tsv_text: &str, inv_scale: f64) -> OcrResult {
    use std::collections::BTreeMap;

    let scale = if inv_scale > 0.0 { inv_scale } else { 1.0 };

    // Track line boxes (level 4) and line words (level 5)
    let mut line_boxes: BTreeMap<(i32, i32, i32), (f64, f64, f64, f64)> = BTreeMap::new();
    let mut line_words: BTreeMap<(i32, i32, i32), Vec<OcrWord>> = BTreeMap::new();
    let mut total_words = 0;

    for (idx, line) in tsv_text.lines().enumerate() {
        if idx == 0 {
            // Header line
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 12 {
            continue;
        }

        let level: i32 = parts[0].trim().parse().unwrap_or(0);
        let block_num: i32 = parts[2].trim().parse().unwrap_or(0);
        let par_num: i32 = parts[3].trim().parse().unwrap_or(0);
        let line_num: i32 = parts[4].trim().parse().unwrap_or(0);
        let left: f64 = parts[6].trim().parse().unwrap_or(0.0) * scale;
        let top: f64 = parts[7].trim().parse().unwrap_or(0.0) * scale;
        let width: f64 = parts[8].trim().parse().unwrap_or(0.0) * scale;
        let height: f64 = parts[9].trim().parse().unwrap_or(0.0) * scale;

        let key = (block_num, par_num, line_num);

        if level == 4 {
            line_boxes.insert(key, (left, top, width, height));
        } else if level == 5 {
            let conf: f32 = parts[10].trim().parse().unwrap_or(0.0);
            let word_text = parts[11].trim().to_string();

            // 1. Discard empty / whitespace words
            if word_text.is_empty() {
                continue;
            }

            let raw_w: f64 = parts[8].trim().parse().unwrap_or(0.0);
            let raw_h: f64 = parts[9].trim().parse().unwrap_or(0.0);

            // 2. Sanity check: discard boxes where width < 3.0 or height < 3.0
            if raw_w < 3.0 || raw_h < 3.0 || width < 1.5 || height < 1.5 {
                continue;
            }

            let char_count = word_text.chars().count();

            // 3. Filter invalid / NaN and low-confidence hallucinations: require finite conf and conf >= 35.0 (and conf >= 45.0 for words of length <= 2)
            if !conf.is_finite() || conf.round() < 35.0 || (conf.round() < 45.0 && char_count <= 2) {
                continue;
            }

            // 4. Filter vertical margin artifacts: discard tall narrow boxes from paper folds / margins.
            // Do NOT drop single narrow characters like 'I', '1', 'l', 'i', '!', '[', ']', '|', '/', '\\'
            // or programming operators like '==', '!=', '<=', '>=', '%', '=', '+', '-', '*', etc.,
            // or programming keywords like 'for', 'if', 'else:', 'try:', 'def', 'in', etc.
            let is_code_operator = is_code_op(&word_text);
            let is_keyword = is_code_keyword(&word_text);
            let is_single_narrow_char = if char_count == 1 {
                let ch = word_text.chars().next().unwrap_or(' ');
                ch.is_alphanumeric()
                    || matches!(
                        ch,
                        '!' | '[' | ']' | '|' | '/' | '\\' | '(' | ')' | '{' | '}'
                            | ':' | ';' | '?' | '%' | '=' | '+' | '-' | '*' | '<' | '>'
                            | '&' | '^' | '~' | '\'' | '"' | ',' | '.' | '_' | '@' | '$' | '#'
                    )
            } else {
                false
            };

            let is_excessive_tall_sliver = height > 40.0 && width < 15.0;
            let should_filter_aspect = if is_code_operator || (is_keyword && height <= 40.0) {
                false
            } else if is_excessive_tall_sliver {
                true
            } else if char_count >= 2 && char_count <= 3 {
                true
            } else if !is_single_narrow_char {
                true
            } else {
                false
            };

            if height > 2.8 * width && should_filter_aspect {
                continue;
            }

            // 5. Filter single noise symbols: discard isolated symbols when conf < 75.0
            const NOISE_SYMBOLS: &[char] = &['|', '—', '_', '~', '^', '•', '·', '«', '»', '¢', '§', '°', '-', '='];
            if char_count == 1 && NOISE_SYMBOLS.contains(&word_text.chars().next().unwrap()) && conf < 75.0 {
                continue;
            }

            let word = OcrWord {
                text: word_text,
                x: left,
                y: top,
                width,
                height,
                confidence: conf,
            };

            line_words.entry(key).or_default().push(word);
            total_words += 1;
        }
    }

    let mut raw_lines = Vec::new();

    for (key, words) in line_words {
        let (lx, ly, lw, lh) = if let Some(&(x, y, w, h)) = line_boxes.get(&key) {
            if w > 0.0 && h > 0.0 {
                (x, y, w, h)
            } else {
                compute_words_bounding_box(&words)
            }
        } else {
            compute_words_bounding_box(&words)
        };

        let raw_line_text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        raw_lines.push(OcrLine {
            text: raw_line_text,
            x: lx,
            y: ly,
            width: lw,
            height: lh,
            words,
        });
    }

    let (lines, full_text, boxes) = coalesce_and_structure_lines(raw_lines);
    let paragraphs = group_lines_into_paragraphs(&lines);

    OcrResult {
        full_text,
        lines,
        word_count: total_words,
        boxes,
        paragraphs,
    }
}


#[cfg(target_os = "windows")]
pub fn extract_text_native(image: &image::RgbaImage) -> Result<OcrResult, String> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;

    // 1. High-precision preprocessing: dark theme inversion + Lanczos scaling
    let (prep_image, scale) = preprocess_for_ocr(image);

    let width = prep_image.width() as i32;
    let height = prep_image.height() as i32;

    let software_bitmap = SoftwareBitmap::Create(BitmapPixelFormat::Rgba8, width, height)
        .map_err(|e| format!("Failed to create SoftwareBitmap: {e}"))?;

    let raw_bytes = prep_image.as_raw();
    let data_writer = DataWriter::new()
        .map_err(|e| format!("Failed to create DataWriter: {e}"))?;
    data_writer
        .WriteBytes(raw_bytes)
        .map_err(|e| format!("Failed to write pixels: {e}"))?;
    let buffer = data_writer
        .DetachBuffer()
        .map_err(|e| format!("Failed to detach buffer: {e}"))?;

    software_bitmap
        .CopyFromBuffer(&buffer)
        .map_err(|e| format!("Failed to copy pixels to SoftwareBitmap: {e}"))?;

    use windows::Globalization::Language;
    let engine = if let Ok(en_lang) = Language::CreateLanguage(&windows::core::HSTRING::from("en-US")) {
        if OcrEngine::IsLanguageSupported(&en_lang).unwrap_or(false) {
            OcrEngine::TryCreateFromLanguage(&en_lang)
                .or_else(|_| OcrEngine::TryCreateFromUserProfileLanguages())
                .map_err(|e| format!("Failed to create OCR engine: {e}"))?
        } else {
            OcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|e| format!("Failed to initialize Windows OCR Engine: {e}"))?
        }
    } else {
        OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|e| format!("Failed to initialize Windows OCR Engine: {e}"))?
    };

    let ocr_result = engine
        .RecognizeAsync(&software_bitmap)
        .map_err(|e| format!("Failed to start OCR recognition: {e}"))?
        .get()
        .map_err(|e| format!("OCR recognition execution failed: {e}"))?;

    let inv_scale = 1.0 / scale;
    let mut raw_lines = Vec::new();

    if let Ok(ocr_lines) = ocr_result.Lines() {
        for line in ocr_lines {
            let mut words = Vec::new();

            if let Ok(ocr_words) = line.Words() {
                for word in ocr_words {
                    let word_text = word.Text().map(|t| t.to_string()).unwrap_or_default();
                    let rect = word.BoundingRect().unwrap_or_default();
                    words.push(OcrWord {
                        text: word_text,
                        x: (rect.X as f64) * inv_scale,
                        y: (rect.Y as f64) * inv_scale,
                        width: (rect.Width as f64) * inv_scale,
                        height: (rect.Height as f64) * inv_scale,
                        confidence: 100.0,
                    });
                }
            }

            if !words.is_empty() {
                let (lx, ly, lw, lh) = compute_words_bounding_box(&words);
                let line_text = line.Text().map(|t| t.to_string()).unwrap_or_default();
                raw_lines.push(OcrLine {
                    text: line_text,
                    x: lx,
                    y: ly,
                    width: lw,
                    height: lh,
                    words,
                });
            }
        }
    }

    for (idx, rl) in raw_lines.iter().enumerate() {
        eprintln!("RAW LINE {}: y={} h={} words={:?}", idx, rl.y, rl.height, rl.words.iter().map(|w| (&w.text, w.x, w.y)).collect::<Vec<_>>());
    }

    let (lines, full_text, boxes) = coalesce_and_structure_lines(raw_lines);
    let paragraphs = group_lines_into_paragraphs(&lines);
    let total_words = lines.iter().map(|l| l.words.len()).sum();

    Ok(OcrResult {
        full_text,
        lines,
        word_count: total_words,
        boxes,
        paragraphs,
    })
}

#[cfg(target_os = "linux")]
pub fn extract_text_native(image: &image::RgbaImage) -> Result<OcrResult, String> {
    use std::process::Command;

    // Use unified OCR preprocessor (handles dark mode inversion + 2x scaling)
    let (temp_image, scale) = preprocess_for_ocr(image);

    // Save temporary PNG for tesseract
    let temp_dir = std::env::temp_dir();
    let temp_image_path = temp_dir.join(format!(
        "desklens_ocr_{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));

    temp_image
        .save_with_format(&temp_image_path, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to write temporary image for OCR: {e}"))?;

    let temp_path_str = temp_image_path.to_string_lossy().to_string();

    // Check tesseract with multi-language (por+eng), --dpi 300, --psm 11, or fallback to default
    let output = match Command::new("tesseract")
        .args([
            &temp_path_str,
            "stdout",
            "--dpi",
            "300",
            "--psm",
            "11",
            "-l",
            "por+eng",
            "tsv",
        ])
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => Command::new("tesseract")
            .args([
                &temp_path_str,
                "stdout",
                "--dpi",
                "300",
                "--psm",
                "11",
                "tsv",
            ])
            .output()
            .map_err(|e| {
                let _ = std::fs::remove_file(&temp_image_path);
                if e.kind() == std::io::ErrorKind::NotFound {
                    "Tesseract OCR is not installed. To enable OCR on Linux, install it via: sudo apt install tesseract-ocr tesseract-ocr-por tesseract-ocr-eng".to_string()
                } else {
                    format!("Failed to execute Tesseract: {e}")
                }
            })?,
    };

    let _ = std::fs::remove_file(&temp_image_path);

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Tesseract OCR failed: {err_msg}"));
    }

    let tsv_text = String::from_utf8_lossy(&output.stdout);
    let inv_scale = if (scale - 1.0).abs() > 0.001 {
        1.0 / scale
    } else {
        1.0
    };
    let result = parse_tesseract_tsv_scaled(&tsv_text, inv_scale);

    Ok(result)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn extract_text_native(_image: &image::RgbaImage) -> Result<OcrResult, String> {
    Err("OCR engine is not configured for this operating system.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_words_bounding_box() {
        let empty: Vec<OcrWord> = vec![];
        assert_eq!(compute_words_bounding_box(&empty), (0.0, 0.0, 0.0, 0.0));

        let words = vec![
            OcrWord {
                text: "Hello".to_string(),
                x: 10.0,
                y: 20.0,
                width: 50.0,
                height: 15.0,
                confidence: 100.0,
            },
            OcrWord {
                text: "World".to_string(),
                x: 70.0,
                y: 18.0,
                width: 60.0,
                height: 19.0,
                confidence: 100.0,
            },
        ];
        let (bx, by, bw, bh) = compute_words_bounding_box(&words);
        assert_eq!(bx, 10.0);
        assert_eq!(by, 18.0);
        assert_eq!(bw, 120.0); // 70 + 60 - 10 = 120
        assert_eq!(bh, 19.0);  // max(20+15=35, 18+19=37) - 18 = 19
    }

    #[test]
    fn test_parse_tesseract_tsv_levels() {
        let mock_tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t800\t600\t-1\t\n\
2\t1\t1\t0\t0\t0\t20\t30\t300\t100\t-1\t\n\
3\t1\t1\t1\t0\t0\t20\t30\t300\t100\t-1\t\n\
4\t1\t1\t1\t1\t0\t20\t30\t200\t25\t-1\t\n\
5\t1\t1\t1\t1\t1\t20\t30\t60\t25\t95\tDeskLens\n\
5\t1\t1\t1\t1\t2\t90\t30\t70\t25\t96\tScreen\n\
4\t1\t1\t1\t2\t0\t20\t65\t180\t25\t-1\t\n\
5\t1\t1\t1\t2\t1\t20\t65\t180\t25\t94\tTranslation\n";

        let result = parse_tesseract_tsv(mock_tsv);
        assert_eq!(result.word_count, 3);
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].text, "DeskLens Screen");
        assert_eq!(result.lines[0].x, 20.0);
        assert_eq!(result.lines[0].y, 30.0);
        assert_eq!(result.lines[0].width, 200.0);
        assert_eq!(result.lines[0].height, 25.0);
        assert_eq!(result.lines[0].words.len(), 2);

        assert_eq!(result.lines[1].text, "Translation");
        assert_eq!(result.lines[1].x, 20.0);
        assert_eq!(result.lines[1].y, 65.0);
        assert_eq!(result.lines[1].width, 180.0);
        assert_eq!(result.lines[1].height, 25.0);
        assert_eq!(result.lines[1].words.len(), 1);

        assert_eq!(result.boxes.len(), 2);
        assert_eq!(result.boxes[0].text, "DeskLens Screen");
        assert_eq!(result.boxes[0].x, 20.0);
        assert_eq!(result.boxes[0].y, 30.0);
        assert_eq!(result.boxes[0].width, 200.0);
        assert_eq!(result.boxes[0].height, 25.0);
        assert!(result.boxes[0].translated_text.is_none());

        assert_eq!(result.boxes[1].text, "Translation");
        assert_eq!(result.boxes[1].x, 20.0);
        assert_eq!(result.boxes[1].y, 65.0);
        assert_eq!(result.boxes[1].width, 180.0);
        assert_eq!(result.boxes[1].height, 25.0);
    }

    #[test]
    fn test_bounding_box_serde_contract() {
        let box_without_trans = BoundingBox {
            text: "Hello".to_string(),
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 15.0,
            translated_text: None,
        };
        let json = serde_json::to_string(&box_without_trans).unwrap();
        assert!(!json.contains("translated_text"), "translated_text should be omitted when None: {}", json);

        let box_with_trans = BoundingBox {
            text: "Hello".to_string(),
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 15.0,
            translated_text: Some("Olá".to_string()),
        };
        let json_trans = serde_json::to_string(&box_with_trans).unwrap();
        assert!(json_trans.contains("\"translated_text\":\"Olá\""), "JSON should contain translated_text: {}", json_trans);
    }

    #[test]
    fn test_join_paragraph_lines_dehyphenation() {
        // Hyphenated word break
        let lines = ["Durante o desenvolvi-", "mento de software ágil,"];
        let joined = join_paragraph_lines(&lines);
        assert_eq!(joined, "Durante o desenvolvimento de software ágil,");

        // Normal lines without hyphen
        let lines_normal = ["Primeira linha de texto", "segunda linha continua."];
        assert_eq!(join_paragraph_lines(&lines_normal), "Primeira linha de texto segunda linha continua.");

        // Dash with space (punctuation dash, not word hyphen)
        let lines_dash = ["Item A - ", "continuação do item"];
        assert_eq!(join_paragraph_lines(&lines_dash), "Item A - continuação do item");
    }

    #[test]
    fn test_group_lines_into_paragraphs_reproduction() {
        // Ground truth reproduction of Image 2 (user issue)
        let l1 = OcrLine {
            text: "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se".to_string(),
            x: 22.0,
            y: 14.0,
            width: 968.0,
            height: 18.0,
            words: vec![],
        };
        let l2 = OcrLine {
            text: "preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas".to_string(),
            x: 22.0,
            y: 40.0,
            width: 968.0,
            height: 18.0,
            words: vec![],
        };
        let l3 = OcrLine {
            text: "faz parte do processo de aprendizagem.".to_string(),
            x: 21.0,
            y: 65.0,
            width: 324.0,
            height: 18.0,
            words: vec![],
        };

        let paragraphs = group_lines_into_paragraphs(&[l1, l2, l3]);
        assert_eq!(paragraphs.len(), 1, "The 3 wrapped lines must form exactly 1 paragraph");
        let p = &paragraphs[0];
        assert_eq!(p.x, 21.0);
        assert_eq!(p.y, 14.0);
        assert_eq!(p.width, 969.0);
        assert_eq!(p.height, 69.0);
        assert_eq!(p.line_indices, vec![0, 1, 2]);
        assert_eq!(p.is_paragraph, Some(true));
        assert!(p.text.starts_with("Lembrando que"));
        assert!(p.text.contains("não se preocupe!"));
        assert!(p.text.contains("Fazer perguntas faz parte"));
        assert!(p.text.ends_with("processo de aprendizagem."));
    }

    #[test]
    fn test_group_bounding_boxes_separate_paragraphs() {
        // Two separate paragraphs separated by a vertical blank gap
        let b1 = BoundingBox {
            text: "Este é o primeiro parágrafo de teste.".to_string(),
            x: 10.0,
            y: 10.0,
            width: 300.0,
            height: 20.0,
            translated_text: None,
        };
        let b2 = BoundingBox {
            text: "Ele termina aqui com ponto final.".to_string(),
            x: 10.0,
            y: 35.0,
            width: 250.0,
            height: 20.0,
            translated_text: None,
        };
        let b3 = BoundingBox {
            text: "Este é o segundo parágrafo após um espaço.".to_string(),
            x: 10.0,
            y: 90.0, // Large vertical gap (pitch = 55px vs normal 25px)
            width: 310.0,
            height: 20.0,
            translated_text: None,
        };

        let paragraphs = group_bounding_boxes_into_paragraphs(&[b1, b2, b3]);
        assert_eq!(paragraphs.len(), 2, "Should separate into 2 paragraphs across vertical gap");
        assert_eq!(paragraphs[0].line_indices, vec![0, 1]);
        assert_eq!(paragraphs[1].line_indices, vec![2]);
    }
}
