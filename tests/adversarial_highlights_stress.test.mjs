import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

// Helper to set up simulated browser DOM environment matching src/main.js
function setupHighlightEnvironment() {
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
      this.className = "";
      this.classList = new MockClassList();
      this.style = {};
      this.children = [];
      this.textContent = "";
      this.title = "";
      this.dataset = {};
      this.eventListeners = {};
      this.parentNode = null;
    }

    set className(val) {
      this._className = val || "";
      this.classList = new MockClassList();
      for (const c of this._className.split(/\s+/)) {
        if (c) this.classList.add(c);
      }
    }

    get className() {
      return this._className || "";
    }

    set innerHTML(val) {
      if (val === "") {
        this.children = [];
        this.textContent = "";
      } else {
        this._innerHTML = val;
      }
    }

    get innerHTML() {
      return this._innerHTML || "";
    }

    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
    }

    querySelectorAll(selector) {
      if (selector === ".crop-word-highlight") {
        return this.children.filter((c) =>
          (c.className && c.className.includes("crop-word-highlight")) ||
          c.classList.contains("crop-word-highlight")
        );
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
    "test-capture-btn": new MockElement("button", "test-capture-btn"),
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
  const windowListeners = {};

  class MockImage {
    constructor() {
      this.naturalWidth = 800;
      this.naturalHeight = 600;
      this._src = "";
    }
    set src(val) {
      this._src = val;
      if (this.onload) {
        // Trigger synchronously for deterministic unit testing
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
      innerWidth: 1920,
      innerHeight: 1080,
      addEventListener(evt, fn) {
        if (evt === "DOMContentLoaded") {
          domCallbacks.push(fn);
        } else {
          windowListeners[evt] = fn;
        }
      },
      removeEventListener(evt) {
        delete windowListeners[evt];
      },
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

  // Export internal renderTextHighlights to window for unit testing without altering file on disk
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

// =============================================================================
// Test Suite: Adversarial Stress Tests for Highlights & Degenerate Bounding Boxes
// =============================================================================

test("HL-1: Degenerate ocrResult structure (null, undefined, non-object, missing lines, empty lines)", () => {
  const env = setupHighlightEnvironment();
  assert.doesNotThrow(() => env.renderTextHighlights(null, 400, 300));
  assert.equal(env.highlightsContainer.children.length, 0);

  assert.doesNotThrow(() => env.renderTextHighlights(undefined, 400, 300));
  assert.equal(env.highlightsContainer.children.length, 0);

  assert.doesNotThrow(() => env.renderTextHighlights({}, 400, 300));
  assert.equal(env.highlightsContainer.children.length, 0);

  assert.doesNotThrow(() => env.renderTextHighlights({ lines: [] }, 400, 300));
  assert.equal(env.highlightsContainer.children.length, 0);

  assert.doesNotThrow(() => env.renderTextHighlights({ lines: [{ words: [] }, { words: null }] }, 400, 300));
  assert.equal(env.highlightsContainer.children.length, 0);
});

test("HL-2: Words with NaN coordinates and dimensions are safely filtered or rendered without throwing", () => {
  const env = setupHighlightEnvironment();

  const ocrWithNaN = {
    lines: [
      {
        text: "Line with NaN",
        words: [
          // NaN width: !word.width evaluates true -> dropped immediately
          { text: "NaNWidth", x: 10, y: 10, width: NaN, height: 20 },
          // NaN height: !word.height evaluates true -> dropped immediately
          { text: "NaNHeight", x: 50, y: 10, width: 30, height: NaN },
          // NaN x coordinate
          { text: "NaNX", x: NaN, y: 10, width: 30, height: 20 },
          // Valid word
          { text: "ValidWord", x: 100, y: 10, width: 60, height: 20 },
        ],
      },
    ],
  };

  assert.doesNotThrow(() => env.renderTextHighlights(ocrWithNaN, 400, 300));
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");

  // NaNX and ValidWord both have non-falsy width and height
  // NaNWidth and NaNHeight were discarded
  assert.ok(pills.length >= 1, "At least ValidWord rendered");
  const validPill = pills.find((p) => p.title.includes("ValidWord"));
  assert.ok(validPill, "ValidWord pill must be rendered");
});

test("HL-3: Words with Infinite coordinates (+Infinity, -Infinity) handled without throwing", () => {
  const env = setupHighlightEnvironment();

  const ocrWithInf = {
    lines: [
      {
        text: "Infinity test",
        words: [
          { text: "InfX", x: Infinity, y: 10, width: 40, height: 20 },
          { text: "InfY", x: 10, y: -Infinity, width: 40, height: 20 },
          { text: "InfW", x: 60, y: 10, width: Infinity, height: 20 },
          { text: "InfH", x: 120, y: 10, width: 40, height: Infinity },
        ],
      },
    ],
  };

  assert.doesNotThrow(() => env.renderTextHighlights(ocrWithInf, 400, 300));
  assert.doesNotThrow(() => env.renderInteractivePills([
    { text: "TestInf", x: Infinity, y: -Infinity, width: Infinity, height: Infinity },
  ]));
});

test("HL-4: Vertical margin fold artifacts (height > 3.2 * width with length <= 3) are filtered out", () => {
  const env = setupHighlightEnvironment();

  const ocrWithMarginArtifacts = {
    lines: [
      {
        text: "RT, EDITORIAL SECTION",
        words: [
          // Margin fold: 17x100 (aspect ratio 5.88, len 3) -> must be discarded!
          { text: "RT,", x: 2, y: 50, width: 17, height: 100 },
          // Margin fold: 50x188 (aspect ratio 3.76, len 2) -> must be discarded!
          { text: "ME", x: 950, y: 40, width: 50, height: 188 },
          // Legitimate tall word (e.g. len > 3): "TALL" (width 20, height 70) -> kept
          { text: "TALL", x: 50, y: 100, width: 20, height: 70 },
          // Legitimate headline words
          { text: "EDITORIAL", x: 100, y: 100, width: 120, height: 28 },
          { text: "SECTION", x: 230, y: 100, width: 100, height: 28 },
        ],
      },
    ],
  };

  env.renderTextHighlights(ocrWithMarginArtifacts, 1000, 800);
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");

  const texts = pills.map((p) => p.title);
  assert.ok(!texts.some((t) => t.includes('"RT,"')), "Margin artifact 'RT,' must be filtered");
  assert.ok(!texts.some((t) => t.includes('"ME"')), "Margin artifact 'ME' must be filtered");
  assert.ok(texts.some((t) => t.includes('"EDITORIAL"')), "'EDITORIAL' must be retained");
  assert.ok(texts.some((t) => t.includes('"SECTION"')), "'SECTION' must be retained");
});

test("HL-5: Sub-3px printer dust and scanner noise (width < 3 or height < 3) are filtered out", () => {
  const env = setupHighlightEnvironment();

  const ocrDust = {
    lines: [
      {
        text: "Valid content",
        words: [
          { text: ".", x: 10, y: 10, width: 2, height: 15 },
          { text: "_", x: 20, y: 10, width: 15, height: 2 },
          { text: ",", x: 30, y: 10, width: 1, height: 1 },
          { text: "Valid", x: 50, y: 10, width: 40, height: 18 },
        ],
      },
    ],
  };

  env.renderTextHighlights(ocrDust, 400, 300);
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");
  assert.equal(pills.length, 1, "Only the valid word must be rendered");
  assert.ok(pills[0].title.includes("Valid"));
});

test("HL-6: Edge border touching artifacts (x <= 2 or y <= 2 with height > 30, width < 15) are filtered out", () => {
  const env = setupHighlightEnvironment();

  const ocrBorderArtifacts = {
    lines: [
      {
        text: "Border lines",
        words: [
          // Crop border artifact sticking to left edge
          { text: "I", x: 1, y: 50, width: 8, height: 120 },
          // Crop border artifact sticking to top edge
          { text: "1", x: 50, y: 1, width: 10, height: 40 },
          // Normal word not at border
          { text: "News", x: 100, y: 100, width: 50, height: 20 },
        ],
      },
    ],
  };

  env.renderTextHighlights(ocrBorderArtifacts, 400, 300);
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");
  assert.equal(pills.length, 1, "Edge border boxes must be discarded");
  assert.ok(pills[0].title.includes("News"));
});

test("HL-7: Massive overlapping bounding boxes are rendered and selectable without crashing", () => {
  const env = setupHighlightEnvironment();

  // 30 overlapping words all sharing the same position
  const overlappingWords = [];
  for (let i = 0; i < 30; i++) {
    overlappingWords.push({
      text: `Word${i}`,
      x: 100,
      y: 100,
      width: 60,
      height: 20,
    });
  }

  const ocrOverlaps = {
    lines: [
      {
        text: "Overlapping words",
        words: overlappingWords,
      },
    ],
  };

  assert.doesNotThrow(() => env.renderTextHighlights(ocrOverlaps, 800, 600));
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");
  assert.equal(pills.length, 30, "All 30 overlapping pills should be created cleanly");

  // Verify dataset index uniqueness
  const indices = new Set(pills.map((p) => p.dataset.index));
  assert.equal(indices.size, 30, "Every pill must have a unique sequential index");
});

test("HL-8: Extreme aspect ratios (ultra-wide ribbon vs ultra-tall column) render safely", () => {
  const env = setupHighlightEnvironment();

  const extremeItems = [
    // Ultra-wide banner
    { text: "UltraWideHeadlineAcrossEntirePage", x: 10, y: 20, width: 5000, height: 20, lineIndex: 0, wordIndex: 0 },
    // Ultra-tall vertical banner with long text
    { text: "VerticalTextBanner", x: 20, y: 50, width: 25, height: 4000, lineIndex: 1, wordIndex: 0 },
  ];

  assert.doesNotThrow(() => env.renderInteractivePills(extremeItems));
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");
  assert.equal(pills.length, 2);

  // Wide pill
  assert.equal(pills[0].style.width, "5002px"); // 5000 + 2 pad
  assert.equal(pills[0].style.height, "22px");

  // Tall pill
  assert.equal(pills[1].style.width, "27px");
  assert.equal(pills[1].style.height, "4002px");
});

test("HL-9: Multi-column overlapping boxes with conflicting baselines preserve structured text", () => {
  const env = setupHighlightEnvironment();

  // Simulated 2-column layout items passed to joinPillItems
  const colItems = [
    { text: "COL1_A", x: 50, y: 100, width: 60, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "COL1_B", x: 120, y: 100, width: 60, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "COL2_A", x: 300, y: 100, width: 60, height: 20, lineIndex: 1, wordIndex: 0 },
    { text: "COL2_B", x: 370, y: 100, width: 60, height: 20, lineIndex: 1, wordIndex: 1 },
  ];

  const joined = env.joinPillItems(colItems);
  assert.ok(joined.includes("COL1_A COL1_B"));
  assert.ok(joined.includes("COL2_A COL2_B"));
});

test("HL-10: Pill interaction stress: mousedown, drag, and click copy to clipboard without error", async () => {
  const env = setupHighlightEnvironment();

  const items = [
    { text: "DAILY", x: 10, y: 20, width: 60, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "NEWS", x: 80, y: 20, width: 60, height: 20, lineIndex: 0, wordIndex: 1 },
  ];

  env.renderInteractivePills(items);
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");
  assert.equal(pills.length, 2);

  // Trigger mousedown on pill 0
  await pills[0].dispatchEvent("mousedown");
  assert.ok(pills[0].classList.contains("selected"), "Pill should be selected on mousedown");

  // Trigger dblclick on pill 0
  await pills[0].dispatchEvent("dblclick");
  assert.ok(env.getClipboard().includes("DAILY"), "Clipboard should contain copied line text");
});

test("HL-11: Rapid repeated calls clear previous highlights without leaking elements", () => {
  const env = setupHighlightEnvironment();

  for (let cycle = 0; cycle < 20; cycle++) {
    const ocr = {
      lines: [
        {
          text: `Cycle ${cycle}`,
          words: [
            { text: `Word_${cycle}`, x: 10 + cycle, y: 20, width: 50, height: 20 },
          ],
        },
      ],
    };
    env.renderTextHighlights(ocr, 400, 300);
  }

  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");
  // Only the last cycle's pills should be present in the container
  assert.equal(pills.length, 1);
  assert.ok(pills[0].title.includes("Word_19"));
});

test("HL-12: Single narrow characters ('I', '1', '!', 'l', '[', ']', '|') with aspect ratio > 3.2 are preserved", () => {
  const env = setupHighlightEnvironment();

  const ocrNarrow = {
    lines: [
      {
        text: "I see 1 item !",
        words: [
          { text: "I", x: 20, y: 50, width: 5, height: 20 },     // ratio 4.0
          { text: "see", x: 35, y: 50, width: 30, height: 20 },
          { text: "1", x: 75, y: 50, width: 5, height: 20 },     // ratio 4.0
          { text: "item", x: 90, y: 50, width: 40, height: 20 },
          { text: "!", x: 140, y: 50, width: 4, height: 20 },    // ratio 5.0
          { text: "[", x: 155, y: 50, width: 5, height: 20 },    // ratio 4.0
          { text: "0", x: 165, y: 50, width: 10, height: 20 },
          { text: "]", x: 180, y: 50, width: 5, height: 20 },    // ratio 4.0
          { text: "|", x: 195, y: 50, width: 3.5, height: 20 },  // ratio 5.71
        ],
      },
    ],
  };

  env.renderTextHighlights(ocrNarrow, 800, 600);
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");

  const texts = pills.map((p) => p.title);
  assert.ok(texts.some((t) => t.includes('"I"')), "'I' must be rendered");
  assert.ok(texts.some((t) => t.includes('"see"')), "'see' must be rendered");
  assert.ok(texts.some((t) => t.includes('"1"')), "'1' must be rendered");
  assert.ok(texts.some((t) => t.includes('"item"')), "'item' must be rendered");
  assert.ok(texts.some((t) => t.includes('"!"')), "'!' must be rendered");
  assert.ok(texts.some((t) => t.includes('"["')), "'[' must be rendered");
  assert.ok(texts.some((t) => t.includes('"]"')), "']' must be rendered");
  assert.ok(texts.some((t) => t.includes('"|"')), "'|' must be rendered");
  assert.equal(pills.length, 9, "All 9 tokens must be rendered as pills");
});

test("HL-13: Edge border artifacts on right and bottom borders are filtered out", () => {
  const env = setupHighlightEnvironment();

  const ocrBorderRightBottom = {
    lines: [
      {
        text: "Boundary checks",
        words: [
          // Normal word safely inside crop (800x600)
          { text: "Normal", x: 200, y: 200, width: 60, height: 20 },
          // Right edge border artifact: x + width = 794 + 5 = 799 >= 800 - 2, height > 30, width < 15
          { text: "RT_EDGE", x: 794, y: 100, width: 5, height: 50 },
          // Bottom edge border artifact: y + height = 400 + 40 = 599 >= 600 - 2
          { text: "BOT_EDGE", x: 300, y: 559, width: 8, height: 40 },
        ],
      },
    ],
  };

  env.renderTextHighlights(ocrBorderRightBottom, 800, 600);
  const pills = env.highlightsContainer.querySelectorAll(".crop-word-highlight");

  const texts = pills.map((p) => p.title);
  assert.ok(texts.some((t) => t.includes('"Normal"')), "'Normal' must be rendered");
  assert.ok(!texts.some((t) => t.includes('"RT_EDGE"')), "Right border artifact must be filtered");
  assert.ok(!texts.some((t) => t.includes('"BOT_EDGE"')), "Bottom border artifact must be filtered");
  assert.equal(pills.length, 1, "Only normal word rendered");
});

