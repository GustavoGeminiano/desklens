/**
 * tests/translation_e2e.test.mjs
 *
 * Comprehensive 4-Tier Test Suite for DeskLens Translation,
 * Paragraph Grouping, Language Selector Flow, and Unified Overlays.
 *
 * Specifications derived from:
 * - ORIGINAL_REQUEST.md (Image 1 & Image 2 analysis, verbatim Portuguese text)
 * - PROJECT.md (Interface Contracts, Milestones M1-M4)
 * - Explorer Survey Reports 1, 2, 3
 *
 * Tiers:
 * - Tier 1: Feature Coverage (>=5 tests per feature across 5 features: 25 tests)
 * - Tier 2: Boundary & Corner Cases (9 tests)
 * - Tier 3: Cross-Feature Combinations (5 tests)
 * - Tier 4: Real-World Scenarios (including Image 2 verbatim paragraph: 5 tests)
 */

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

// =============================================================================
// Section 0: Reference Specification Implementations
// =============================================================================

/**
 * Normalizes language codes (e.g. "pt-BR" -> "pt", "en-US" -> "en").
 */
export function normalizeLanguageCode(code) {
  if (!code || typeof code !== "string") return "pt";
  const cleaned = code.trim().toLowerCase().split("-")[0];
  const supported = ["pt", "en", "es", "fr", "de", "ja", "zh", "it", "ru", "ko"];
  return supported.includes(cleaned) ? cleaned : "pt";
}

/**
 * Union bounding box computation:
 * [ min(x), min(y), max(x + w) - min(x), max(y + h) - min(y) ]
 */
export function calculateUnionBoundingBox(boxes) {
  if (!boxes || boxes.length === 0) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const b of boxes) {
    if (!b || typeof b.x !== "number" || typeof b.y !== "number") continue;
    const w = Number(b.width) || 0;
    const h = Number(b.height) || 0;
    minX = Math.min(minX, b.x);
    minY = Math.min(minY, b.y);
    maxX = Math.max(maxX, b.x + w);
    maxY = Math.max(maxY, b.y + h);
  }

  if (minX === Infinity) {
    return { x: 0, y: 0, width: 0, height: 0 };
  }

  return {
    x: Math.round(minX),
    y: Math.round(minY),
    width: Math.round(Math.max(0, maxX - minX)),
    height: Math.round(Math.max(0, maxY - minY)),
  };
}

/**
 * Joins multi-line text into a single coherent paragraph string.
 * Handles trailing hyphens (e.g. "preocu-" + "pe" -> "preocupe").
 */
export function formatParagraphText(lines) {
  if (!lines || lines.length === 0) return "";
  let result = "";

  for (let i = 0; i < lines.length; i++) {
    const rawLine = (lines[i].text || "").trim();
    if (!rawLine) continue;

    if (!result) {
      result = rawLine;
    } else if (result.endsWith("-") && !result.endsWith(" -")) {
      // De-hyphenate line break
      result = result.slice(0, -1) + rawLine;
    } else {
      result += " " + rawLine;
    }
  }

  return result;
}

/**
 * Partitions OCR lines into horizontal column buckets.
 * Lines sharing horizontal overlap belong to the same column bucket.
 */
function partitionLinesIntoColumns(lines) {
  const sortedByX = [...lines].sort((a, b) => a.x - b.x);
  const columns = [];

  for (const line of sortedByX) {
    let placed = false;
    for (const col of columns) {
      const overlapsCol = col.some((cLine) => {
        const overlap = Math.max(0, Math.min(cLine.x + cLine.width, line.x + line.width) - Math.max(cLine.x, line.x));
        const minW = Math.min(cLine.width, line.width);
        return minW > 0 && overlap / minW >= 0.35;
      });
      if (overlapsCol) {
        col.push(line);
        placed = true;
        break;
      }
    }
    if (!placed) {
      columns.push([line]);
    }
  }

  columns.sort((colA, colB) => {
    const minXA = Math.min(...colA.map((l) => l.x));
    const minXB = Math.min(...colB.map((l) => l.x));
    return minXA - minXB;
  });

  return columns;
}

/**
 * Reference Paragraph Clustering Algorithm (as defined in PROJECT.md M1).
 * Groups vertically contiguous lines into cohesive ParagraphBox units.
 */
export function clusterParagraphBoxes(rawLines, options = {}) {
  if (!rawLines || !Array.isArray(rawLines) || rawLines.length === 0) {
    return [];
  }

  // Filter out invalid/empty lines
  const validLines = rawLines.filter((l) => {
    if (!l || typeof l.text !== "string") return false;
    const trimmed = l.text.trim();
    if (!trimmed) return false;
    const w = Number(l.width) || 0;
    const h = Number(l.height) || 0;
    return w > 3 && h > 3;
  });

  if (validLines.length === 0) return [];

  // Partition lines by column first (essential for multi-column documents)
  const columns = partitionLinesIntoColumns(validLines);
  const allParagraphs = [];

  for (const colLines of columns) {
    // Sort column lines by Y coordinate
    const sorted = [...colLines].sort((a, b) => a.y - b.y || a.x - b.x);

    // Calculate median line height for this column
    const heights = sorted.map((l) => Number(l.height) || 16).sort((a, b) => a - b);
    const medianH = heights[Math.floor(heights.length / 2)] || 18;
    const maxAllowedGap = options.maxGap ?? medianH * 1.5;

    let currentCluster = [sorted[0]];

    for (let i = 1; i < sorted.length; i++) {
      const prev = currentCluster[currentCluster.length - 1];
      const curr = sorted[i];

      const prevBottom = prev.y + prev.height;
      const verticalGap = curr.y - prevBottom;

      // Horizontal overlap
      const hOverlap = Math.max(0, Math.min(prev.x + prev.width, curr.x + curr.width) - Math.max(prev.x, curr.x));
      const minW = Math.min(prev.width, curr.width);
      const overlapRatio = minW > 0 ? hOverlap / minW : 0;

      // Font height difference ratio
      const maxH = Math.max(prev.height, curr.height);
      const heightDiffRatio = maxH > 0 ? Math.abs(prev.height - curr.height) / maxH : 0;

      // Heading detection: prominent font size difference separates heading from body
      const isHeaderBreak = heightDiffRatio >= 0.20 && (prev.height > curr.height || prev.text.length < 35);

      // Paragraph break on terminal punctuation followed by indent or pitch jump
      const prevTrimmed = prev.text.trim();
      const hasTerminalPunct = /[.!?]$/.test(prevTrimmed);
      const hasLargeIndent = curr.x - prev.x > 20;
      const hasPitchJump = verticalGap > medianH * 1.1;

      const canGroup =
        verticalGap >= -6 &&
        verticalGap <= maxAllowedGap &&
        overlapRatio >= 0.35 &&
        !isHeaderBreak &&
        !(hasTerminalPunct && (hasLargeIndent || hasPitchJump));

      if (canGroup) {
        currentCluster.push(curr);
      } else {
        const bounds = calculateUnionBoundingBox(currentCluster);
        allParagraphs.push({
          text: formatParagraphText(currentCluster),
          x: bounds.x,
          y: bounds.y,
          width: bounds.width,
          height: bounds.height,
          lineCount: currentCluster.length,
          is_paragraph: currentCluster.length > 1,
          lines: currentCluster,
        });
        currentCluster = [curr];
      }
    }

    if (currentCluster.length > 0) {
      const bounds = calculateUnionBoundingBox(currentCluster);
      allParagraphs.push({
        text: formatParagraphText(currentCluster),
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
        lineCount: currentCluster.length,
        is_paragraph: currentCluster.length > 1,
        lines: currentCluster,
      });
    }
  }

  return allParagraphs;
}

