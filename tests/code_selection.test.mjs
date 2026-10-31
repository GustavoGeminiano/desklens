import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

function setupEnvironment() {
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
    Image: class {
      constructor() {
        this.naturalWidth = 800;
        this.naturalHeight = 600;
      }
    },
  };

  const scriptPath = path.resolve("src", "main.js");
  const scriptContent = fs.readFileSync(scriptPath, "utf-8");

  const context = vm.createContext(sandbox);
  vm.runInContext(scriptContent, context);

  // Trigger DOMContentLoaded
  for (const cb of domCallbacks) {
    cb();
  }

  return { sandbox, elements, getClipboard: () => clipboardData };
}

test("CODE-1: Single word selection returns token directly", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  const items = [{ text: "def", x: 20, y: 30, width: 30, height: 20, lineIndex: 0 }];
  const result = joinPillItems(items);
  assert.equal(result, "def");
});

test("CODE-2: Drag selection across single indented line preserves indentation", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  // Single indented line: `    result = (count * 2)`
  const items = [
    { text: "result", x: 60, y: 100, width: 50, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    result = (count * 2)" },
    { text: "=", x: 120, y: 100, width: 10, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    result = (count * 2)" },
    { text: "(count", x: 140, y: 100, width: 50, height: 20, lineIndex: 1, wordIndex: 2, lineText: "    result = (count * 2)" },
    { text: "*", x: 200, y: 100, width: 10, height: 20, lineIndex: 1, wordIndex: 3, lineText: "    result = (count * 2)" },
    { text: "2)", x: 220, y: 100, width: 20, height: 20, lineIndex: 1, wordIndex: 4, lineText: "    result = (count * 2)" },
  ];

  const result = joinPillItems(items);
  assert.equal(result, "    result = (count * 2)");
});

test("CODE-3: Mid-line phrase selection does not prepend line indentation", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  // Selection starts from mid-line word (wordIndex > 0)
  const items = [
    { text: "(count", x: 140, y: 100, width: 50, height: 20, lineIndex: 1, wordIndex: 2, lineText: "    result = (count * 2)" },
    { text: "*", x: 200, y: 100, width: 10, height: 20, lineIndex: 1, wordIndex: 3, lineText: "    result = (count * 2)" },
    { text: "2)", x: 220, y: 100, width: 20, height: 20, lineIndex: 1, wordIndex: 4, lineText: "    result = (count * 2)" },
  ];

  const result = joinPillItems(items);
  assert.equal(result, "(count * 2)");
});

test("CODE-4: Multi-line selection preserves relative indentation and line breaks", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  const items = [
    // Line 0: def calculate():
    { text: "def", x: 20, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 0, lineText: "def calculate():" },
    { text: "calculate():", x: 60, y: 20, width: 100, height: 20, lineIndex: 0, wordIndex: 1, lineText: "def calculate():" },
    // Line 1:     result = 10
    { text: "result", x: 60, y: 50, width: 50, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    result = 10" },
    { text: "=", x: 120, y: 50, width: 10, height: 20, lineIndex: 1, wordIndex: 1, lineText: "    result = 10" },
    { text: "10", x: 140, y: 50, width: 20, height: 20, lineIndex: 1, wordIndex: 2, lineText: "    result = 10" },
    // Line 2:         return result
    { text: "return", x: 100, y: 80, width: 50, height: 20, lineIndex: 2, wordIndex: 0, lineText: "        return result" },
    { text: "result", x: 160, y: 80, width: 50, height: 20, lineIndex: 2, wordIndex: 1, lineText: "        return result" },
  ];

  const result = joinPillItems(items);
  const lines = result.split("\n");
  assert.equal(lines.length, 3);
  assert.equal(lines[0], "def calculate():");
  assert.equal(lines[1], "    result = 10");
  assert.equal(lines[2], "        return result");
});

test("CODE-5: Single-character items estimate char width and spacing without NaN", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  // Single-character tokens only
  const items = [
    { text: "x", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "=", x: 40, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "1", x: 60, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 2 },
  ];

  const result = joinPillItems(items);
  assert.ok(!result.includes("NaN"), "Result must not contain NaN");
  assert.equal(result, "x = 1");
});

test("CODE-6: renderInteractivePills renders pills with pixel-aligned coordinates", () => {
  const { sandbox, elements } = setupEnvironment();
  const renderInteractivePills = sandbox.window.renderInteractivePills;

  const items = [
    { text: "def", x: 20.4, y: 30.2, width: 35.8, height: 20.0, lineIndex: 0, wordIndex: 0, lineText: "def foo():" },
    { text: "foo():", x: 60.0, y: 30.0, width: 50.0, height: 20.0, lineIndex: 0, wordIndex: 1, lineText: "def foo():" },
  ];

  renderInteractivePills(items);

  const container = elements["crop-text-highlights"];
  assert.equal(container.children.length, 2);

  const p0 = container.children[0];
  assert.equal(p0.style.left, "19px"); // Math.round(20.4) - 1 = 19
  assert.equal(p0.style.top, "29px");  // Math.round(30.2) - 1 = 29
  assert.equal(p0.style.width, "38px"); // Math.round(35.8) + 2 = 38
  assert.equal(p0.style.height, "22px"); // 20 + 2 = 22
  assert.equal(p0.dataset.index, "0");
  assert.equal(p0.dataset.line, "0");
});

