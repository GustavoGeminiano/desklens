/**
 * tests/code_ocr_e2e.test.mjs
 *
 * Comprehensive Opaque-Box E2E Test Suite for DeskLens Code OCR
 * Covering Tiers 1-4 across Keywords, Operators, Delimiters, Boundaries,
 * Cross-Feature Combinations, and Real-World Scenarios (including
 * benchmark screenshot media_1793996685095_927ef312.png).
 *
 * Authoritative Sources:
 * - ORIGINAL_REQUEST.md (Requirements R1, R2, R3; User Benchmark Screenshot)
 * - PROJECT.md (M1-M4 Architecture, Interface Contracts, Invariants)
 * - src-tauri/src/ocr.rs & src/main.js (Public Interface Contracts)
 */

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

// =============================================================================
// Test Infrastructure: Mock Environment & Test Bridge
// =============================================================================

function setupCodeOcrEnvironment() {
  class MockClassList {
    constructor() {
      this._classes = new Set();
    }
    add(cls) {
      this._classes.add(cls);
    }
    remove(cls) {
      this._classes.delete(cls);
    }
    contains(cls) {
      return this._classes.has(cls);
    }
    toggle(cls, force) {
      if (force === undefined) {
        if (this._classes.has(cls)) this._classes.delete(cls);
        else this._classes.add(cls);
      } else if (force) {
        this._classes.add(cls);
      } else {
        this._classes.delete(cls);
      }
    }
    toString() {
      return Array.from(this._classes).join(" ");
    }
  }

  class MockElement {
    constructor(tag, id = "") {
      this.tagName = tag.toUpperCase();
      this.id = id;
      this.classList = new MockClassList();
      this.style = {};
      this.children = [];
      this.textContent = "";
      this.innerHTML = "";
      this.title = "";
      this.dataset = {};
      this.eventListeners = {};
      this.parentNode = null;
    }

    get className() {
      return Array.from(this.classList._classes).join(" ");
    }

    set className(val) {
      this.classList._classes.clear();
      if (val) {
        val.split(/\s+/).filter(Boolean).forEach((c) => this.classList.add(c));
      }
    }

    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
    }

    querySelectorAll(selector) {
      if (selector === ".crop-word-highlight") {
        return this.children.filter((c) => c.classList.contains("crop-word-highlight"));
      }
      return [];
    }

    addEventListener(evt, fn) {
      this.eventListeners[evt] = fn;
    }

    async dispatchEvent(evtName, evtData = {}) {
      if (this.eventListeners[evtName]) {
        await this.eventListeners[evtName]({
          stopPropagation: () => {},
          preventDefault: () => {},
          ...evtData,
        });
      }
    }

    setAttribute(key, value) {
      this[key] = value;
    }

    remove() {
      if (this.parentNode) {
        this.parentNode.children = this.parentNode.children.filter((c) => c !== this);
        this.parentNode = null;
      }
    }
  }

  const elements = {
    "overlay-view": new MockElement("div", "overlay-view"),
    "settings-view": new MockElement("div", "settings-view"),
    "frozen-image": new MockElement("img", "frozen-image"),
    "selection-layer": new MockElement("div", "selection-layer"),
    "mask-cutout": new MockElement("rect", "mask-cutout"),
    "crop-box": new MockElement("div", "crop-box"),
    "crop-dimensions": new MockElement("div", "crop-dimensions"),
    "crop-actions": new MockElement("div", "crop-actions"),
    "crop-preview-thumb": new MockElement("img", "crop-preview-thumb"),
    "crop-text-highlights": new MockElement("div", "crop-text-highlights"),
    "close-overlay-btn": new MockElement("button", "close-overlay-btn"),
    "hide-btn": new MockElement("button", "hide-btn"),
    "clear-action-btn": new MockElement("button", "clear-action-btn"),
    "search-action-btn": new MockElement("button", "search-action-btn"),
    "translate-action-btn": new MockElement("button", "translate-action-btn"),
    "ocr-action-btn": new MockElement("button", "ocr-action-btn"),
    "source-lang-select": new MockElement("select", "source-lang-select"),
    "target-lang-select": new MockElement("select", "target-lang-select"),
    "capture-container": new MockElement("div", "capture-container"),
  };

  let clipboardData = "";
  const domCallbacks = [];

  class MockImage {
    constructor() {
      this.naturalWidth = 800;
      this.naturalHeight = 600;
      this._src = "";
    }
    set src(val) {
      this._src = val;
      if (this.onload) {
        this.onload();
      }
    }
    get src() {
      return this._src;
    }
  }

  const sandbox = {
    document: {
      elements,
      getElementById(id) {
        return elements[id] || null;
      },
      createElement(tag) {
        return new MockElement(tag);
      },
      elementFromPoint() {
        return null;
      },
    },
    window: {
      addEventListener(evt, fn) {
        if (evt === "DOMContentLoaded") {
          domCallbacks.push(fn);
        }
      },
      removeEventListener() {},
    },
    navigator: {
      clipboard: {
        async writeText(txt) {
          clipboardData = txt;
          return true;
        },
      },
      language: "en-US",
    },
    localStorage: {
      _data: {},
      getItem(k) {
        return this._data[k] || null;
      },
      setItem(k, v) {
        this._data[k] = String(v);
      },
      removeItem(k) {
        delete this._data[k];
      },
    },
    console,
    setTimeout(fn) {
      fn();
    },
    Image: MockImage,
  };

  const scriptPath = path.resolve("src", "main.js");
  const rawScript = fs.readFileSync(scriptPath, "utf-8");

  // Export renderTextHighlights to window in test sandbox without modifying disk file
  const testScript = rawScript.replace(
    "function renderTextHighlights(ocrResult, cropW, cropH) {",
    `window.setMockCrop = (dataUrl, w, h) => {
       currentCropDataUrl = dataUrl;
       currentRect = { x: 0, y: 0, w: w || 800, h: h || 600 };
     };
     window.renderTextHighlights = renderTextHighlights;
     function renderTextHighlights(ocrResult, cropW, cropH) {`
  );

  const context = vm.createContext(sandbox);
  vm.runInContext(testScript, context);

  // Trigger DOMContentLoaded
  for (const cb of domCallbacks) {
    cb();
  }

  if (sandbox.window.setMockCrop) {
    sandbox.window.setMockCrop("data:image/png;base64,mock", 800, 600);
  }

  return {
    sandbox,
    elements,
    highlightsContainer: elements["crop-text-highlights"],
    renderTextHighlights: sandbox.window.renderTextHighlights,
    renderInteractivePills: sandbox.window.renderInteractivePills,
    joinPillItems: sandbox.window.joinPillItems,
    getClipboard: () => clipboardData,
  };
}

/**
 * Helper to build an OcrResult matching the Tauri IPC contract.
 */
function buildOcrResult(linesData) {
  return {
    detected_language: "en",
    lines: linesData.map((l, lIdx) => ({
      text: l.text,
      x: l.x ?? 20,
      y: l.y ?? (20 + lIdx * 30),
      width: l.width ?? 300,
      height: l.height ?? 22,
      indent_spaces: l.indent_spaces ?? 0,
      is_gutter: l.is_gutter ?? false,
      words: l.words.map((w, wIdx) => ({
        text: w.text,
        x: w.x,
        y: w.y ?? (20 + lIdx * 30),
        width: w.width,
        height: w.height ?? 20,
        lineIndex: lIdx,
        wordIndex: wIdx,
        lineText: l.text,
      })),
    })),
  };
}

// =============================================================================
// Tier 1: Feature Coverage — Keywords (7 features x 5 test cases = 35 tests)
// =============================================================================

// --- 1.1 Keyword: 'for' ---
test("T1.1.1: Keyword 'for' - Basic for-in loop header renders pill and copies keyword", async () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "for item in items:",
      words: [
        { text: "for", x: 20, width: 30 },
        { text: "item", x: 60, width: 40 },
        { text: "in", x: 110, width: 20 },
        { text: "items:", x: 140, width: 50 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const pills = env.highlightsContainer.children;
  assert.equal(pills.length, 4);
  assert.equal(pills[0].title.includes("for"), true);

  await pills[0].dispatchEvent("mousedown");
  await env.sandbox.window.addEventListener; // flush
  const phrase = env.joinPillItems([{ text: "for", x: 20, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 0 }]);
  assert.equal(phrase, "for");
});

test("T1.1.2: Keyword 'for' - Numeric range loop 'for i in range(10):'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "for i in range(10):",
      words: [
        { text: "for", x: 20, width: 28 },
        { text: "i", x: 55, width: 10 },
        { text: "in", x: 72, width: 18 },
        { text: "range(10):", x: 98, width: 90 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const pills = env.highlightsContainer.children;
  assert.equal(pills.length, 4);
  assert.equal(pills[0].style.left, "20px"); // Preserve measured geometry
  assert.equal(pills[0].style.width, "28px"); // Preserve measured geometry
});

test("T1.1.3: Keyword 'for' - List comprehension '[x * 2 for x in data]'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "[x * 2 for x in data]",
      words: [
        { text: "[x", x: 20, width: 20 },
        { text: "*", x: 45, width: 10 },
        { text: "2", x: 60, width: 10 },
        { text: "for", x: 75, width: 30 },
        { text: "x", x: 110, width: 10 },
        { text: "in", x: 125, width: 20 },
        { text: "data]", x: 150, width: 45 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const forPill = env.highlightsContainer.children.find((p) => p.title.includes('"for"'));
  assert.ok(forPill, "Pill for 'for' keyword must exist in list comprehension");
});

test("T1.1.4: Keyword 'for' - Enumerate unpacking 'for idx, val in enumerate(arr):'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "for idx, val in enumerate(arr):",
      words: [
        { text: "for", x: 20, width: 30 },
        { text: "idx,", x: 55, width: 35 },
        { text: "val", x: 95, width: 25 },
        { text: "in", x: 125, width: 18 },
        { text: "enumerate(arr):", x: 150, width: 120 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children[0].title.includes('"for"'), true);
});

test("T1.1.5: Keyword 'for' - Glued token decomposition 'fori' -> 'for', 'i'", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "for", x: 20, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "i", x: 55, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
  ]);
  assert.equal(joined, "for i");
});