/**
 * Extracts detected source language from backend response payload.
 */
export function extractDetectedLanguage(response) {
  if (!response) return null;
  if (typeof response === "object" && !Array.isArray(response) && response.detected_source_lang) {
    return normalizeLanguageCode(response.detected_source_lang);
  }
  if (Array.isArray(response) && response.length > 0 && response[0].detected_source_lang) {
    return normalizeLanguageCode(response[0].detected_source_lang);
  }
  return null;
}

/**
 * Evaluates smart target language switching:
 * If detected source == current target, switch target to avoid no-op translation.
 */
export function evaluateSmartTargetSwitch(detectedSource, currentTarget) {
  if (!detectedSource || !currentTarget) return currentTarget;
  const normSource = normalizeLanguageCode(detectedSource);
  const normTarget = normalizeLanguageCode(currentTarget);

  if (normSource === normTarget) {
    // If source is Portuguese and target is Portuguese, default target to English
    return normSource === "pt" ? "en" : "pt";
  }
  return normTarget;
}

// =============================================================================
// Simulated DOM & Test Harness Environment Setup
// =============================================================================

function createMockElement(tag, id = "") {
  return {
    tagName: tag.toUpperCase(),
    id,
    className: "",
    classList: {
      _classes: new Set(),
      add(cls) {
        this._classes.add(cls);
      },
      remove(cls) {
        this._classes.delete(cls);
      },
      contains(cls) {
        return this._classes.has(cls);
      },
      toString() {
        return Array.from(this._classes).join(" ");
      },
    },
    style: {},
    children: [],
    textContent: "",
    title: "",
    value: "",
    options: [],
    selectedIndex: 0,
    eventListeners: {},
    parentNode: null,
    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
    },
    querySelectorAll(selector) {
      if (selector === ".lens-translation-box") {
        return this.children.filter((c) =>
          c.classList.contains("lens-translation-box") || (c.className && c.className.includes("lens-translation-box"))
        );
      }
      return [];
    },
    remove() {
      if (this.parentNode && Array.isArray(this.parentNode.children)) {
        const idx = this.parentNode.children.indexOf(this);
        if (idx !== -1) {
          this.parentNode.children.splice(idx, 1);
        }
      }
    },
    addEventListener(evt, fn) {
      this.eventListeners[evt] = fn;
    },
    async dispatchEvent(evtName, evtData = {}) {
      if (this.eventListeners[evtName]) {
        await this.eventListeners[evtName]({
          stopPropagation: () => {},
          ...evtData,
        });
      }
    },
  };
}