test("CODE-7: Multi-line drag starting mid-line on Line 0 does NOT prepend phantom spaces to Line 0", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  // Line 0 starts mid-line at `calculate(` (x=100, wordIndex=1).
  // Line 1 is indented 4 spaces at `item_a` (x=40, wordIndex=0, lineText="    item_a").
  const items = [
    { text: "calculate(", x: 100, y: 20, width: 80, height: 20, lineIndex: 0, wordIndex: 1, lineText: "items = calculate(" },
    { text: "item_a", x: 40, y: 50, width: 50, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    item_a" },
  ];

  const result = joinPillItems(items);
  const lines = result.split("\n");
  assert.equal(lines.length, 2);
  assert.equal(lines[0], "calculate(", "Line 0 starting mid-line must NOT have phantom spaces prepended");
  assert.equal(lines[1], "    item_a", "Line 1 must retain its 4 spaces indentation");
});

test("CODE-8: Multi-line selection preserves vertical blank line gaps", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  const items = [
    // Line 0: import os at y=20
    { text: "import", x: 20, y: 20, width: 50, height: 20, lineIndex: 0, wordIndex: 0, lineText: "import os" },
    { text: "os", x: 80, y: 20, width: 20, height: 20, lineIndex: 0, wordIndex: 1, lineText: "import os" },
    // Line 1: def foo(): at y=75 (pitch gap: 75 - 20 = 55px >= medianPitch * 1.6)
    { text: "def", x: 20, y: 75, width: 30, height: 20, lineIndex: 1, wordIndex: 0, lineText: "def foo():" },
    { text: "foo():", x: 60, y: 75, width: 50, height: 20, lineIndex: 1, wordIndex: 1, lineText: "def foo():" },
  ];

  const result = joinPillItems(items);
  const lines = result.split("\n");
  assert.equal(lines.length, 3, "Must insert blank line between import and def block");
  assert.equal(lines[0], "import os");
  assert.equal(lines[1], "", "Line 1 must be empty line");
  assert.equal(lines[2], "def foo():");
});

test("CODE-9: Selection items in non-monotonic line order are stably sorted", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  // Items passed with Line 1 first, Line 0 second
  const items = [
    { text: "result", x: 60, y: 50, width: 50, height: 20, lineIndex: 1, wordIndex: 0, lineText: "    result" },
    { text: "def", x: 20, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 0, lineText: "def" },
  ];

  const result = joinPillItems(items);
  const lines = result.split("\n");
  assert.equal(lines.length, 2);
  assert.equal(lines[0], "def", "Line 0 must come first");
  assert.equal(lines[1], "    result", "Line 1 must come second");
});

test("CODE-10: Proportional font relative indentation snaps to exact grid without drift", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  // Proportional font without lineText:
  // charW estimated from word width is ~8.2px, but 2-space tab step is 20px.
  // 3 indented lines at 20px, 40px, 60px should snap to 2, 4, 6 spaces (not 2, 5, 7 spaces).
  const items = [
    { text: "root", x: 20, y: 20, width: 32.8, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "step1", x: 40, y: 45, width: 41.0, height: 20, lineIndex: 1, wordIndex: 0 },
    { text: "step2", x: 60, y: 70, width: 41.0, height: 20, lineIndex: 2, wordIndex: 0 },
    { text: "step3", x: 80, y: 95, width: 41.0, height: 20, lineIndex: 3, wordIndex: 0 },
  ];

  const result = joinPillItems(items);
  const lines = result.split("\n");
  assert.equal(lines[0], "root");
  assert.equal(lines[1], "  step1", "20px offset must snap to 2 spaces");
  assert.equal(lines[2], "    step2", "40px offset must snap to 4 spaces");
  assert.equal(lines[3], "      step3", "60px offset must snap to 6 spaces");
});

test("CODE-11: Indented block followed by outer block without lineText preserves Line 0 relative indentation", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  // Selection where Line 0 is inner indented (x=80, wordIndex=0)
  // and Line 1 is outer unindented (x=20, wordIndex=0)
  // Line 0 must NOT have its indentation stripped by a `lineSeq > 0` check.
  const items = [
    { text: "return", x: 80, y: 20, width: 60, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "42", x: 150, y: 20, width: 20, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "next_call()", x: 20, y: 50, width: 110, height: 20, lineIndex: 1, wordIndex: 0 },
  ];

  const result = joinPillItems(items);
  const lines = result.split("\n");
  assert.equal(lines.length, 2);
  assert.ok(lines[0].startsWith("      return"), `Line 0 must have 6 spaces relative indentation: '${lines[0]}'`);
  assert.equal(lines[1], "next_call()", "Line 1 must have 0 relative indentation");
});

test("CODE-12: Hanging continuation indent preserves exact column character spacing without forced 4-space tab snapping", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  // Hanging indent continuation line at column 10 (100px from margin) with charW = 10px.
  // Must NOT be rounded up to 12 spaces by a 4-space tab grid!
  const items = [
    { text: "def", x: 20, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "query(alpha,", x: 60, y: 20, width: 120, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "beta)", x: 120, y: 50, width: 50, height: 20, lineIndex: 1, wordIndex: 0 },
  ];

  const result = joinPillItems(items);
  const lines = result.split("\n");
  assert.equal(lines.length, 2);
  assert.equal(lines[0], "def query(alpha,");
  assert.equal(lines[1], "          beta)", `Hanging indent must be preserved with exactly 10 spaces: '${lines[1]}'`);
});

test("CODE-13: Fullwidth Unicode programming characters in pills copy as normalized syntax", () => {
  const { sandbox } = setupEnvironment();
  const joinPillItems = sandbox.window.joinPillItems;

  const items = [
    { text: "x", x: 20, y: 20, width: 10, height: 20, lineIndex: 0, wordIndex: 0 },
    { text: "===", x: 40, y: 20, width: 30, height: 20, lineIndex: 0, wordIndex: 1 },
    { text: "10", x: 80, y: 20, width: 20, height: 20, lineIndex: 0, wordIndex: 2 },
  ];

  const result = joinPillItems(items);
  assert.equal(result, "x === 10");
});