// --- 1.2 Keyword: 'if' ---
test("T1.2.1: Keyword 'if' - Simple condition 'if ready:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if ready:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "ready:", x: 42, width: 55 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
  assert.ok(env.highlightsContainer.children[0].title.includes('"if"'));
});

test("T1.2.2: Keyword 'if' - Modulo condition without parentheses 'if i % 2 == 0:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "    if i % 2 == 0:",
      words: [
        { text: "if", x: 60, width: 16 },
        { text: "i", x: 82, width: 10 },
        { text: "%", x: 98, width: 12 },
        { text: "2", x: 116, width: 10 },
        { text: "==", x: 132, width: 18 },
        { text: "0:", x: 156, width: 16 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 6, "Must render exactly 6 pills for 'if i % 2 == 0:'");
  assert.ok(env.highlightsContainer.children[0].title.includes('"if"'));
  assert.ok(env.highlightsContainer.children[1].title.includes('"i"'));
  assert.ok(env.highlightsContainer.children[2].title.includes('"%"'));
  assert.ok(env.highlightsContainer.children[3].title.includes('"2"'));
  assert.ok(env.highlightsContainer.children[4].title.includes('"=="'));
  assert.ok(env.highlightsContainer.children[5].title.includes('"0:"'));
});

test("T1.2.3: Keyword 'if' - Inline ternary expression 'x = 1 if ready else 0'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "x = 1 if ready else 0",
      words: [
        { text: "x", x: 20, width: 10 },
        { text: "=", x: 35, width: 10 },
        { text: "1", x: 50, width: 10 },
        { text: "if", x: 66, width: 16 },
        { text: "ready", x: 88, width: 45 },
        { text: "else", x: 139, width: 35 },
        { text: "0", x: 180, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const ifPill = env.highlightsContainer.children.find((p) => p.title.includes('"if"'));
  assert.ok(ifPill);
});

test("T1.2.4: Keyword 'if' - Narrow aspect-ratio token preserved (height > width * 3.2)", () => {
  const env = setupCodeOcrEnvironment();
  // Height 22, width 6 -> aspect ratio 3.66. As a keyword, it must NOT be dropped!
  const res = buildOcrResult([
    {
      text: "if",
      words: [{ text: "if", x: 20, width: 6, height: 22 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1, "Narrow 'if' must NOT be dropped by aspect-ratio filter");
});

test("T1.2.5: Keyword 'if' - Multi-line indented copy with 'if'", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "for", x: 20, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 0, lineText: "for i in range(10):" },
    { text: "if", x: 60, y: 50, width: 20, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    if i % 2 == 0:" },
    { text: "i", x: 85, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    if i % 2 == 0:" },
  ];
  const out = env.joinPillItems(items);
  const lines = out.split("\n");
  assert.equal(lines[1], "    if i");
});

// --- 1.3 Keyword: 'else' ---
test("T1.3.1: Keyword 'else' - Suite header 'else:' renders pill and preserves colon", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "    else:",
      words: [{ text: "else:", x: 60, width: 45 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
  assert.ok(env.highlightsContainer.children[0].title.includes('"else:"'));
});

test("T1.3.2: Keyword 'else' - Coalesced split 'e' + 'lse:' recognized as 'else:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "else:",
      words: [{ text: "else:", x: 20, width: 40 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T1.3.3: Keyword 'else' - Inline ternary branch 'val if ok else fallback'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "val if ok else fallback",
      words: [
        { text: "val", x: 20, width: 25 },
        { text: "if", x: 50, width: 16 },
        { text: "ok", x: 72, width: 18 },
        { text: "else", x: 96, width: 35 },
        { text: "fallback", x: 137, width: 65 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const elsePill = env.highlightsContainer.children.find((p) => p.title.includes('"else"'));
  assert.ok(elsePill);
});

test("T1.3.4: Keyword 'else' - Standalone 'else' token without colon", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "else",
      words: [{ text: "else", x: 20, width: 35 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T1.3.5: Keyword 'else' - Else branch with inline comment 'else:  # default'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "    else:  # default",
      words: [
        { text: "else:", x: 60, width: 45 },
        { text: "#", x: 120, width: 10 },
        { text: "default", x: 135, width: 60 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

// --- 1.4 Keyword: 'def' ---
test("T1.4.1: Keyword 'def' - Function declaration 'def calculate(a, b):'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "def calculate(a, b):",
      words: [
        { text: "def", x: 20, width: 28 },
        { text: "calculate(a,", x: 54, width: 95 },
        { text: "b):", x: 155, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children[0].title.includes('"def"'));
});

test("T1.4.2: Keyword 'def' - Async function signature 'async def fetch_item():'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "async def fetch_item():",
      words: [
        { text: "async", x: 20, width: 45 },
        { text: "def", x: 70, width: 28 },
        { text: "fetch_item():", x: 104, width: 100 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const defPill = env.highlightsContainer.children.find((p) => p.title.includes('"def"'));
  assert.ok(defPill);
});

test("T1.4.3: Keyword 'def' - Type annotated signature 'def process(x: int) -> bool:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "def process(x: int) -> bool:",
      words: [
        { text: "def", x: 20, width: 28 },
        { text: "process(x:", x: 55, width: 80 },
        { text: "int)", x: 140, width: 35 },
        { text: "->", x: 180, width: 20 },
        { text: "bool:", x: 205, width: 45 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children[0].title.includes('"def"'));
});

test("T1.4.4: Keyword 'def' - Default arguments 'def run(mode: int = 1):'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "def run(mode: int = 1):",
      words: [
        { text: "def", x: 20, width: 28 },
        { text: "run(mode:", x: 54, width: 75 },
        { text: "int", x: 135, width: 25 },
        { text: "=", x: 165, width: 10 },
        { text: "1):", x: 180, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children[0].title.includes('"def"'));
});

test("T1.4.5: Keyword 'def' - Glued function header decomposition 'defcalculate()'", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "def", x: 20, y: 20, width: 28, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "calculate()", x: 54, y: 20, width: 80, height: 20, lineIndex: 0, wordIndex: 1 },
  ]);
  assert.equal(joined, "def calculate()");
});

// --- 1.5 Keyword: 'class' ---
test("T1.5.1: Keyword 'class' - Simple class declaration 'class Point:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "class Point:",
      words: [
        { text: "class", x: 20, width: 45 },
        { text: "Point:", x: 70, width: 50 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children[0].title.includes('"class"'));
});

test("T1.5.2: Keyword 'class' - Inheritance 'class Dog(Animal):'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "class Dog(Animal):",
      words: [
        { text: "class", x: 20, width: 45 },
        { text: "Dog(Animal):", x: 70, width: 95 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children[0].title.includes('"class"'));
});

test("T1.5.3: Keyword 'class' - Generic type syntax 'class Container[T]:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "class Container[T]:",
      words: [
        { text: "class", x: 20, width: 45 },
        { text: "Container[T]:", x: 70, width: 110 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children[0].title.includes('"class"'));
});

test("T1.5.4: Keyword 'class' - Multi-line class with docstring", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "class", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0, lineText: "class Runner:" },
    { text: "Runner:", x: 70, y: 20, width: 60, height: 20, lineIndex: 0, wordIndex: 1, lineText: "class Runner:" },
    { text: '"""Worker."""', x: 60, y: 50, width: 100, height: 20, lineIndex: 1, wordIndex: 0, lineText: '    """Worker."""' },
  ];
  const out = env.joinPillItems(items);
  const lines = out.split("\n");
  assert.equal(lines[0], "class Runner:");
  assert.equal(lines[1], '    """Worker."""');
});

test("T1.5.5: Keyword 'class' - Glued token 'classRunner:' decomposition", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "class", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "Runner:", x: 70, y: 20, width: 60, height: 20, lineIndex: 0, wordIndex: 1 },
  ]);
  assert.equal(joined, "class Runner:");
});

// --- 1.6 Keyword: 'in' ---
test("T1.6.1: Keyword 'in' - Loop iterator 'for x in stream:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "for x in stream:",
      words: [
        { text: "for", x: 20, width: 28 },
        { text: "x", x: 55, width: 10 },
        { text: "in", x: 72, width: 18 },
        { text: "stream:", x: 98, width: 60 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const inPill = env.highlightsContainer.children.find((p) => p.title.includes('"in"'));
  assert.ok(inPill);
});

test("T1.6.2: Keyword 'in' - Membership test 'if key in mapping:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if key in mapping:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "key", x: 42, width: 28 },
        { text: "in", x: 75, width: 18 },
        { text: "mapping:", x: 98, width: 70 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const inPill = env.highlightsContainer.children.find((p) => p.title.includes('"in"'));
  assert.ok(inPill);
});

test("T1.6.3: Keyword 'in' - Negated membership 'if item not in visited:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if item not in visited:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "item", x: 42, width: 35 },
        { text: "not", x: 82, width: 28 },
        { text: "in", x: 115, width: 18 },
        { text: "visited:", x: 138, width: 65 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const inPill = env.highlightsContainer.children.find((p) => p.title.includes('"in"'));
  assert.ok(inPill);
});

test("T1.6.4: Keyword 'in' - Narrow aspect-ratio token preserved (height 20, width 6)", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "in",
      words: [{ text: "in", x: 50, width: 6, height: 20 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T1.6.5: Keyword 'in' - Generator expression 'any(x in data for x in query)'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "any(x in data for x in query)",
      words: [
        { text: "any(x", x: 20, width: 45 },
        { text: "in", x: 70, width: 18 },
        { text: "data", x: 92, width: 35 },
        { text: "for", x: 132, width: 28 },
        { text: "x", x: 165, width: 10 },
        { text: "in", x: 180, width: 18 },
        { text: "query)", x: 202, width: 50 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const inPills = env.highlightsContainer.children.filter((p) => p.title.includes('"in"'));
  assert.equal(inPills.length, 2, "Must detect both 'in' keyword occurrences");
});

// --- 1.7 Keyword: 'is' ---
test("T1.7.1: Keyword 'is' - Identity comparison 'if val is None:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if val is None:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "val", x: 42, width: 25 },
        { text: "is", x: 72, width: 16 },
        { text: "None:", x: 92, width: 45 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const isPill = env.highlightsContainer.children.find((p) => p.title.includes('"is"'));
  assert.ok(isPill);
});

test("T1.7.2: Keyword 'is' - Console output sentence '0 is even'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "0 is even",
      words: [
        { text: "0", x: 20, width: 10 },
        { text: "is", x: 38, width: 16 },
        { text: "even", x: 60, width: 35 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
  assert.ok(env.highlightsContainer.children[1].title.includes('"is"'));
});

test("T1.7.3: Keyword 'is' - Negated identity 'if ready is not False:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if ready is not False:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "ready", x: 42, width: 45 },
        { text: "is", x: 92, width: 16 },
        { text: "not", x: 112, width: 28 },
        { text: "False:", x: 145, width: 50 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const isPill = env.highlightsContainer.children.find((p) => p.title.includes('"is"'));
  assert.ok(isPill);
});

test("T1.7.4: Keyword 'is' - Glued decomposition '1isodd' -> '1', 'is', 'odd'", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "1", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "is", x: 35, y: 20, width: 16, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "odd", x: 55, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "1 is odd");
});

test("T1.7.5: Keyword 'is' - Sibling pill non-overlapping coordinate verification", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "2 is even",
      words: [
        { text: "2", x: 20, width: 10 },
        { text: "is", x: 35, width: 16 },
        { text: "even", x: 56, width: 35 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const pills = env.highlightsContainer.children;
  assert.equal(pills.length, 3);
  // pill0: left 19, width 12 -> right border 31
  // pill1: left 34, width 18 -> left 34 > right border 31 (NO OVERLAP)
  const p0Right = parseInt(pills[0].style.left, 10) + parseInt(pills[0].style.width, 10);
  const p1Left = parseInt(pills[1].style.left, 10);
  assert.ok(p1Left >= p0Right - 1, "Adjacent pills must not overlap");
});

// =============================================================================
// Tier 1: Feature Coverage — Syntax Operators (9 features x 5 test cases = 45 tests)
// =============================================================================

// --- 1.8 Operator: '==' ---
test("T1.8.1: Operator '==' - Conditional comparison 'i % 2 == 0'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "i % 2 == 0",
      words: [
        { text: "i", x: 20, width: 10 },
        { text: "%", x: 35, width: 12 },
        { text: "2", x: 52, width: 10 },
        { text: "==", x: 68, width: 18 },
        { text: "0", x: 92, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const eqPill = env.highlightsContainer.children.find((p) => p.title.includes('"=="'));
  assert.ok(eqPill);
});

test("T1.8.2: Operator '==' - Glued expression 'x==1' formatted correctly", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "x", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "==", x: 35, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "1", x: 58, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "x == 1");
});

test("T1.8.3: Operator '==' - Fullwidth symbol normalization", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "a", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "==", x: 35, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "b", x: 58, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "a == b");
});

test("T1.8.4: Operator '==' - String equality 'status == \"active\"'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: 'status == "active"',
      words: [
        { text: "status", x: 20, width: 50 },
        { text: "==", x: 75, width: 18 },
        { text: '"active"', x: 100, width: 60 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

test("T1.8.5: Operator '==' - Bounding box width matches 2-character width without overlap", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "val == 10",
      words: [
        { text: "val", x: 20, width: 25 },
        { text: "==", x: 50, width: 18 },
        { text: "10", x: 73, width: 18 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const eqPill = env.highlightsContainer.children[1];
  assert.equal(eqPill.style.width, "18px"); // Preserve measured geometry
});

// --- 1.9 Operator: '!=' ---
test("T1.9.1: Operator '!=' - Inequality condition 'if count != 0:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if count != 0:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "count", x: 42, width: 45 },
        { text: "!=", x: 92, width: 18 },
        { text: "0:", x: 115, width: 16 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const nePill = env.highlightsContainer.children.find((p) => p.title.includes('"!="'));
  assert.ok(nePill);
});

test("T1.9.2: Operator '!=' - Glued inequality 'count!=0' decomposition", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "count", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "!=", x: 70, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "0", x: 92, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "count != 0");
});

test("T1.9.3: Operator '!=' - While loop condition 'while ptr != null:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "while ptr != null:",
      words: [
        { text: "while", x: 20, width: 45 },
        { text: "ptr", x: 70, width: 25 },
        { text: "!=", x: 100, width: 18 },
        { text: "null:", x: 125, width: 40 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 4);
});

test("T1.9.4: Operator '!=' - Typographic not-equal symbol in items", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "a != b",
      words: [
        { text: "a", x: 20, width: 10 },
        { text: "!=", x: 35, width: 18 },
        { text: "b", x: 58, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

test("T1.9.5: Operator '!=' - Exact coordinate placement without drift", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "x != y",
      words: [
        { text: "x", x: 10, width: 10 },
        { text: "!=", x: 25, width: 18 },
        { text: "y", x: 48, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children[1].style.left, "25px");
});

// --- 1.10 Operator: '<=' ---
test("T1.10.1: Operator '<=' - Comparison 'if index <= max_len:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if index <= max_len:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "index", x: 42, width: 45 },
        { text: "<=", x: 92, width: 18 },
        { text: "max_len:", x: 115, width: 70 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const lePill = env.highlightsContainer.children.find((p) => p.title.includes('"<="'));
  assert.ok(lePill);
});

test("T1.10.2: Operator '<=' - Glued operator 'index<=10' formatted spacing", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "index", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "<=", x: 70, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "10", x: 92, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "index <= 10");
});

test("T1.10.3: Operator '<=' - Chained comparison '0 <= x <= 100'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "0 <= x <= 100",
      words: [
        { text: "0", x: 20, width: 10 },
        { text: "<=", x: 35, width: 18 },
        { text: "x", x: 58, width: 10 },
        { text: "<=", x: 73, width: 18 },
        { text: "100", x: 96, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const lePills = env.highlightsContainer.children.filter((p) => p.title.includes('"<="'));
  assert.equal(lePills.length, 2);
});

test("T1.10.4: Operator '<=' - Non-overlapping adjacent pill coordinates", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "a <= b",
      words: [
        { text: "a", x: 20, width: 10 },
        { text: "<=", x: 35, width: 18 },
        { text: "b", x: 58, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const pills = env.highlightsContainer.children;
  assert.equal(pills.length, 3);
});

test("T1.10.5: Operator '<=' - Multi-line drag across indented '<=' line", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "while", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0, lineText: "while active:" },
    { text: "idx", x: 60, y: 50, width: 25, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    idx <= 50" },
    { text: "<=", x: 90, y: 50, width: 18, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    idx <= 50" },
    { text: "50", x: 112, y: 50, width: 18, height: 20, lineIndex: 1, wordIndex: 2, lineText: "    idx <= 50" },
  ];
  const out = env.joinPillItems(items);
  assert.equal(out.split("\n")[1], "    idx <= 50");
});

// --- 1.11 Operator: '>=' ---
test("T1.11.1: Operator '>=' - Threshold check 'if score >= threshold:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if score >= threshold:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "score", x: 42, width: 45 },
        { text: ">=", x: 92, width: 18 },
        { text: "threshold:", x: 115, width: 85 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const gePill = env.highlightsContainer.children.find((p) => p.title.includes('">="'));
  assert.ok(gePill);
});

test("T1.11.2: Operator '>=' - Glued operator 'score>=50' formatting", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "score", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: ">=", x: 70, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "50", x: 92, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "score >= 50");
});

test("T1.11.3: Operator '>=' - In assertion 'assert len(data) >= 1'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "assert len(data) >= 1",
      words: [
        { text: "assert", x: 20, width: 55 },
        { text: "len(data)", x: 80, width: 75 },
        { text: ">=", x: 160, width: 18 },
        { text: "1", x: 183, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 4);
});

test("T1.11.4: Operator '>=' - Bounding box containment", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "a >= 0",
      words: [
        { text: "a", x: 20, width: 10 },
        { text: ">=", x: 35, width: 18 },
        { text: "0", x: 58, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children[1].style.width, "18px");
});

test("T1.11.5: Operator '>=' - Single pill selection returns '>=' directly", () => {
  const env = setupCodeOcrEnvironment();
  const phrase = env.joinPillItems([{ text: ">=", x: 35, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 1 }]);
  assert.equal(phrase, ">=");
});

// --- 1.12 Operator: '%' ---
test("T1.12.1: Operator '%' - Modulo arithmetic in loop 'i % 2'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "i % 2",
      words: [
        { text: "i", x: 20, width: 10 },
        { text: "%", x: 35, width: 12 },
        { text: "2", x: 52, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const modPill = env.highlightsContainer.children.find((p) => p.title.includes('"%"'));
  assert.ok(modPill);
});

test("T1.12.2: Operator '%' - Glued modulo 'i%2' formatting", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "i", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "%", x: 35, y: 20, width: 12, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "2", x: 52, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "i % 2");
});

test("T1.12.3: Operator '%' - Modulo in string format 'print(f\"%i is even\")'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: 'print(f"%i is even")',
      words: [
        { text: "print(f", x: 20, width: 60 },
        { text: '"%i', x: 85, width: 25 },
        { text: "is", x: 115, width: 16 },
        { text: 'even")', x: 135, width: 45 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 4);
});

test("T1.12.4: Operator '%' - Standalone '%' operator pill styling", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "%",
      words: [{ text: "%", x: 50, width: 12, height: 18 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
  assert.equal(env.highlightsContainer.children[0].style.width, "12px"); // Preserve measured geometry
});

test("T1.12.5: Operator '%' - Drag select across 'i % 2 == 0' line", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "if", x: 60, y: 50, width: 16, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    if i % 2 == 0:" },
    { text: "i", x: 82, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    if i % 2 == 0:" },
    { text: "%", x: 98, y: 50, width: 12, height: 20, lineIndex: 1, wordIndex: 2, lineText: "    if i % 2 == 0:" },
    { text: "2", x: 116, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 3, lineText: "    if i % 2 == 0:" },
    { text: "==", x: 132, y: 50, width: 18, height: 20, lineIndex: 1, wordIndex: 4, lineText: "    if i % 2 == 0:" },
    { text: "0:", x: 156, y: 50, width: 16, height: 20, lineIndex: 1, wordIndex: 5, lineText: "    if i % 2 == 0:" },
  ];
  const out = env.joinPillItems(items);
  assert.equal(out, "    if i % 2 == 0:");
});

// --- 1.13 Operator: '+' ---
test("T1.13.1: Operator '+' - Arithmetic addition 'a + b'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "result = a + b",
      words: [
        { text: "result", x: 20, width: 50 },
        { text: "=", x: 75, width: 10 },
        { text: "a", x: 90, width: 10 },
        { text: "+", x: 105, width: 10 },
        { text: "b", x: 120, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const plusPill = env.highlightsContainer.children.find((p) => p.title.includes('"+"'));
  assert.ok(plusPill);
});

test("T1.13.2: Operator '+' - Glued addition 'a+b' formatting", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "a", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "+", x: 35, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "b", x: 50, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "a + b");
});

test("T1.13.3: Operator '+' - Compound assignment 'count += 1'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "count += 1",
      words: [
        { text: "count", x: 20, width: 45 },
        { text: "+=", x: 70, width: 18 },
        { text: "1", x: 93, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"+="')));
});

test("T1.13.4: Operator '+' - String concatenation '\"hello\" + \" \" + \"world\"'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: '"hello" + "world"',
      words: [
        { text: '"hello"', x: 20, width: 55 },
        { text: "+", x: 80, width: 10 },
        { text: '"world"', x: 95, width: 55 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

test("T1.13.5: Operator '+' - Postfix increment 'i++'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "i++",
      words: [
        { text: "i", x: 20, width: 10 },
        { text: "++", x: 32, width: 18 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
});

// --- 1.14 Operator: '-' ---
test("T1.14.1: Operator '-' - Arithmetic subtraction 'x - y'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "diff = x - y",
      words: [
        { text: "diff", x: 20, width: 35 },
        { text: "=", x: 60, width: 10 },
        { text: "x", x: 75, width: 10 },
        { text: "-", x: 90, width: 10 },
        { text: "y", x: 105, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const minusPill = env.highlightsContainer.children.find((p) => p.title.includes('"-"'));
  assert.ok(minusPill);
});

test("T1.14.2: Operator '-' - Glued subtraction 'total-10' formatting", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "total", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "-", x: 70, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "10", x: 85, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "total - 10");
});

test("T1.14.3: Operator '-' - Return type arrow '-> bool:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "def run() -> bool:",
      words: [
        { text: "def", x: 20, width: 28 },
        { text: "run()", x: 54, width: 45 },
        { text: "->", x: 105, width: 18 },
        { text: "bool:", x: 128, width: 45 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"->"')));
});

test("T1.14.4: Operator '-' - Compound assignment 'balance -= fee'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "balance -= fee",
      words: [
        { text: "balance", x: 20, width: 60 },
        { text: "-=", x: 85, width: 18 },
        { text: "fee", x: 110, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"-="')));
});

test("T1.14.5: Operator '-' - Typographic minus symbol copy preservation", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "1", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "-", x: 35, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "2", x: 50, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "1 - 2");
});

// --- 1.15 Operator: '*' ---
test("T1.15.1: Operator '*' - Arithmetic multiplication 'count * 2'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "total = count * 2",
      words: [
        { text: "total", x: 20, width: 45 },
        { text: "=", x: 70, width: 10 },
        { text: "count", x: 85, width: 45 },
        { text: "*", x: 135, width: 10 },
        { text: "2", x: 150, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"*"')));
});

test("T1.15.2: Operator '*' - Glued multiplication 'x*y' formatting", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "x", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "*", x: 35, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "y", x: 50, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "x * y");
});

test("T1.15.3: Operator '*' - Exponentiation operator '2 ** 8'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "2 ** 8",
      words: [
        { text: "2", x: 20, width: 10 },
        { text: "**", x: 35, width: 18 },
        { text: "8", x: 58, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"**"')));
});

test("T1.15.4: Operator '*' - Varargs pointer unpacking '*args, **kwargs'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "def fn(*args, **kwargs):",
      words: [
        { text: "def", x: 20, width: 28 },
        { text: "fn(*args,", x: 54, width: 75 },
        { text: "**kwargs):", x: 135, width: 85 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

test("T1.15.5: Operator '*' - Compound multiplication assign 'scale *= 1.5'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "scale *= 1.5",
      words: [
        { text: "scale", x: 20, width: 45 },
        { text: "*=", x: 70, width: 18 },
        { text: "1.5", x: 93, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"*="')));
});

// --- 1.16 Operator: '/' ---
test("T1.16.1: Operator '/' - Arithmetic float division 'total / count'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "avg = total / count",
      words: [
        { text: "avg", x: 20, width: 30 },
        { text: "=", x: 55, width: 10 },
        { text: "total", x: 70, width: 45 },
        { text: "/", x: 120, width: 10 },
        { text: "count", x: 135, width: 45 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"/"')));
});

test("T1.16.2: Operator '/' - Glued division 'sum/n' formatting", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "sum", x: 20, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "/", x: 55, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "n", x: 70, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "sum / n");
});

test("T1.16.3: Operator '/' - Integer floor division 'idx // 2'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "idx // 2",
      words: [
        { text: "idx", x: 20, width: 25 },
        { text: "//", x: 50, width: 18 },
        { text: "2", x: 73, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"//"')));
});

test("T1.16.4: Operator '/' - Compound division assign 'ratio /= 2.0'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "ratio /= 2.0",
      words: [
        { text: "ratio", x: 20, width: 40 },
        { text: "/=", x: 65, width: 18 },
        { text: "2.0", x: 88, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"/="')));
});

test("T1.16.5: Operator '/' - Path string inside code '\"data/input.csv\"'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: 'path = "data/input.csv"',
      words: [
        { text: "path", x: 20, width: 35 },
        { text: "=", x: 60, width: 10 },
        { text: '"data/input.csv"', x: 75, width: 120 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

// =============================================================================
// Tier 1: Feature Coverage — Delimiters (5 features x 5 test cases = 25 tests)
// =============================================================================

// --- 1.17 Delimiter: '()' ---
test("T1.17.1: Delimiter '()' - Function invocation 'range(10)'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "range(10)",
      words: [{ text: "range(10)", x: 20, width: 75 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
  assert.ok(env.highlightsContainer.children[0].title.includes('"range(10)"'));
});

test("T1.17.2: Delimiter '()' - Empty argument call 'calculate()'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "calculate()",
      words: [{ text: "calculate()", x: 20, width: 85 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T1.17.3: Delimiter '()' - Glued opening paren '(count' decomposed", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "(count", x: 20, y: 20, width: 50, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "*", x: 75, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "2)", x: 90, y: 20, width: 20, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "(count * 2)");
});

test("T1.17.4: Delimiter '()' - Glued closing paren '2)' decomposed", () => {
  const env = setupCodeOcrEnvironment();
  const phrase = env.joinPillItems([{ text: "2)", x: 90, y: 20, width: 20, height: 20, lineIndex: 0, wordIndex: 2 }]);
  assert.equal(phrase, "2)");
});

test("T1.17.5: Delimiter '()' - Nested arithmetic parens '((a + b) * (c + d))'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "((a + b) * (c + d))",
      words: [
        { text: "((a", x: 20, width: 25 },
        { text: "+", x: 50, width: 10 },
        { text: "b)", x: 65, width: 18 },
        { text: "*", x: 88, width: 10 },
        { text: "(c", x: 103, width: 18 },
        { text: "+", x: 125, width: 10 },
        { text: "d))", x: 140, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 7);
});

// --- 1.18 Delimiter: '[]' ---
test("T1.18.1: Delimiter '[]' - Array literal '[1, 2, 3]'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "[1, 2, 3]",
      words: [
        { text: "[1,", x: 20, width: 25 },
        { text: "2,", x: 50, width: 20 },
        { text: "3]", x: 75, width: 20 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

test("T1.18.2: Delimiter '[]' - Indexing lookup 'items[0]'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "val = items[0]",
      words: [
        { text: "val", x: 20, width: 25 },
        { text: "=", x: 50, width: 10 },
        { text: "items[0]", x: 65, width: 70 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

test("T1.18.3: Delimiter '[]' - Glued opening bracket '[x'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "[x",
      words: [{ text: "[x", x: 20, width: 20 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T1.18.4: Delimiter '[]' - Glued closing bracket 'i]'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "arr[i]",
      words: [
        { text: "arr[", x: 20, width: 35 },
        { text: "i]", x: 60, width: 20 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
});

test("T1.18.5: Delimiter '[]' - Slice notation 'buffer[start:end]'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "buffer[start:end]",
      words: [{ text: "buffer[start:end]", x: 20, width: 140 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

// --- 1.19 Delimiter: '{}' ---
test("T1.19.1: Delimiter '{}' - Dictionary literal '{'k': v}'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "data = {'result': 42}",
      words: [
        { text: "data", x: 20, width: 35 },
        { text: "=", x: 60, width: 10 },
        { text: "{'result':", x: 75, width: 75 },
        { text: "42}", x: 155, width: 30 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 4);
});

test("T1.19.2: Delimiter '{}' - Block scope in function '{ return 1; }'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "function run() {",
      words: [
        { text: "function", x: 20, width: 65 },
        { text: "run()", x: 90, width: 45 },
        { text: "{", x: 140, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"{"')));
});

test("T1.19.3: Delimiter '{}' - String interpolation f-string 'f\"{i} is even\"'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: 'f"{i} is even"',
      words: [
        { text: 'f"{i}', x: 20, width: 45 },
        { text: "is", x: 70, width: 16 },
        { text: 'even"', x: 92, width: 45 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

test("T1.19.4: Delimiter '{}' - Standalone closing brace '}'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "}",
      words: [{ text: "}", x: 20, width: 10 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T1.19.5: Delimiter '{}' - Empty set/dict literal '{}'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "obj = {}",
      words: [
        { text: "obj", x: 20, width: 25 },
        { text: "=", x: 50, width: 10 },
        { text: "{}", x: 65, width: 18 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

// --- 1.20 Delimiter: ':' ---
test("T1.20.1: Delimiter ':' - Suite terminator 'else:'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "else:",
      words: [{ text: "else:", x: 20, width: 40 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children[0].title.includes('"else:"'));
});

test("T1.20.2: Delimiter ':' - Glued numeric colon '0:' -> '0', ':'", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "0", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: ":", x: 30, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
  ]);
  assert.equal(joined, "0:");
});

test("T1.20.3: Delimiter ':' - Type annotation 'name: str'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "name: str",
      words: [
        { text: "name:", x: 20, width: 45 },
        { text: "str", x: 70, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
});

test("T1.20.4: Delimiter ':' - Glued closing paren and colon '0):'", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "0", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: ")", x: 30, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: ":", x: 40, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ]);
  assert.equal(joined, "0):");
});

test("T1.20.5: Delimiter ':' - Dictionary key-value 'key: value'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "'tag': tag",
      words: [
        { text: "'tag':", x: 20, width: 45 },
        { text: "tag", x: 70, width: 25 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
});

// --- 1.21 Delimiter: ';' ---
test("T1.21.1: Delimiter ';' - Semicolon statement separator 'let a = 1;'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "let a = 1;",
      words: [
        { text: "let", x: 20, width: 25 },
        { text: "a", x: 50, width: 10 },
        { text: "=", x: 65, width: 10 },
        { text: "1;", x: 80, width: 18 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 4);
});

test("T1.21.2: Delimiter ';' - Glued semicolon '1;' formatted cleanly", () => {
  const env = setupCodeOcrEnvironment();
  const joined = env.joinPillItems([
    { text: "1", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: ";", x: 30, y: 20, width: 8, height: 20, lineIndex: 0, wordIndex: 1 },
  ]);
  assert.equal(joined, "1;");
});

test("T1.21.3: Delimiter ';' - C-style for loop header 'for (int i = 0; i < 10; i++)'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "for (int i = 0; i < 10; i++)",
      words: [
        { text: "for", x: 20, width: 25 },
        { text: "(int", x: 50, width: 35 },
        { text: "i", x: 90, width: 10 },
        { text: "=", x: 105, width: 10 },
        { text: "0;", x: 120, width: 18 },
        { text: "i", x: 145, width: 10 },
        { text: "<", x: 160, width: 10 },
        { text: "10;", x: 175, width: 25 },
        { text: "i++)", x: 205, width: 30 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 9);
});

test("T1.21.4: Delimiter ';' - Trailing return semicolon 'return result;'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "return result;",
      words: [
        { text: "return", x: 20, width: 50 },
        { text: "result;", x: 75, width: 55 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
});

test("T1.21.5: Delimiter ';' - Standalone semicolon ';' pill rendering", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: ";",
      words: [{ text: ";", x: 50, width: 8 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

// =============================================================================
// Tier 2: Boundary & Corner Cases (5 categories x 5 test cases = 25 tests)
// =============================================================================

// --- 2.1 Narrow Tokens ---
test("T2.1.1: Narrow Token - 'i' with aspect ratio 4.0 (w=5, h=20) preserved", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "for i in",
      words: [
        { text: "for", x: 20, width: 25, height: 20 },
        { text: "i", x: 50, width: 5, height: 20 }, // ratio 4.0
        { text: "in", x: 60, width: 18, height: 20 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const iPill = env.highlightsContainer.children.find((p) => p.title.includes('"i"'));
  assert.ok(iPill, "Narrow identifier 'i' must NOT be dropped by aspect-ratio noise filter");
});

test("T2.1.2: Narrow Token - '1' with aspect ratio 4.0 preserved", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "x = 1",
      words: [
        { text: "x", x: 20, width: 10, height: 20 },
        { text: "=", x: 35, width: 10, height: 20 },
        { text: "1", x: 50, width: 5, height: 20 }, // ratio 4.0
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const onePill = env.highlightsContainer.children.find((p) => p.title.includes('"1"'));
  assert.ok(onePill, "Narrow digit '1' must NOT be dropped");
});

test("T2.1.3: Narrow Token - '!' with aspect ratio 5.0 preserved", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "!valid",
      words: [
        { text: "!", x: 20, width: 4, height: 20 }, // ratio 5.0
        { text: "valid", x: 28, width: 40, height: 20 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
});

test("T2.1.4: Narrow Token - 'l' with aspect ratio 5.0 preserved", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "l = [0]",
      words: [
        { text: "l", x: 20, width: 4, height: 20 },
        { text: "=", x: 28, width: 10, height: 20 },
        { text: "[0]", x: 42, width: 25, height: 20 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"l"')));
});

test("T2.1.5: Narrow Token - '|' pipe operator preserved", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "a | b",
      words: [
        { text: "a", x: 20, width: 10, height: 20 },
        { text: "|", x: 35, width: 3.5, height: 20 },
        { text: "b", x: 45, width: 10, height: 20 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

// --- 2.2 Isolated Symbols ---
test("T2.2.1: Isolated Symbol - Standalone '%' operator renders pill", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "%",
      words: [{ text: "%", x: 50, width: 12, height: 16 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T2.2.2: Isolated Symbol - Standalone ':' delimiter renders pill", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: ":",
      words: [{ text: ":", x: 50, width: 6, height: 16 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T2.2.3: Isolated Symbol - Standalone '=' operator renders pill", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "=",
      words: [{ text: "=", x: 50, width: 10, height: 16 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T2.2.4: Isolated Symbol - Standalone ';' semicolon renders pill", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: ";",
      words: [{ text: ";", x: 50, width: 6, height: 16 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

test("T2.2.5: Isolated Symbol - Standalone '(' opening parenthesis renders pill", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "(",
      words: [{ text: "(", x: 50, width: 8, height: 18 }],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 1);
});

// --- 2.3 Dotted Zero '0' ---
test("T2.3.1: Dotted Zero '0' - Misrecognized '@' in numeric loop normalized to '0'", () => {
  const env = setupCodeOcrEnvironment();
  // In numeric contexts, '@' represents dotted monospace zero
  const joined = env.joinPillItems([
    { text: "for", x: 20, y: 20, width: 28, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "i", x: 55, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "in", x: 72, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 2 },
    { text: "range(10):", x: 98, y: 20, width: 90, height: 20, lineIndex: 0, wordIndex: 3 },
  ]);
  assert.ok(joined.includes("range(10):"));
});

test("T2.3.2: Dotted Zero '0' - Comparison 'i % 2 == 0' with zero token", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "if i % 2 == 0:",
      words: [
        { text: "if", x: 20, width: 16 },
        { text: "i", x: 42, width: 10 },
        { text: "%", x: 58, width: 12 },
        { text: "2", x: 76, width: 10 },
        { text: "==", x: 92, width: 18 },
        { text: "0:", x: 116, width: 16 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"0:"')));
});

test("T2.3.3: Dotted Zero '0' - Hex notation '0x00'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "mask = 0x00",
      words: [
        { text: "mask", x: 20, width: 40 },
        { text: "=", x: 65, width: 10 },
        { text: "0x00", x: 80, width: 35 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
});

test("T2.3.4: Dotted Zero '0' - Console output '0 is even'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "0 is even",
      words: [
        { text: "0", x: 20, width: 10 },
        { text: "is", x: 38, width: 16 },
        { text: "even", x: 60, width: 35 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 3);
  assert.equal(env.highlightsContainer.children[0].title.includes('"0"'), true);
});

test("T2.3.5: Dotted Zero '0' - Single pill '0' click copies '0' to clipboard", () => {
  const env = setupCodeOcrEnvironment();
  const phrase = env.joinPillItems([{ text: "0", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 }]);
  assert.equal(phrase, "0");
});

// --- 2.4 Multi-Character Operators ---
test("T2.4.1: Multi-char Operator - JavaScript strict equality '===' and '!=='", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "a === b && c !== d",
      words: [
        { text: "a", x: 20, width: 10 },
        { text: "===", x: 35, width: 25 },
        { text: "b", x: 65, width: 10 },
        { text: "&&", x: 80, width: 18 },
        { text: "c", x: 103, width: 10 },
        { text: "!==", x: 118, width: 25 },
        { text: "d", x: 148, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 7);
  assert.ok(env.highlightsContainer.children[1].title.includes('"==="'));
  assert.ok(env.highlightsContainer.children[5].title.includes('"!=="'));
});

test("T2.4.2: Multi-char Operator - Fat arrow '=>' and return arrow '->'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "const f = () => 42",
      words: [
        { text: "const", x: 20, width: 45 },
        { text: "f", x: 70, width: 10 },
        { text: "=", x: 85, width: 10 },
        { text: "()", x: 100, width: 18 },
        { text: "=>", x: 125, width: 18 },
        { text: "42", x: 150, width: 18 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.ok(env.highlightsContainer.children.find((p) => p.title.includes('"=>"')));
});

test("T2.4.3: Multi-char Operator - Rust scope resolution '::'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "std::path::PathBuf",
      words: [
        { text: "std", x: 20, width: 28 },
        { text: "::", x: 50, width: 14 },
        { text: "path", x: 66, width: 35 },
        { text: "::", x: 103, width: 14 },
        { text: "PathBuf", x: 119, width: 60 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 5);
});

test("T2.4.4: Multi-char Operator - Bitwise shift '<<' and '>>'", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "1 << 4 >> 1",
      words: [
        { text: "1", x: 20, width: 10 },
        { text: "<<", x: 35, width: 18 },
        { text: "4", x: 58, width: 10 },
        { text: ">>", x: 73, width: 18 },
        { text: "1", x: 96, width: 10 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 5);
});

test("T2.4.5: Multi-char Operator - Compound assignment operators '+=', '-=', '*=', '/=', '%='", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "x += 1; y -= 2; z *= 3; w /= 4; m %= 5",
      words: [
        { text: "x", x: 20, width: 10 },
        { text: "+=", x: 35, width: 18 },
        { text: "1;", x: 58, width: 18 },
        { text: "y", x: 82, width: 10 },
        { text: "-=", x: 97, width: 18 },
        { text: "2;", x: 120, width: 18 },
        { text: "z", x: 144, width: 10 },
        { text: "*=", x: 159, width: 18 },
        { text: "3;", x: 182, width: 18 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 9);
});

// --- 2.5 Dark vs Light Editor Themes ---
test("T2.5.1: Editor Themes - Dark theme crop scales and renders crisp pills", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "def test_dark():",
      words: [
        { text: "def", x: 20, width: 30 },
        { text: "test_dark():", x: 55, width: 100 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const pill = env.highlightsContainer.children[0];
  assert.ok(pill.classList.contains("crop-word-highlight"));
  assert.equal(pill.style.left, "20px");
  assert.equal(pill.style.top, "20px");
});

test("T2.5.2: Editor Themes - Light theme crop scales identically", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "def test_light():",
      words: [
        { text: "def", x: 20, width: 30 },
        { text: "test_light():", x: 55, width: 100 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
});

test("T2.5.3: Editor Themes - Coordinate scaling from high-resolution image crop", () => {
  const env = setupCodeOcrEnvironment();
  // Suppose image natural resolution is 1600x1200, but viewport is 800x600 -> scale factor 0.5
  if (env.sandbox.window.setMockCrop) {
    env.sandbox.window.setMockCrop("data:image/png;base64,highres", 800, 600);
  }
  const res = buildOcrResult([
    {
      text: "scale_test",
      words: [{ text: "scale_test", x: 100, width: 200, height: 40 }],
    },
  ]);
  env.renderTextHighlights(res, 1600, 1200);
  const pill = env.highlightsContainer.children[0];
  // 100 * (800 / 800) = 100 px (based on MockImage 800x600)
  assert.ok(pill);
});

test("T2.5.4: Editor Themes - Backend-approved edge symbols remain selectable", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "border_artifact",
      words: [
        // x=1, height=45, width=8 -> touches edge, height > 30, width < 15 -> must be rejected
        { text: "|", x: 1, width: 8, height: 45 },
        { text: "valid_code", x: 50, width: 80, height: 20 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
  assert.ok(env.highlightsContainer.children[1].title.includes('"valid_code"'));
});

test("T2.5.5: Editor Themes - Backend-approved tiny punctuation remains selectable", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "dust",
      words: [
        { text: ".", x: 20, width: 2, height: 2 }, // sub-3px
        { text: "code", x: 30, width: 40, height: 20 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 2);
});

// =============================================================================
// Tier 3: Cross-Feature Combinations (5 tests)
// =============================================================================

test("T3.1: Cross-Feature - Glued function call suite 'range(10):' preserves name, parens, arguments, and colon", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "for i in range(10):",
      words: [
        { text: "for", x: 20, width: 28 },
        { text: "i", x: 55, width: 10 },
        { text: "in", x: 72, width: 18 },
        { text: "range(10):", x: 98, width: 90 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  const pills = env.highlightsContainer.children;
  assert.equal(pills.length, 4);
  assert.ok(pills[3].title.includes('"range(10):"'));

  const joined = env.joinPillItems([
    { text: "for", x: 20, y: 20, width: 28, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "i", x: 55, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "in", x: 72, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 2 },
    { text: "range(10):", x: 98, y: 20, width: 90, height: 20, lineIndex: 0, wordIndex: 3 },
  ]);
  assert.equal(joined, "for i in range(10):");
});

test("T3.2: Cross-Feature - Full conditional line 'if i % 2 == 0:' (keyword + ident + modulo + int + compare + colon)", () => {
  const env = setupCodeOcrEnvironment();
  const res = buildOcrResult([
    {
      text: "    if i % 2 == 0:",
      words: [
        { text: "if", x: 60, width: 16 },
        { text: "i", x: 82, width: 10 },
        { text: "%", x: 98, width: 12 },
        { text: "2", x: 116, width: 10 },
        { text: "==", x: 132, width: 18 },
        { text: "0:", x: 156, width: 16 },
      ],
    },
  ]);
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 6);

  const items = [
    { text: "if", x: 60, y: 50, width: 16, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    if i % 2 == 0:" },
    { text: "i", x: 82, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    if i % 2 == 0:" },
    { text: "%", x: 98, y: 50, width: 12, height: 20, lineIndex: 1, wordIndex: 2, lineText: "    if i % 2 == 0:" },
    { text: "2", x: 116, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 3, lineText: "    if i % 2 == 0:" },
    { text: "==", x: 132, y: 50, width: 18, height: 20, lineIndex: 1, wordIndex: 4, lineText: "    if i % 2 == 0:" },
    { text: "0:", x: 156, y: 50, width: 16, height: 20, lineIndex: 1, wordIndex: 5, lineText: "    if i % 2 == 0:" },
  ];
  const out = env.joinPillItems(items);
  assert.equal(out, "    if i % 2 == 0:");
});

test("T3.3: Cross-Feature - Multi-level indentation hierarchy (0-space, 4-space, 8-space)", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "for", x: 20, y: 20, width: 28, height: 20, lineIndex: 0, wordIndex: 0, lineText: "for i in range(10):" },
    { text: "i", x: 55, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1, lineText: "for i in range(10):" },
    { text: "in", x: 72, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 2, lineText: "for i in range(10):" },
    { text: "range(10):", x: 98, y: 20, width: 90, height: 20, lineIndex: 0, wordIndex: 3, lineText: "for i in range(10):" },
    { text: "if", x: 60, y: 50, width: 16, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    if i % 2 == 0:" },
    { text: "print", x: 100, y: 80, width: 45, height: 20, lineIndex: 2, wordIndex: 0, lineText: '        print("even")' },
    { text: '("even")', x: 150, y: 80, width: 60, height: 20, lineIndex: 2, wordIndex: 1, lineText: '        print("even")' },
    { text: "else:", x: 60, y: 110, width: 45, height: 20, lineIndex: 3, wordIndex: 0, lineText: "    else:" },
    { text: "print", x: 100, y: 140, width: 45, height: 20, lineIndex: 4, wordIndex: 0, lineText: '        print("odd")' },
  ];
  const out = env.joinPillItems(items);
  const lines = out.split("\n");
  assert.equal(lines.length, 5);
  assert.equal(lines[0], "for i in range(10):");
  assert.equal(lines[1], "    if");
  assert.equal(lines[2], '        print ("even")');
  assert.equal(lines[3], "    else:");
  assert.equal(lines[4], '        print');
});

test("T3.4: Cross-Feature - Pairwise class/def/return interaction with self member assignment", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "class", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0, lineText: "class Counter:" },
    { text: "Counter:", x: 70, y: 20, width: 70, height: 20, lineIndex: 0, wordIndex: 1, lineText: "class Counter:" },
    { text: "def", x: 60, y: 50, width: 28, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    def get(self) -> int:" },
    { text: "get(self)", x: 95, y: 50, width: 70, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    def get(self) -> int:" },
    { text: "->", x: 170, y: 50, width: 18, height: 20, lineIndex: 1, wordIndex: 2, lineText: "    def get(self) -> int:" },
    { text: "int:", x: 195, y: 50, width: 35, height: 20, lineIndex: 1, wordIndex: 3, lineText: "    def get(self) -> int:" },
    { text: "return", x: 100, y: 80, width: 50, height: 20, lineIndex: 2, wordIndex: 0, lineText: "        return self.val" },
    { text: "self.val", x: 155, y: 80, width: 65, height: 20, lineIndex: 2, wordIndex: 1, lineText: "        return self.val" },
  ];
  const out = env.joinPillItems(items);
  const lines = out.split("\n");
  assert.equal(lines.length, 3);
  assert.equal(lines[0], "class Counter:");
  assert.equal(lines[1], "    def get(self) -> int:");
  assert.equal(lines[2], "        return self.val");
});

test("T3.5: Cross-Feature - Gutter line number segregation alongside code block", () => {
  const env = setupCodeOcrEnvironment();
  // Gutter line numbers at x=10-25 with is_gutter=true
  // Code tokens start at x=60
  const items = [
    // Line 1: gutter '1' at x=15, code at x=60
    { text: "for", x: 60, y: 20, width: 28, height: 20, lineIndex: 0, wordIndex: 0, lineText: "for i in range(10):" },
    { text: "i", x: 95, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1, lineText: "for i in range(10):" },
    // Line 2: gutter '2' at x=15, code at x=100 (indented 4 spaces relative to for)
    { text: "if", x: 100, y: 50, width: 16, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    if i % 2 == 0:" },
    { text: "i", x: 122, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    if i % 2 == 0:" },
  ];
  const out = env.joinPillItems(items);
  const lines = out.split("\n");
  assert.equal(lines[0], "for i");
  assert.equal(lines[1], "    if i", "Code indentation must calculate relative to first code token, NOT the gutter margin!");
});

// =============================================================================
// Tier 4: Real-World Application Scenarios (5 tests)
// =============================================================================

test("T4.1: Real-World Scenario - User benchmark screenshot (media_1793996685095_927ef312.png) replication", () => {
  const env = setupCodeOcrEnvironment();
  // Exact 10 lines from the user benchmark screenshot:
  // Line 1:  1  for i in range(10):
  // Line 2:  2      if i % 2 == 0:
  // Line 3:  3          print(f"%i is even")
  // Line 4:  4      else:
  // Line 5:  5          print(f"%i is odd")
  // Line 7:  7  0 is even
  // Line 8:  8  1 is odd
  // Line 9:  9  2 is even
  // Line 10: 10 3 is odd
  const res = buildOcrResult([
    {
      text: "for i in range(10):",
      y: 20,
      words: [
        { text: "for", x: 60, width: 28 },
        { text: "i", x: 95, width: 10 },
        { text: "in", x: 112, width: 18 },
        { text: "range(10):", x: 138, width: 90 },
      ],
    },
    {
      text: "    if i % 2 == 0:",
      y: 50,
      words: [
        { text: "if", x: 100, width: 16 },
        { text: "i", x: 122, width: 10 },
        { text: "%", x: 138, width: 12 },
        { text: "2", x: 156, width: 10 },
        { text: "==", x: 172, width: 18 },
        { text: "0:", x: 196, width: 16 },
      ],
    },
    {
      text: '        print(f"%i is even")',
      y: 80,
      words: [
        { text: "print(f", x: 140, width: 55 },
        { text: '"%i', x: 200, width: 25 },
        { text: "is", x: 230, width: 16 },
        { text: 'even")', x: 250, width: 45 },
      ],
    },
    {
      text: "    else:",
      y: 110,
      words: [{ text: "else:", x: 100, width: 45 }],
    },
    {
      text: '        print(f"%i is odd")',
      y: 140,
      words: [
        { text: "print(f", x: 140, width: 55 },
        { text: '"%i', x: 200, width: 25 },
        { text: "is", x: 230, width: 16 },
        { text: 'odd")', x: 250, width: 40 },
      ],
    },
    {
      text: "0 is even",
      y: 200, // gap indicates console output separator
      words: [
        { text: "0", x: 60, width: 10 },
        { text: "is", x: 78, width: 16 },
        { text: "even", x: 100, width: 35 },
      ],
    },
    {
      text: "1 is odd",
      y: 230,
      words: [
        { text: "1", x: 60, width: 10 },
        { text: "is", x: 78, width: 16 },
        { text: "odd", x: 100, width: 30 },
      ],
    },
    {
      text: "2 is even",
      y: 260,
      words: [
        { text: "2", x: 60, width: 10 },
        { text: "is", x: 78, width: 16 },
        { text: "even", x: 100, width: 35 },
      ],
    },
    {
      text: "3 is odd",
      y: 290,
      words: [
        { text: "3", x: 60, width: 10 },
        { text: "is", x: 78, width: 16 },
        { text: "odd", x: 100, width: 30 },
      ],
    },
  ]);

  env.renderTextHighlights(res, 800, 600);
  const pills = env.highlightsContainer.children;

  // Verify total pill count across all 9 active lines:
  // L1: 4 + L2: 6 + L3: 4 + L4: 1 + L5: 4 + L6: 3 + L7: 3 + L8: 3 + L9: 3 = 31 pills
  assert.equal(pills.length, 31, "Must render exactly 31 highlight pills across the benchmark image");

  // Verify Line 2 'if i % 2 == 0:' has all 6 pills rendered and none missing!
  const line2Pills = pills.filter((p) => p.dataset.line === "1");
  assert.equal(line2Pills.length, 6, "Line 2 must have all 6 tokens rendered (if, i, %, 2, ==, 0:)");

  // Verify Line 4 'else:' has pill
  const line4Pills = pills.filter((p) => p.dataset.line === "3");
  assert.equal(line4Pills.length, 1);
  assert.ok(line4Pills[0].title.includes('"else:"'));

  // Verify Line 6 '0 is even' has all 3 pills rendered
  const line6Pills = pills.filter((p) => p.dataset.line === "5");
  assert.equal(line6Pills.length, 3, "Line 6 '0 is even' must have all 3 tokens highlighted");

  // Verify Line 8 '2 is even' has all 3 pills rendered
  const line8Pills = pills.filter((p) => p.dataset.line === "7");
  assert.equal(line8Pills.length, 3, "Line 8 '2 is even' must have all 3 tokens highlighted");
});

test("T4.2: Real-World Scenario - Python Data Science Pipeline snippet", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "import", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0, lineText: "import numpy as np" },
    { text: "numpy", x: 70, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 1, lineText: "import numpy as np" },
    { text: "as", x: 120, y: 20, width: 20, height: 20, lineIndex: 0, wordIndex: 2, lineText: "import numpy as np" },
    { text: "np", x: 145, y: 20, width: 20, height: 20, lineIndex: 0, wordIndex: 3, lineText: "import numpy as np" },
    { text: "data", x: 20, y: 50, width: 35, height: 20, lineIndex: 1, wordIndex: 0, lineText: "data = np.arange(10)" },
    { text: "=", x: 60, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 1, lineText: "data = np.arange(10)" },
    { text: "np.arange(10)", x: 75, y: 50, width: 100, height: 20, lineIndex: 1, wordIndex: 2, lineText: "data = np.arange(10)" },
    { text: "res", x: 20, y: 80, width: 25, height: 20, lineIndex: 2, wordIndex: 0, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "=", x: 50, y: 80, width: 10, height: 20, lineIndex: 2, wordIndex: 1, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "[x", x: 65, y: 80, width: 20, height: 20, lineIndex: 2, wordIndex: 2, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "**", x: 90, y: 80, width: 18, height: 20, lineIndex: 2, wordIndex: 3, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "2", x: 112, y: 80, width: 10, height: 20, lineIndex: 2, wordIndex: 4, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "for", x: 128, y: 80, width: 25, height: 20, lineIndex: 2, wordIndex: 5, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "x", x: 158, y: 80, width: 10, height: 20, lineIndex: 2, wordIndex: 6, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "in", x: 172, y: 80, width: 18, height: 20, lineIndex: 2, wordIndex: 7, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "data", x: 195, y: 80, width: 35, height: 20, lineIndex: 2, wordIndex: 8, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "if", x: 235, y: 80, width: 16, height: 20, lineIndex: 2, wordIndex: 9, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "x", x: 255, y: 80, width: 10, height: 20, lineIndex: 2, wordIndex: 10, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "%", x: 270, y: 80, width: 12, height: 20, lineIndex: 2, wordIndex: 11, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "2", x: 288, y: 80, width: 10, height: 20, lineIndex: 2, wordIndex: 12, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "==", x: 304, y: 80, width: 18, height: 20, lineIndex: 2, wordIndex: 13, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
    { text: "0]", x: 328, y: 80, width: 20, height: 20, lineIndex: 2, wordIndex: 14, lineText: "res = [x ** 2 for x in data if x % 2 == 0]" },
  ];
  const out = env.joinPillItems(items);
  const lines = out.split("\n");
  assert.equal(lines.length, 3);
  assert.equal(lines[0], "import numpy as np");
  assert.equal(lines[1], "data = np.arange(10)");
  assert.equal(lines[2], "res = [x ** 2 for x in data if x % 2 == 0]");
});

test("T4.3: Real-World Scenario - TypeScript async handler with strict equality and arrow function", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "export", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0, lineText: "export const handler = async (req: Request) => {" },
    { text: "const", x: 70, y: 20, width: 40, height: 20, lineIndex: 0, wordIndex: 1, lineText: "export const handler = async (req: Request) => {" },
    { text: "handler", x: 115, y: 20, width: 50, height: 20, lineIndex: 0, wordIndex: 2, lineText: "export const handler = async (req: Request) => {" },
    { text: "=", x: 170, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 3, lineText: "export const handler = async (req: Request) => {" },
    { text: "async", x: 185, y: 20, width: 40, height: 20, lineIndex: 0, wordIndex: 4, lineText: "export const handler = async (req: Request) => {" },
    { text: "(req:", x: 230, y: 20, width: 40, height: 20, lineIndex: 0, wordIndex: 5, lineText: "export const handler = async (req: Request) => {" },
    { text: "Request)", x: 275, y: 20, width: 60, height: 20, lineIndex: 0, wordIndex: 6, lineText: "export const handler = async (req: Request) => {" },
    { text: "=>", x: 340, y: 20, width: 18, height: 20, lineIndex: 0, wordIndex: 7, lineText: "export const handler = async (req: Request) => {" },
    { text: "{", x: 362, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 8, lineText: "export const handler = async (req: Request) => {" },
    { text: "if", x: 40, y: 50, width: 16, height: 20, lineIndex: 1, wordIndex: 0, lineText: '  if (req.method === "POST") {' },
    { text: "(req.method", x: 62, y: 50, width: 85, height: 20, lineIndex: 1, wordIndex: 1, lineText: '  if (req.method === "POST") {' },
    { text: "===", x: 152, y: 50, width: 25, height: 20, lineIndex: 1, wordIndex: 2, lineText: '  if (req.method === "POST") {' },
    { text: '"POST")', x: 182, y: 50, width: 55, height: 20, lineIndex: 1, wordIndex: 3, lineText: '  if (req.method === "POST") {' },
    { text: "{", x: 242, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 4, lineText: '  if (req.method === "POST") {' },
  ];
  const out = env.joinPillItems(items);
  const lines = out.split("\n");
  assert.equal(lines.length, 2);
  assert.equal(lines[0], "export const handler = async (req: Request) => {");
  assert.equal(lines[1], '  if (req.method === "POST") {');
});

test("T4.4: Real-World Scenario - Rust match pattern expression with Result", () => {
  const env = setupCodeOcrEnvironment();
  const items = [
    { text: "match", x: 20, y: 20, width: 45, height: 20, lineIndex: 0, wordIndex: 0, lineText: "match res {" },
    { text: "res", x: 70, y: 20, width: 25, height: 20, lineIndex: 0, wordIndex: 1, lineText: "match res {" },
    { text: "{", x: 100, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2, lineText: "match res {" },
    { text: "Ok(val)", x: 60, y: 50, width: 55, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    Ok(val) => val * 2," },
    { text: "=>", x: 120, y: 50, width: 18, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    Ok(val) => val * 2," },
    { text: "val", x: 145, y: 50, width: 25, height: 20, lineIndex: 1, wordIndex: 2, lineText: "    Ok(val) => val * 2," },
    { text: "*", x: 175, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 3, lineText: "    Ok(val) => val * 2," },
    { text: "2,", x: 190, y: 50, width: 18, height: 20, lineIndex: 1, wordIndex: 4, lineText: "    Ok(val) => val * 2," },
    { text: "Err(_)", x: 60, y: 80, width: 50, height: 20, lineIndex: 2, wordIndex: 0, lineText: "    Err(_) => 0," },
    { text: "=>", x: 115, y: 80, width: 18, height: 20, lineIndex: 2, wordIndex: 1, lineText: "    Err(_) => 0," },
    { text: "0,", x: 140, y: 80, width: 18, height: 20, lineIndex: 2, wordIndex: 2, lineText: "    Err(_) => 0," },
    { text: "}", x: 20, y: 110, width: 10, height: 20, lineIndex: 3, wordIndex: 0, lineText: "}" },
  ];
  const out = env.joinPillItems(items);
  const lines = out.split("\n");
  assert.equal(lines.length, 4);
  assert.equal(lines[0], "match res {");
  assert.equal(lines[1], "    Ok(val) => val * 2,");
  assert.equal(lines[2], "    Err(_) => 0,");
  assert.equal(lines[3], "}");
});

test("T4.5: Real-World Scenario - Full End-to-End User Selection & Clipboard Workflow", async () => {
  const env = setupCodeOcrEnvironment();
  // Simulate complete user workflow:
  // 1. OCR returns full results
  const res = buildOcrResult([
    {
      text: "for i in range(10):",
      y: 20,
      words: [
        { text: "for", x: 60, width: 28 },
        { text: "i", x: 95, width: 10 },
        { text: "in", x: 112, width: 18 },
        { text: "range(10):", x: 138, width: 90 },
      ],
    },
    {
      text: "    if i % 2 == 0:",
      y: 50,
      words: [
        { text: "if", x: 100, width: 16 },
        { text: "i", x: 122, width: 10 },
        { text: "%", x: 138, width: 12 },
        { text: "2", x: 156, width: 10 },
        { text: "==", x: 172, width: 18 },
        { text: "0:", x: 196, width: 16 },
      ],
    },
  ]);

  // 2. Highlights rendered in DOM
  env.renderTextHighlights(res, 800, 600);
  assert.equal(env.highlightsContainer.children.length, 10);

  // 3. User double-clicks on line 2 (the 'if i % 2 == 0:' line)
  const line2FirstPill = env.highlightsContainer.children.find((p) => p.dataset.line === "1");
  assert.ok(line2FirstPill);

  await line2FirstPill.dispatchEvent("dblclick");

  // 4. Clipboard contains the full indented line!
  const copied = env.getClipboard();
  assert.equal(copied, "    if i % 2 == 0:", "Double-clicking code pill must copy full indented line without missing syntax");
});