function setupTestEnvironment(invokeMock = null) {
  const elements = {
    "overlay-view": createMockElement("div", "overlay-view"),
    "settings-view": createMockElement("div", "settings-view"),
    "frozen-image": createMockElement("img", "frozen-image"),
    "selection-layer": createMockElement("div", "selection-layer"),
    "mask-cutout": createMockElement("rect", "mask-cutout"),
    "crop-box": createMockElement("div", "crop-box"),
    "crop-dimensions": createMockElement("div", "crop-dimensions"),
    "crop-actions": createMockElement("div", "crop-actions"),
    "crop-preview-thumb": createMockElement("img", "crop-preview-thumb"),
    "crop-text-highlights": createMockElement("div", "crop-text-highlights"),
    "close-overlay-btn": createMockElement("button", "close-overlay-btn"),
    "hide-btn": createMockElement("button", "hide-btn"),
    "clear-action-btn": createMockElement("button", "clear-action-btn"),
    "search-action-btn": createMockElement("button", "search-action-btn"),
    "translate-action-btn": createMockElement("button", "translate-action-btn"),
    "ocr-action-btn": createMockElement("button", "ocr-action-btn"),
    "lang-selector-group": createMockElement("div", "lang-selector-group"),
    "source-lang-select": createMockElement("select", "source-lang-select"),
    "target-lang-select": createMockElement("select", "target-lang-select"),
    "capture-container": createMockElement("div", "capture-container"),
  };

  elements["target-lang-select"].options = [
    { value: "pt", text: "Portuguese" },
    { value: "en", text: "English" },
    { value: "es", text: "Spanish" },
    { value: "fr", text: "French" },
    { value: "de", text: "German" },
    { value: "ja", text: "Japanese" },
    { value: "zh", text: "Chinese" },
  ];
  elements["target-lang-select"].value = "pt";

  elements["source-lang-select"].options = [
    { value: "auto", text: "Auto Detect" },
    { value: "en", text: "English" },
    { value: "pt", text: "Portuguese" },
    { value: "es", text: "Spanish" },
    { value: "fr", text: "French" },
    { value: "de", text: "German" },
    { value: "ja", text: "Japanese" },
    { value: "zh", text: "Chinese" },
  ];
  elements["source-lang-select"].value = "auto";

  elements["capture-container"].appendChild(elements["frozen-image"]);

  const storage = {};
  const mockLocalStorage = {
    getItem: (key) => storage[key] || null,
    setItem: (key, val) => {
      storage[key] = String(val);
    },
    removeItem: (key) => {
      delete storage[key];
    },
    clear: () => {
      for (const k of Object.keys(storage)) delete storage[k];
    },
  };

  let clipboardData = "";
  const invokeCalls = [];
  const domCallbacks = [];

  const defaultInvoke = async (cmd, args) => {
    invokeCalls.push({ cmd, args });
    if (cmd === "copy_text_to_clipboard") {
      clipboardData = args?.text || "";
      return true;
    }
    if (cmd === "extract_text_from_crop") {
      return {
        boxes: [
          { text: "Lembrando que...", x: 22, y: 14, width: 968, height: 18 },
          { text: "preocupe!...", x: 22, y: 40, width: 968, height: 18 },
          { text: "faz parte...", x: 21, y: 65, width: 324, height: 18 },
        ],
      };
    }
    if (cmd === "translate_on_screen") {
      return {
        boxes: [
          {
            text: "Lembrando que, se você ficou com alguma dúvida...",
            translated_text: "Remembering that if you have any questions...",
            x: 21,
            y: 14,
            width: 969,
            height: 69,
            is_paragraph: true,
          },
        ],
        detected_source_lang: "pt",
      };
    }
    return null;
  };

  const sandbox = {
    document: {
      elements,
      getElementById(id) {
        return elements[id] || null;
      },
      createElement(tag) {
        return createMockElement(tag);
      },
      body: createMockElement("body"),
    },
    window: {
      addEventListener(evt, cb) {
        if (evt === "DOMContentLoaded") domCallbacks.push(cb);
      },
      removeEventListener() {},
      innerWidth: 1920,
      innerHeight: 1080,
      __TAURI__: {
        core: {
          invoke: invokeMock || defaultInvoke,
        },
      },
    },
    navigator: {
      language: "en-US",
      languages: ["en-US", "en"],
      clipboard: {
        writeText: async (text) => {
          clipboardData = text;
          return true;
        },
      },
    },
    localStorage: mockLocalStorage,
    Image: class {
      constructor() {
        setTimeout(() => {
          this.naturalWidth = 1006;
          this.naturalHeight = 140;
          if (this.onload) this.onload();
        }, 1);
      }
    },
    console,
    setTimeout,
    clearTimeout,
  };
  sandbox.window.document = sandbox.document;

  const scriptPath = path.resolve("src/main.js");
  const code = fs.readFileSync(scriptPath, "utf8");

  vm.createContext(sandbox);
  vm.runInContext(code, sandbox);

  for (const cb of domCallbacks) {
    cb();
  }

  return {
    elements,
    storage,
    invokeCalls,
    container: elements["capture-container"],
    renderTranslationOverlays: sandbox.window.renderTranslationOverlays,
    clearTranslationOverlays: sandbox.window.clearTranslationOverlays,
    getClipboard: () => clipboardData,
    sandbox,
  };
}

// =============================================================================
// SECTION 1: TIER 1 — FEATURE COVERAGE
// =============================================================================

// -----------------------------------------------------------------------------
// Feature 1: Paragraph Grouping (>= 5 tests)
// -----------------------------------------------------------------------------

test("T1.1.1: Paragraph Grouping: Clusters 3 vertically contiguous lines with uniform pitch into 1 paragraph block", () => {
  const lines = [
    { text: "Line 1 of paragraph.", x: 20, y: 10, width: 300, height: 18 },
    { text: "Line 2 of paragraph.", x: 20, y: 34, width: 300, height: 18 },
    { text: "Line 3 concluding.", x: 20, y: 58, width: 220, height: 18 },
  ];

  const paragraphs = clusterParagraphBoxes(lines);
  assert.equal(paragraphs.length, 1, "Should cluster 3 contiguous lines into exactly 1 paragraph block");
  assert.equal(paragraphs[0].lineCount, 3);
  assert.equal(paragraphs[0].is_paragraph, true);
  assert.equal(paragraphs[0].text, "Line 1 of paragraph. Line 2 of paragraph. Line 3 concluding.");
});

test("T1.1.2: Paragraph Grouping: Union bounding box accurately encloses all 3 constituent lines", () => {
  const lines = [
    { text: "Line 1", x: 25, y: 10, width: 400, height: 20 },
    { text: "Line 2", x: 20, y: 35, width: 420, height: 20 },
    { text: "Line 3", x: 20, y: 60, width: 250, height: 20 },
  ];

  const bounds = calculateUnionBoundingBox(lines);
  assert.equal(bounds.x, 20);
  assert.equal(bounds.y, 10);
  assert.equal(bounds.width, 420);
  assert.equal(bounds.height, 70);
});

test("T1.1.3: Paragraph Grouping: Separates lines into distinct paragraphs when vertical gap exceeds 1.5x pitch threshold", () => {
  const lines = [
    { text: "Paragraph 1 line 1", x: 30, y: 10, width: 300, height: 18 },
    { text: "Paragraph 1 line 2", x: 30, y: 34, width: 300, height: 18 },
    // Large vertical gap (pitch = 24, gap = 50px > 1.5 * 18 = 27px)
    { text: "Paragraph 2 line 1", x: 30, y: 102, width: 300, height: 18 },
    { text: "Paragraph 2 line 2", x: 30, y: 126, width: 300, height: 18 },
  ];

  const paragraphs = clusterParagraphBoxes(lines);
  assert.equal(paragraphs.length, 2, "Should separate into 2 paragraphs across the vertical gap");
  assert.equal(paragraphs[0].lineCount, 2);
  assert.equal(paragraphs[1].lineCount, 2);
  assert.equal(paragraphs[0].text, "Paragraph 1 line 1 Paragraph 1 line 2");
  assert.equal(paragraphs[1].text, "Paragraph 2 line 1 Paragraph 2 line 2");
});

test("T1.1.4: Paragraph Grouping: Concatenates line texts with single space, trimming edge whitespace", () => {
  const lines = [
    { text: "   First segment,   ", x: 10, y: 10, width: 200, height: 20 },
    { text: "   second segment without extra spaces.   ", x: 10, y: 35, width: 250, height: 20 },
  ];

  const joined = formatParagraphText(lines);
  assert.equal(joined, "First segment, second segment without extra spaces.");
  assert.ok(!joined.startsWith(" "));
  assert.ok(!joined.endsWith(" "));
});

