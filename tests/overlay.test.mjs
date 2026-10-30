import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

// Helper to set up the real src/main.js inside a simulated DOM environment
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
      this.eventListeners = {};
      this.parentNode = null;
    }

    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
    }

    querySelectorAll(selector) {
      if (selector === ".lens-translation-box") {
        return this.children.filter(
          (c) =>
            (c.className && c.className.includes("lens-translation-box")) ||
            c.classList.contains("lens-translation-box")
        );
      }
      return [];
    }

    remove() {
      this.isRemoved = true;
      if (this.parentNode && Array.isArray(this.parentNode.children)) {
        const idx = this.parentNode.children.indexOf(this);
        if (idx !== -1) {
          this.parentNode.children.splice(idx, 1);
        }
      }
    }

    addEventListener(evt, fn) {
      this.eventListeners[evt] = fn;
    }

    async dispatchEvent(evtName, evtData = {}) {
      if (this.eventListeners[evtName]) {
        await this.eventListeners[evtName]({
          stopPropagation: () => {},
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
    "capture-container": new MockElement("div", "capture-container"),
  };

  // Nest #frozen-image inside #capture-container as in index.html
  elements["capture-container"].appendChild(elements["frozen-image"]);

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
      body: new MockElement("body"),
    },
    window: {
      addEventListener(evt, cb) {
        if (evt === "DOMContentLoaded") domCallbacks.push(cb);
      },
      removeEventListener() {},
      innerWidth: 1920,
      innerHeight: 1080,
    },
    navigator: {
      clipboard: {
        writeText: async (text) => {
          clipboardData = text;
        },
      },
    },
    Image: class {
      constructor() {}
    },
    console,
  };
  sandbox.window.document = sandbox.document;

  const scriptPath = path.resolve("src/main.js");
  const code = fs.readFileSync(scriptPath, "utf8");

  vm.createContext(sandbox);
  vm.runInContext(code, sandbox);

  // Trigger DOMContentLoaded
  for (const cb of domCallbacks) {
    cb();
  }

  return {
    renderTranslationOverlays: sandbox.window.renderTranslationOverlays,
    clearTranslationOverlays: sandbox.window.clearTranslationOverlays,
    container: elements["capture-container"],
    frozenImage: elements["frozen-image"],
    getClipboard: () => clipboardData,
  };
}

// =============================================================================
// Test Suite: Overlay Rendering, Positioning, Styling, and Interaction
// =============================================================================

test("R3.1: renderTranslationOverlays creates .lens-translation-box elements in #capture-container", () => {
  const env = setupEnvironment();

  const boxes = [
    { text: "Settings", x: 120, y: 80, width: 90, height: 28, translated_text: "Configurações" },
    { text: "Search", x: 220, y: 80, width: 80, height: 28, translated_text: "Pesquisar" },
  ];

  env.renderTranslationOverlays(boxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 2);
  assert.equal(overlays[0].textContent, "Configurações");
  assert.equal(overlays[1].textContent, "Pesquisar");
});

test("R3.2: Overlays match coordinate specifications accurately with px units", () => {
  const env = setupEnvironment();

  const boxes = [
    { text: "Cancel", x: 350.4, y: 410.6, width: 100.2, height: 35.8, translated_text: "Cancelar" },
  ];

  env.renderTranslationOverlays(boxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  const overlay = overlays[0];
  assert.equal(overlay.style.left, "350px");
  assert.equal(overlay.style.top, "411px");
  assert.equal(overlay.style.width, "100px");
  assert.equal(overlay.style.height, "36px");
});

test("R3.3: Overlays apply dark background (#1a1a1a) and white text (#ffffff)", () => {
  const env = setupEnvironment();

  const boxes = [
    { text: "Help", x: 50, y: 50, width: 60, height: 24, translated_text: "Ajuda" },
  ];

  env.renderTranslationOverlays(boxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  const overlay = overlays[0];

  // Verify class assignment on rendered overlay DOM element
  assert.ok(
    overlay.className.includes("lens-translation-box") || overlay.classList.contains("lens-translation-box"),
    "Overlay element must have lens-translation-box class"
  );

  // Directly verify stylesheet rules in src/styles.css to guarantee styling integrity
  const cssPath = path.resolve("src/styles.css");
  const cssContent = fs.readFileSync(cssPath, "utf8");

  assert.ok(cssContent.includes(".lens-translation-box"), "CSS must contain .lens-translation-box selector");
  assert.ok(
    /background-color:\s*#1a1a1a/i.test(cssContent),
    "CSS .lens-translation-box must define background-color: #1a1a1a"
  );
  assert.ok(
    /color:\s*#ffffff/i.test(cssContent),
    "CSS .lens-translation-box must define color: #ffffff"
  );
  assert.ok(
    /border-radius:\s*4px/i.test(cssContent),
    "CSS .lens-translation-box must specify border-radius: 4px"
  );
  assert.ok(
    /z-index:\s*25/i.test(cssContent),
    "CSS .lens-translation-box must specify z-index: 25"
  );
});

test("R3.4: clearTranslationOverlays removes all overlay elements cleanly", () => {
  const env = setupEnvironment();

  const boxes = [
    { text: "Help", x: 50, y: 50, width: 60, height: 24, translated_text: "Ajuda" },
    { text: "Exit", x: 120, y: 50, width: 60, height: 24, translated_text: "Sair" },
  ];

  env.renderTranslationOverlays(boxes);
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 2);

  env.clearTranslationOverlays();
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);
  assert.equal(env.container.children.length, 1);
  assert.equal(env.container.children[0].id, "frozen-image");
});

test("R3.5: Dynamic font sizing adapts to box height and width", () => {
  const env = setupEnvironment();

  const boxes = [
    { text: "A", x: 10, y: 10, width: 150, height: 16, translated_text: "Texto Longo de Exemplo" },
    { text: "Title", x: 10, y: 100, width: 300, height: 40, translated_text: "Título Grande" },
  ];

  env.renderTranslationOverlays(boxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 2);
  const smallBox = overlays[0];
  const largeBox = overlays[1];

  const smallFontSize = parseInt(smallBox.style.fontSize, 10);
  const largeFontSize = parseInt(largeBox.style.fontSize, 10);

  assert.ok(smallFontSize >= 9 && smallFontSize <= 14);
  assert.ok(largeFontSize >= 18 && largeFontSize <= 22);
});

test("R3.6: Click handler copies translation to clipboard and adds .copied class", async () => {
  const env = setupEnvironment();

  const boxes = [
    { text: "Copy Me", x: 50, y: 50, width: 100, height: 30, translated_text: "Copie-me" },
  ];

  env.renderTranslationOverlays(boxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1);
  const overlay = overlays[0];

  await overlay.dispatchEvent("click");

  assert.equal(env.getClipboard(), "Copie-me");
  assert.ok(overlay.classList.contains("copied"));
});

test("R3.7: Graceful handling of empty or invalid box arrays", () => {
  const env = setupEnvironment();

  assert.doesNotThrow(() => env.renderTranslationOverlays(null));
  assert.doesNotThrow(() => env.renderTranslationOverlays([]));
  assert.doesNotThrow(() => env.renderTranslationOverlays([{}]));
  assert.doesNotThrow(() => env.renderTranslationOverlays([{ x: "bad", y: 10 }]));
  assert.doesNotThrow(() => env.renderTranslationOverlays({}));
  assert.doesNotThrow(() => env.renderTranslationOverlays("invalid"));
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);
});