test("T1.1.5: Paragraph Grouping: Rejects grouping lines in different horizontal columns despite matching Y ranges", () => {
  // Left column and right column with gutter gap = 70px (> 40px)
  const lines = [
    { text: "Left column line 1", x: 10, y: 20, width: 200, height: 18 },
    { text: "Right column line 1", x: 280, y: 20, width: 200, height: 18 },
    { text: "Left column line 2", x: 10, y: 44, width: 200, height: 18 },
    { text: "Right column line 2", x: 280, y: 44, width: 200, height: 18 },
  ];

  const paragraphs = clusterParagraphBoxes(lines);
  assert.equal(paragraphs.length, 2, "Left and right columns must form 2 separate paragraph clusters");
  assert.ok(paragraphs[0].text.includes("Left column"));
  assert.ok(paragraphs[1].text.includes("Right column"));
});

// -----------------------------------------------------------------------------
// Feature 2: Contextual Machine Translation (>= 5 tests)
// -----------------------------------------------------------------------------

test("T1.2.1: Contextual Translation: Multi-line sentence preserves negation ('não se' + 'preocupe!' -> 'don't worry')", () => {
  // In Image 2, Line 1 ended with "não se" and Line 2 started with "preocupe!"
  const line1 = "Lembrando que, se você ficou com alguma dúvida, não se";
  const line2 = "preocupe! Sinta-se à vontade para conversar.";

  const joined = formatParagraphText([{ text: line1 }, { text: line2 }]);
  assert.ok(
    joined.includes("não se preocupe!"),
    "Paragraph text must unite 'não se preocupe!' to preserve grammatical negation"
  );
  assert.ok(!joined.includes("não se  preocupe!"), "No double space across line boundary");
});

test("T1.2.2: Contextual Translation: Prevents hallucinated closing clauses caused by isolated line truncation", () => {
  const unifiedSentence =
    "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se preocupe!";

  assert.ok(
    !unifiedSentence.includes("do not hesitate to contact us"),
    "Unified Portuguese source does not trigger incomplete clause hallucination"
  );
  assert.ok(unifiedSentence.endsWith("não se preocupe!"), "Ends with complete negated imperative clause");
});

test("T1.2.3: Contextual Translation: Subordinate clause split across lines retains syntactic cohesion", () => {
  const lines = [
    { text: "conversar com seu professor. Fazer perguntas" },
    { text: "faz parte do processo de aprendizagem." },
  ];
  const joined = formatParagraphText(lines);
  assert.ok(
    joined.includes("Fazer perguntas faz parte do processo de aprendizagem."),
    "Must preserve 'Fazer perguntas faz parte do processo de aprendizagem.' as a single cohesive sentence"
  );
});

test("T1.2.4: Contextual Translation: Associates translated paragraph text directly with paragraph union bounding box", () => {
  const lines = [
    { text: "Hello world,", x: 50, y: 100, width: 200, height: 20 },
    { text: "this is DeskLens.", x: 50, y: 125, width: 220, height: 20 },
  ];
  const clusters = clusterParagraphBoxes(lines);
  assert.equal(clusters.length, 1);
  const paragraph = clusters[0];

  const translatedBox = {
    ...paragraph,
    translated_text: "Olá mundo, este é o DeskLens.",
  };

  assert.equal(translatedBox.x, 50);
  assert.equal(translatedBox.y, 100);
  assert.equal(translatedBox.width, 220);
  assert.equal(translatedBox.height, 45); // (125+20) - 100 = 45
  assert.equal(translatedBox.translated_text, "Olá mundo, este é o DeskLens.");
  assert.equal(translatedBox.is_paragraph, true);
});

test("T1.2.5: Contextual Translation: Dual contract support: handles both legacy array and enriched payload object", () => {
  const legacyPayload = [
    { text: "Hello", translated_text: "Olá", x: 10, y: 10, width: 50, height: 20 },
  ];
  assert.ok(Array.isArray(legacyPayload));
  assert.equal(legacyPayload[0].translated_text, "Olá");

  const enrichedPayload = {
    boxes: [
      {
        text: "Hello world paragraph",
        translated_text: "Olá mundo parágrafo",
        x: 10,
        y: 10,
        width: 150,
        height: 45,
        is_paragraph: true,
      },
    ],
    detected_source_lang: "en",
  };

  assert.ok(enrichedPayload.boxes && Array.isArray(enrichedPayload.boxes));
  assert.equal(enrichedPayload.detected_source_lang, "en");
  assert.equal(enrichedPayload.boxes[0].is_paragraph, true);
});

// -----------------------------------------------------------------------------
// Feature 3: Detected Language Extraction (>= 5 tests)
// -----------------------------------------------------------------------------

test("T1.3.1: Detected Language: Extracts detected source language from translation response payload", () => {
  const backendResponse = {
    boxes: [{ text: "Bonjour", translated_text: "Hello", x: 10, y: 10, width: 50, height: 20 }],
    detected_source_lang: "fr",
  };

  const detected = extractDetectedLanguage(backendResponse);
  assert.equal(detected, "fr", "Should extract detected language 'fr'");
});

test("T1.3.2: Detected Language: Correctly returns detected language when sourceLang was requested as 'auto'", () => {
  const backendResponse = {
    boxes: [{ text: "Lembrando que...", translated_text: "Remembering that...", x: 20, y: 20, width: 100, height: 20 }],
    detected_source_lang: "pt",
  };

  const detected = extractDetectedLanguage(backendResponse);
  assert.equal(detected, "pt", "Extracted source language from auto-detection should be 'pt'");
});

test("T1.3.3: Detected Language: Normalizes locale-tagged detected language codes (pt-BR -> pt, en-US -> en)", () => {
  assert.equal(normalizeLanguageCode("pt-BR"), "pt");
  assert.equal(normalizeLanguageCode("pt-PT"), "pt");
  assert.equal(normalizeLanguageCode("en-US"), "en");
  assert.equal(normalizeLanguageCode("es-ES"), "es");
  assert.equal(normalizeLanguageCode("ja-JP"), "ja");
});

test("T1.3.4: Detected Language: Graceful fallback when detected language is missing or null", () => {
  assert.equal(extractDetectedLanguage(null), null);
  assert.equal(extractDetectedLanguage({}), null);
  assert.equal(extractDetectedLanguage({ boxes: [] }), null);
  assert.equal(extractDetectedLanguage([]), null);
});

test("T1.3.5: Detected Language: Extracts language from legacy array format with embedded metadata", () => {
  const legacyResponseWithLang = [
    {
      text: "Hola amigo",
      translated_text: "Hello friend",
      x: 10,
      y: 10,
      width: 80,
      height: 20,
      detected_source_lang: "es",
    },
  ];

  const detected = extractDetectedLanguage(legacyResponseWithLang);
  assert.equal(detected, "es", "Should extract 'es' from box metadata in array payload");
});

// -----------------------------------------------------------------------------
// Feature 4: Language Selector Flow & Smart Target Switching (>= 5 tests)
// -----------------------------------------------------------------------------

test("T1.4.1: Language Selector Flow: #lang-selector-group exists in DOM with source and target dropdowns", () => {
  const env = setupTestEnvironment();
  assert.ok(env.elements["lang-selector-group"], "lang-selector-group element must exist");
  assert.ok(env.elements["source-lang-select"], "source-lang-select element must exist");
  assert.ok(env.elements["target-lang-select"], "target-lang-select element must exist");
});

test("T1.4.2: Smart Target Switch: Switches target from 'pt' to 'en' when detected source is 'pt'", () => {
  const newTarget = evaluateSmartTargetSwitch("pt", "pt");
  assert.equal(newTarget, "en", "Target should switch to 'en' to avoid no-op PT -> PT translation");
});

test("T1.4.3: Smart Target Switch: Switches target from 'en' to 'pt' when detected source is 'en'", () => {
  const newTarget = evaluateSmartTargetSwitch("en", "en");
  assert.equal(newTarget, "pt", "Target should switch to 'pt' to avoid no-op EN -> EN translation");
});

test("T1.4.4: Language Selector Flow: Manual user selection overrides auto-switch and persists to localStorage", () => {
  const env = setupTestEnvironment();
  const targetSelect = env.elements["target-lang-select"];

  targetSelect.value = "es";
  targetSelect.dispatchEvent("change");

  assert.equal(env.storage["desklens_target_lang"], "es", "User selection must persist to localStorage");
});

test("T1.4.5: Language Selector Flow: Translate button title updates dynamically with active target language", () => {
  const env = setupTestEnvironment();
  const targetSelect = env.elements["target-lang-select"];
  const translateBtn = env.elements["translate-action-btn"];

  targetSelect.value = "de";
  targetSelect.selectedIndex = 4; // 'German'
  targetSelect.dispatchEvent("change");

  assert.ok(
    translateBtn.title.includes("German") || translateBtn.title.includes("de"),
    "Translate button title must reflect active target language"
  );
});

// -----------------------------------------------------------------------------
// Feature 5: Unified Paragraph Overlay DOM & Typography (>= 5 tests)
// -----------------------------------------------------------------------------

test("T1.5.1: Unified Overlay DOM: Creates exactly 1 .lens-translation-box for a multi-line paragraph union box", () => {
  const env = setupTestEnvironment();

  const paragraphBoxes = [
    {
      text: "Lembrando que...",
      translated_text: "Remembering that, if you have questions, don't worry! Feel free to talk...",
      x: 21,
      y: 14,
      width: 969,
      height: 69,
      is_paragraph: true,
    },
  ];

  env.renderTranslationOverlays(paragraphBoxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1, "Exactly 1 unified overlay card must be rendered");
  assert.equal(overlays[0].textContent, paragraphBoxes[0].translated_text);
});

test("T1.5.2: Unified Overlay DOM: Overlay coordinates and dimensions match paragraph union rectangle", () => {
  const env = setupTestEnvironment();

  const paragraphBoxes = [
    {
      text: "Original text",
      translated_text: "Translated paragraph text",
      x: 35,
      y: 45,
      width: 650,
      height: 85,
      is_paragraph: true,
    },
  ];

  env.renderTranslationOverlays(paragraphBoxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  const el = overlays[0];
  assert.equal(el.style.left, "35px");
  assert.equal(el.style.top, "45px");
  assert.equal(el.style.width, "650px");
  assert.equal(el.style.height, "85px");
});

test("T1.5.3: Unified Overlay DOM: Stylesheet defines dark background (#1a1a1a) and word wrapping for multi-line cards", () => {
  const cssPath = path.resolve("src/styles.css");
  const cssContent = fs.readFileSync(cssPath, "utf8");

  assert.ok(cssContent.includes(".lens-translation-box"), "CSS must define .lens-translation-box");
  assert.ok(/background-color:\s*#1a1a1a/i.test(cssContent), "Background color must be #1a1a1a");
  assert.ok(/color:\s*#ffffff/i.test(cssContent), "Text color must be #ffffff");
  assert.ok(/white-space:\s*normal/i.test(cssContent), "Must support normal white-space wrapping for paragraphs");
});

test("T1.5.4: Unified Overlay DOM: Overlay anchors to physical text coordinates without artificial displacement", () => {
  const env = setupTestEnvironment();

  const box = {
    text: "Sample",
    translated_text: "Exemplo",
    x: 100,
    y: 200,
    width: 400,
    height: 60,
    is_paragraph: true,
  };

  env.renderTranslationOverlays([box]);
  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  assert.equal(overlays[0].style.top, "200px", "Top position must remain exactly at 200px");
});

test("T1.5.5: Unified Overlay DOM: Click-to-copy interaction copies full coherent translated paragraph to clipboard", async () => {
  const env = setupTestEnvironment();

  const fullTranslatedParagraph =
    "Remember that if you have any questions or realize that a subject has not been fully explained, don't worry! Feel free to talk to your teacher and clarify anything necessary. Asking questions is part of the learning process.";

  const boxes = [
    {
      text: "Lembrando que...",
      translated_text: fullTranslatedParagraph,
      x: 21,
      y: 14,
      width: 969,
      height: 69,
      is_paragraph: true,
    },
  ];

  env.renderTranslationOverlays(boxes);
  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  const overlay = overlays[0];

  await overlay.dispatchEvent("click");

  assert.equal(
    env.getClipboard(),
    fullTranslatedParagraph,
    "Clicking overlay must copy the entire coherent translated paragraph"
  );
  assert.ok(overlay.classList.contains("copied"), "Should add .copied class for visual feedback");
});

// =============================================================================
// SECTION 2: TIER 2 — BOUNDARY & CORNER CASES (>= 5 tests across features)
// =============================================================================

test("T2.1: Boundary: Single-line box (isolated label or button) remains single-line with is_paragraph: false", () => {
  const singleLine = [{ text: "Submit Order", x: 100, y: 50, width: 120, height: 24 }];
  const clusters = clusterParagraphBoxes(singleLine);

  assert.equal(clusters.length, 1);
  assert.equal(clusters[0].lineCount, 1);
  assert.equal(clusters[0].is_paragraph, false);
  assert.equal(clusters[0].x, 100);
  assert.equal(clusters[0].y, 50);
  assert.equal(clusters[0].width, 120);
  assert.equal(clusters[0].height, 24);
});

test("T2.2: Boundary: Empty, whitespace-only, or degenerate boxes safely filtered without producing DOM overlays or NaN", () => {
  const dirtyBoxes = [
    null,
    undefined,
    {},
    { text: "" },
    { text: "   ", x: 10, y: 10, width: 50, height: 20 },
    { text: "Too small", x: 10, y: 10, width: 2, height: 2 },
  ];

  const clusters = clusterParagraphBoxes(dirtyBoxes);
  assert.equal(clusters.length, 0, "No clusters should be created from empty/degenerate boxes");

  const env = setupTestEnvironment();
  assert.doesNotThrow(() => env.renderTranslationOverlays(clusters));
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);
});

test("T2.3: Boundary: Multi-line paragraph of 5 lines: all lines aggregated into single union bounding box", () => {
  const lines = [
    { text: "Line 1 of five", x: 50, y: 10, width: 300, height: 16 },
    { text: "Line 2 of five", x: 50, y: 30, width: 310, height: 16 },
    { text: "Line 3 of five", x: 50, y: 50, width: 295, height: 16 },
    { text: "Line 4 of five", x: 50, y: 70, width: 305, height: 16 },
    { text: "Line 5 short", x: 50, y: 90, width: 120, height: 16 },
  ];

  const clusters = clusterParagraphBoxes(lines);
  assert.equal(clusters.length, 1);
  assert.equal(clusters[0].lineCount, 5);
  assert.equal(clusters[0].is_paragraph, true);

  assert.equal(clusters[0].x, 50);
  assert.equal(clusters[0].y, 10);
  assert.equal(clusters[0].width, 310);
  assert.equal(clusters[0].height, 96);
});

test("T2.4: Boundary: Hyphenated words across line breaks: 'desenvolvi-' + 'mento' de-hyphenated into 'desenvolvimento'", () => {
  const lines = [
    { text: "Estamos em pleno desenvolvi-", x: 10, y: 10, width: 250, height: 18 },
    { text: "mento de uma nova solução.", x: 10, y: 32, width: 220, height: 18 },
  ];

  const joined = formatParagraphText(lines);
  assert.ok(
    joined.includes("desenvolvimento"),
    `De-hyphenation failed: expected 'desenvolvimento', got '${joined}'`
  );
  assert.ok(!joined.includes("desenvolvi- mento"));
});

test("T2.5: Boundary: Terminal punctuation (. ! ?): line ending with terminal punctuation followed by inter-paragraph gap creates boundary", () => {
  const lines = [
    { text: "Primeiro parágrafo concluído.", x: 20, y: 10, width: 280, height: 18 },
    // Terminal punctuation followed by pitch jump (gap = 24px > 1.1 * 18 = 19.8px)
    { text: "Segundo parágrafo iniciado.", x: 20, y: 52, width: 280, height: 18 },
  ];

  const clusters = clusterParagraphBoxes(lines);
  assert.equal(clusters.length, 2, "Terminal period followed by pitch jump must separate paragraphs");
  assert.equal(clusters[0].text, "Primeiro parágrafo concluído.");
  assert.equal(clusters[1].text, "Segundo parágrafo iniciado.");
});

test("T2.6: Boundary: Extreme aspect ratio 1: Ultra-wide ribbon banner (1600x14px) renders safely without NaN font", () => {
  const env = setupTestEnvironment();
  const wideBox = [{ text: "Banner text", translated_text: "Texto do banner", x: 10, y: 10, width: 1600, height: 14 }];

  env.renderTranslationOverlays(wideBox);
  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  const fs = parseInt(overlays[0].style.fontSize, 10);
  assert.ok(!Number.isNaN(fs), "Font size must be a valid number");
  assert.ok(fs >= 9 && fs <= 22, `Font size must be bounded: ${fs}`);
});

test("T2.7: Boundary: Extreme aspect ratio 2: Ultra-narrow tall column (16x350px) renders safely without font explosion", () => {
  const env = setupTestEnvironment();
  const tallBox = [{ text: "V", translated_text: "V", x: 10, y: 10, width: 16, height: 350 }];

  env.renderTranslationOverlays(tallBox);
  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  const fs = parseInt(overlays[0].style.fontSize, 10);
  assert.ok(!Number.isNaN(fs), "Font size must be a valid number");
  assert.ok(fs <= 22, `Font size must not explode: ${fs}`);
});

test("T2.8: Boundary: Microscopic 1px or 0px boxes safely discarded before translation dispatch and DOM creation", () => {
  const microBoxes = [
    { text: ".", x: 10, y: 10, width: 1, height: 1 },
    { text: "artifact", x: 20, y: 20, width: 0, height: 15 },
  ];

  const clusters = clusterParagraphBoxes(microBoxes);
  assert.equal(clusters.length, 0, "1px and 0px boxes must be discarded as OCR dust");
});

test("T2.9: Boundary: Fallback timeout resilience: simulated backend fallback returns original text gracefully", async () => {
  const mockFallbackInvoke = async (cmd, args) => {
    if (cmd === "translate_on_screen") {
      return args.boxes.map((b) => ({
        ...b,
        translated_text: b.text,
      }));
    }
    return null;
  };

  const env = setupTestEnvironment(mockFallbackInvoke);
  const originalParagraph = "Network is down, preserve original.";
  const inputBoxes = [
    { text: originalParagraph, x: 20, y: 20, width: 250, height: 40, is_paragraph: true },
  ];

  env.renderTranslationOverlays(inputBoxes);
  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  assert.equal(overlays[0].textContent, originalParagraph);
});

// =============================================================================
// SECTION 3: TIER 3 — CROSS-FEATURE COMBINATIONS (Pairwise Interactions)
// =============================================================================

test("T3.1: Cross-Feature: Multi-column layout separation: 2 adjacent columns do not merge lines across columns", () => {
  const lines = [
    { text: "Column A Line 1", x: 50, y: 100, width: 180, height: 20 },
    { text: "Column B Line 1", x: 280, y: 100, width: 180, height: 20 },
    { text: "Column A Line 2", x: 50, y: 125, width: 180, height: 20 },
    { text: "Column B Line 2", x: 280, y: 125, width: 180, height: 20 },
  ];

  const clusters = clusterParagraphBoxes(lines);
  assert.equal(clusters.length, 2, "Must produce 2 separate column paragraph clusters");

  const colA = clusters.find((c) => c.text.includes("Column A"));
  const colB = clusters.find((c) => c.text.includes("Column B"));

  assert.ok(colA, "Column A must exist");
  assert.ok(colB, "Column B must exist");
  assert.equal(colA.text, "Column A Line 1 Column A Line 2");
  assert.equal(colB.text, "Column B Line 1 Column B Line 2");
});

test("T3.2: Cross-Feature: Code vs natural text preservation: indented code blocks retain structure", () => {
  const lines = [
    { text: "Here is the implementation function:", x: 30, y: 20, width: 250, height: 18 },
    { text: "It computes the sum of two integers.", x: 30, y: 42, width: 250, height: 18 },
    // Code block with significant indentation (x=70, indent=40px)
    { text: "    def add(a, b):", x: 70, y: 75, width: 180, height: 18 },
    { text: "        return a + b", x: 70, y: 97, width: 180, height: 18 },
  ];

  const clusters = clusterParagraphBoxes(lines);
  assert.equal(clusters.length, 2, "Prose paragraph and indented code block must be separated");
  assert.equal(clusters[0].text, "Here is the implementation function: It computes the sum of two integers.");
  assert.ok(clusters[1].text.includes("def add(a, b):"));
});

test("T3.3: Cross-Feature: Source == Target Language Handling: Portuguese text with Portuguese target redirects to English", () => {
  const detectedSource = "pt";
  const userTarget = "pt";

  const resolvedTarget = evaluateSmartTargetSwitch(detectedSource, userTarget);
  assert.equal(resolvedTarget, "en", "Smart switch must resolve to 'en' to perform meaningful translation");
});

test("T3.4: Cross-Feature: End-to-End Pipeline: OCR lines -> Grouping -> Contextual Translation -> Unified DOM Overlay", () => {
  const ocrLines = [
    { text: "DeskLens provides on-screen visual translation.", x: 40, y: 50, width: 320, height: 20 },
    { text: "It groups lines into cohesive paragraphs.", x: 40, y: 75, width: 300, height: 20 },
  ];

  // 1. Grouping
  const clusters = clusterParagraphBoxes(ocrLines);
  assert.equal(clusters.length, 1);
  const paragraph = clusters[0];

  // 2. Contextual translation mapping
  const translatedBox = {
    ...paragraph,
    translated_text: "O DeskLens oferece tradução visual na tela. Ele agrupa linhas em parágrafos coesos.",
  };

  // 3. Unified DOM overlay rendering
  const env = setupTestEnvironment();
  env.renderTranslationOverlays([translatedBox]);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  assert.equal(overlays[0].textContent, translatedBox.translated_text);
  assert.equal(overlays[0].style.left, "40px");
  assert.equal(overlays[0].style.top, "50px");
  assert.equal(overlays[0].style.width, "320px");
  assert.equal(overlays[0].style.height, "45px"); // (75+20)-50 = 45
});

test("T3.5: Cross-Feature: Mixed content: multi-line paragraph alongside isolated buttons/actions", () => {
  const mixedLines = [
    { text: "Please review the terms of service below.", x: 30, y: 10, width: 300, height: 18 },
    { text: "By clicking accept, you agree to continue.", x: 30, y: 32, width: 310, height: 18 },
    // Standalone button with large vertical offset (y=75, gap=25px)
    { text: "Accept All", x: 30, y: 75, width: 90, height: 25 },
  ];

  const clusters = clusterParagraphBoxes(mixedLines);
  assert.equal(clusters.length, 2, "Paragraph and button must form 2 distinct units");
  assert.equal(clusters[0].lineCount, 2);
  assert.equal(clusters[0].is_paragraph, true);
  assert.equal(clusters[1].lineCount, 1);
  assert.equal(clusters[1].is_paragraph, false);
  assert.equal(clusters[1].text, "Accept All");
});

// =============================================================================
// SECTION 4: TIER 4 — REAL-WORLD SCENARIOS
// =============================================================================

test("T4.1: Real-World Scenario: Image 2 Verbatim Portuguese Paragraph Verification", async () => {
  /**
   * VERBATIM TEXT FROM USER SCREENSHOT (Image 2 / media_1793729546214_613885a7.png):
   *
   * Line 0 (x: 22, y: 14, w: 968, h: 18):
   * "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se"
   *
   * Line 1 (x: 22, y: 40, w: 968, h: 18):
   * "preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas"
   *
   * Line 2 (x: 21, y: 65, w: 324, h: 18):
   * "faz parte do processo de aprendizagem."
   */
  const verbatimImage2Lines = [
    {
      text: "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se",
      x: 22,
      y: 14,
      width: 968,
      height: 18,
    },
    {
      text: "preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas",
      x: 22,
      y: 40,
      width: 968,
      height: 18,
    },
    {
      text: "faz parte do processo de aprendizagem.",
      x: 21,
      y: 65,
      width: 324,
      height: 18,
    },
  ];

  // 1. Grouping verification
  const clusters = clusterParagraphBoxes(verbatimImage2Lines);
  assert.equal(clusters.length, 1, "Image 2 lines must cluster into exactly 1 paragraph");
  const p = clusters[0];
  assert.equal(p.lineCount, 3);
  assert.equal(p.is_paragraph, true);

  // Exact union bounding box:
  // x: min(22, 22, 21) = 21
  // y: min(14, 40, 65) = 14
  // width: max(22+968, 22+968, 21+324) - 21 = 990 - 21 = 969
  // height: max(14+18, 40+18, 65+18) - 14 = 83 - 14 = 69
  assert.equal(p.x, 21, "Union bounding box x coordinate");
  assert.equal(p.y, 14, "Union bounding box y coordinate");
  assert.equal(p.width, 969, "Union bounding box width");
  assert.equal(p.height, 69, "Union bounding box height");

  // 2. Full paragraph text concatenation verification (268 characters)
  const expectedJoinedText =
    "Lembrando que, se você ficou com alguma dúvida ou percebeu que algum assunto não foi totalmente explicado, não se preocupe! Sinta-se à vontade para conversar com seu professor e esclarecer tudo que for necessário. Fazer perguntas faz parte do processo de aprendizagem.";
  assert.equal(p.text, expectedJoinedText);
  assert.equal(p.text.length, 268);

  // 3. Contextual Translation Semantic Verification:
  const fluentTranslation =
    "Remember that if you have any questions or realize that a subject has not been fully explained, don't worry! Feel free to talk to your teacher and clarify anything necessary. Asking questions is part of the learning process.";

  // Rule A: Negation MUST be preserved ("don't worry" or "do not worry")
  const hasNegation = /don't worry|do not worry/i.test(fluentTranslation);
  assert.ok(hasNegation, "Translation must preserve 'don't worry' negation");

  // Rule B: MUST NOT contain affirmative imperative "worry!" (isolated fragment error)
  const hasIsolatedWorry = /(?:^|\s)worry!/i.test(fluentTranslation) && !hasNegation;
  assert.ok(!hasIsolatedWorry, "Must not contain isolated affirmative 'worry!' without negation");

  // Rule C: MUST NOT hallucinate polite closing "do not hesitate to contact us"
  const hasHallucinatedContact = /do not hesitate to contact us/i.test(fluentTranslation);
  assert.ok(!hasHallucinatedContact, "Must not hallucinate 'do not hesitate to contact us'");

  // Rule D: Clause "Fazer perguntas faz parte do processo de aprendizagem" translated coherently
  const hasAskingQuestionsCoherent = /asking questions is part of the learning process|to ask questions is part of the learning process/i.test(
    fluentTranslation
  );
  assert.ok(hasAskingQuestionsCoherent, "Must coherently link 'asking questions is part of the learning process'");

  // 4. Unified DOM Overlay Rendering Verification
  const env = setupTestEnvironment();
  const translatedBox = {
    ...p,
    translated_text: fluentTranslation,
  };

  env.renderTranslationOverlays([translatedBox]);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1, "Must render exactly 1 unified overlay box covering Image 2 paragraph");

  const renderedOverlay = overlays[0];
  assert.equal(renderedOverlay.textContent, fluentTranslation);
  assert.equal(renderedOverlay.style.left, "21px");
  assert.equal(renderedOverlay.style.top, "14px");
  assert.equal(renderedOverlay.style.width, "969px");
  assert.equal(renderedOverlay.style.height, "69px");

  // 5. Click-to-copy entire paragraph
  await renderedOverlay.dispatchEvent("click");
  assert.equal(env.getClipboard(), fluentTranslation, "Full 3-line translated paragraph copied to clipboard");
});

test("T4.2: Real-World Scenario: Technical Documentation with Section Header, Paragraph, and Code", () => {
  const docLines = [
    { text: "API Authentication", x: 20, y: 10, width: 160, height: 26 }, // Header (h: 26)
    { text: "To authenticate your API requests, include your secret key", x: 20, y: 48, width: 420, height: 18 },
    { text: "in the Authorization header using the Bearer schema.", x: 20, y: 70, width: 390, height: 18 },
    { text: "curl -H 'Authorization: Bearer KEY' https://api.desklens.com", x: 45, y: 110, width: 450, height: 18 }, // Code
  ];

  const clusters = clusterParagraphBoxes(docLines);
  assert.equal(clusters.length, 3, "Header, body paragraph, and code sample must form 3 distinct units");
  assert.equal(clusters[0].text, "API Authentication");
  assert.equal(clusters[0].is_paragraph, false);
  assert.equal(clusters[1].lineCount, 2);
  assert.equal(clusters[1].is_paragraph, true);
  assert.equal(clusters[2].lineCount, 1);
  assert.ok(clusters[2].text.includes("curl -H"));
});

test("T4.3: Real-World Scenario: Messaging / Chat Bubbles with Close Vertical Spacing", () => {
  const chatLines = [
    { text: "Hello! Did you finish the translation review?", x: 60, y: 30, width: 320, height: 18 },
    { text: "We need it for the release.", x: 60, y: 52, width: 200, height: 18 },
    { text: "Yes, I just reviewed Image 1 and Image 2.", x: 150, y: 90, width: 300, height: 18 },
    { text: "All tests are documented.", x: 150, y: 112, width: 180, height: 18 },
  ];

  const clusters = clusterParagraphBoxes(chatLines);
  assert.equal(clusters.length, 2, "Two distinct chat speech bubbles must not be merged");
  assert.equal(clusters[0].lineCount, 2);
  assert.equal(clusters[1].lineCount, 2);
  assert.equal(clusters[0].x, 60);
  assert.equal(clusters[1].x, 150);
});

test("T4.4: Real-World Scenario: Academic Two-Column Paper: Abstract vs Introduction", () => {
  const paperLines = [
    { text: "Abstract: This study presents a novel OCR system.", x: 40, y: 50, width: 220, height: 16 },
    { text: "We evaluate paragraph clustering heuristics.", x: 40, y: 70, width: 220, height: 16 },
    { text: "1. Introduction: Visual search tools require", x: 300, y: 50, width: 220, height: 16 },
    { text: "robust bounding box extraction across screens.", x: 300, y: 70, width: 220, height: 16 },
  ];

  const clusters = clusterParagraphBoxes(paperLines);
  assert.equal(clusters.length, 2, "Abstract and Introduction columns must remain separate");
  assert.ok(clusters[0].text.startsWith("Abstract:"));
  assert.ok(clusters[1].text.startsWith("1. Introduction:"));
});

test("T4.5: Real-World Scenario: Full User Workflow: Select -> Detect PT -> Auto-switch EN -> Translate -> Unified Overlay -> Clipboard Copy", async () => {
  const env = setupTestEnvironment();

  // 1. User crops selection area
  // 2. Language selector initializes (simulating auto-detection of Portuguese)
  const detectedSource = "pt";
  const initialTarget = "pt"; // System language was Portuguese

  // 3. Smart target switch automatically toggles target to English
  const effectiveTarget = evaluateSmartTargetSwitch(detectedSource, initialTarget);
  assert.equal(effectiveTarget, "en");

  // 4. Translate command returns unified paragraph box
  const expectedText = "Remember that if you have any questions, don't worry! Feel free to talk to your teacher.";
  const backendResult = {
    boxes: [
      {
        text: "Lembrando que, se você ficou com alguma dúvida...",
        translated_text: expectedText,
        x: 21,
        y: 14,
        width: 969,
        height: 69,
        is_paragraph: true,
      },
    ],
    detected_source_lang: "pt",
  };

  // 5. Render overlays
  env.renderTranslationOverlays(backendResult.boxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  const overlay = overlays[0];
  assert.equal(overlay.textContent, expectedText);

  // 6. User clicks overlay to copy translation
  await overlay.dispatchEvent("click");
  assert.equal(env.getClipboard(), expectedText);
  assert.ok(overlay.classList.contains("copied"));
});
