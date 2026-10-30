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
        return this.children.filter((c) =>
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
// Test Suite: Adversarial Verification of renderTranslationOverlays & clearTranslationOverlays
// =============================================================================

test("ADV-1: Empty, null, undefined, and non-array inputs are handled gracefully without error", () => {
  const env = setupEnvironment();

  assert.doesNotThrow(() => env.renderTranslationOverlays([]));
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);

  assert.doesNotThrow(() => env.renderTranslationOverlays(null));
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);

  assert.doesNotThrow(() => env.renderTranslationOverlays(undefined));
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);

  assert.doesNotThrow(() => env.renderTranslationOverlays({}));
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);

  assert.doesNotThrow(() => env.renderTranslationOverlays("invalid"));
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);

  assert.doesNotThrow(() => env.renderTranslationOverlays(12345));
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);
});

test("ADV-2: Malformed boxes (missing x, missing y, null box) are safely filtered", () => {
  const env = setupEnvironment();

  const malformedList = [
    null,
    undefined,
    {},
    { text: "No coordinates" },
    { x: "not-a-number", y: 10, width: 50, height: 20 },
    { x: 10, y: "string", width: 50, height: 20 },
    { text: "Valid Box", x: 100, y: 150, width: 200, height: 40, translated_text: "Caixa Válida" },
  ];

  env.renderTranslationOverlays(malformedList);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1, "Only the single valid box should be rendered");
  assert.equal(overlays[0].textContent, "Caixa Válida");
  assert.equal(overlays[0].style.left, "100px");
  assert.equal(overlays[0].style.top, "150px");
  assert.equal(overlays[0].style.width, "200px");
  assert.equal(overlays[0].style.height, "40px");
});

test("ADV-3: Extreme dimensions (0px, 1px, 5000px, negative coords, float decimals)", () => {
  const env = setupEnvironment();

  const extremeBoxes = [
    // 0px dimension
    { text: "Zero", x: 0, y: 0, width: 0, height: 0, translated_text: "Zero" },
    // 1px micro-box
    { text: "Tiny", x: 5, y: 5, width: 1, height: 1, translated_text: "Minúsculo" },
    // 5000px massive dimension
    { text: "Huge", x: 50, y: 50, width: 5000, height: 5000, translated_text: "Enorme" },
    // Negative coordinates and dimensions
    { text: "Negative", x: -120.4, y: -45.8, width: -100.2, height: -30.7, translated_text: "Negativo" },
    // High-precision float decimals
    { text: "Decimals", x: 12.34567, y: 89.98765, width: 123.456, height: 32.789, translated_text: "Decimais" },
  ];

  assert.doesNotThrow(() => env.renderTranslationOverlays(extremeBoxes));

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 5);

  // 0px box
  assert.equal(overlays[0].style.left, "0px");
  assert.equal(overlays[0].style.top, "0px");
  assert.equal(overlays[0].style.width, "0px");
  assert.equal(overlays[0].style.height, "0px");
  const font0 = parseInt(overlays[0].style.fontSize, 10);
  assert.ok(!isNaN(font0) && font0 >= 9 && font0 <= 22, `Font size must be valid integer: ${font0}`);

  // 1px micro-box
  assert.equal(overlays[1].style.left, "5px");
  assert.equal(overlays[1].style.top, "5px");
  assert.equal(overlays[1].style.width, "1px");
  assert.equal(overlays[1].style.height, "1px");
  const font1 = parseInt(overlays[1].style.fontSize, 10);
  assert.ok(!isNaN(font1) && font1 >= 9 && font1 <= 22);

  // 5000px massive box
  assert.equal(overlays[2].style.left, "50px");
  assert.equal(overlays[2].style.top, "50px");
  assert.equal(overlays[2].style.width, "5000px");
  assert.equal(overlays[2].style.height, "5000px");
  const fontHuge = parseInt(overlays[2].style.fontSize, 10);
  assert.equal(fontHuge, 22, "Huge box font size should cap at 22px");

  // Negative coords & negative dimensions
  assert.equal(overlays[3].style.left, "-120px");
  assert.equal(overlays[3].style.top, "-46px");
  assert.equal(overlays[3].style.width, "-100px");
  assert.equal(overlays[3].style.height, "-31px");
  const fontNeg = parseInt(overlays[3].style.fontSize, 10);
  assert.ok(!isNaN(fontNeg) && fontNeg >= 9 && fontNeg <= 22);

  // Decimals rounded with Math.round
  assert.equal(overlays[4].style.left, "12px");
  assert.equal(overlays[4].style.top, "90px");
  assert.equal(overlays[4].style.width, "123px");
  assert.equal(overlays[4].style.height, "33px");
});

test("ADV-4: Text length extremes: massive multi-line paragraph vs ultra-short word ('Oi')", () => {
  const env = setupEnvironment();

  const longParagraph =
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. " +
    "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. " +
    "Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";

  const textBoxes = [
    { text: "Hi", x: 10, y: 10, width: 40, height: 24, translated_text: "Oi" },
    { text: "Long", x: 10, y: 60, width: 250, height: 28, translated_text: longParagraph },
    { text: "Empty text", x: 10, y: 150, width: 100, height: 30, translated_text: "" },
    { text: "Original Fallback", x: 10, y: 200, width: 120, height: 30 }, // No translated_text provided
  ];

  env.renderTranslationOverlays(textBoxes);

  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 4);

  // Short word "Oi"
  assert.equal(overlays[0].textContent, "Oi");
  const fontShort = parseInt(overlays[0].style.fontSize, 10);
  assert.ok(fontShort >= 10 && fontShort <= 22);

  // Long paragraph: font calculation adjusts factor with 0.65 floor clamp
  assert.equal(overlays[1].textContent, longParagraph);
  const fontLong = parseInt(overlays[1].style.fontSize, 10);
  assert.equal(fontLong, 13, "Text scales down to 65% floor factor (Math.round(20 * 0.65) = 13px)");

  // Test an even narrower box (height = 16) where 65% hits the 9px absolute minimum
  env.renderTranslationOverlays([
    { text: "Long narrow", x: 10, y: 60, width: 250, height: 16, translated_text: longParagraph },
  ]);
  const narrowOverlay = env.container.querySelectorAll(".lens-translation-box")[0];
  const narrowFont = parseInt(narrowOverlay.style.fontSize, 10);
  assert.equal(narrowFont, 9, "Extremely long text in 16px height clamps to absolute minimum 9px");

  // When translated_text is empty "", it falls back to box.text ("Empty text")
  assert.equal(overlays[2].textContent, "Empty text");

  // Missing translated_text also falls back to box.text
  assert.equal(overlays[3].textContent, "Original Fallback");

  // When BOTH translated_text and text are empty, textContent is empty string ""
  env.renderTranslationOverlays([
    { text: "", x: 10, y: 300, width: 50, height: 20, translated_text: "" },
  ]);
  const emptyBothOverlay = env.container.querySelectorAll(".lens-translation-box")[0];
  assert.equal(emptyBothOverlay.textContent, "");
});

test("ADV-5: XSS and HTML injection resilience: textContent used instead of innerHTML", () => {
  const env = setupEnvironment();

  const injectionBoxes = [
    {
      text: "XSS Test",
      x: 50,
      y: 50,
      width: 200,
      height: 30,
      translated_text: `<script>alert('xss')</script><img src="x" onerror="steal()">`,
    },
  ];

  env.renderTranslationOverlays(injectionBoxes);

  const overlay = env.container.querySelectorAll(".lens-translation-box")[0];
  assert.equal(
    overlay.textContent,
    `<script>alert('xss')</script><img src="x" onerror="steal()">`,
    "Text must be assigned literally via textContent to avoid HTML injection"
  );
  assert.equal(overlay.children.length, 0, "No child DOM nodes should be created from HTML tags");
});

test("ADV-6: CSS stylesheet enforcement: .lens-translation-box defines #1a1a1a background & #ffffff text", () => {
  const cssPath = path.resolve("src/styles.css");
  const cssContent = fs.readFileSync(cssPath, "utf8");

  // Verify CSS stylesheet defines .lens-translation-box with required colors
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

test("ADV-7: DOM container isolation: overlays inserted strictly into #capture-container, siblings preserved", () => {
  const env = setupEnvironment();

  // Verify frozen-image exists inside capture-container initially
  assert.equal(env.container.children.length, 1);
  assert.equal(env.container.children[0].id, "frozen-image");

  const boxes = [
    { text: "One", x: 10, y: 10, width: 50, height: 20, translated_text: "Um" },
    { text: "Two", x: 70, y: 10, width: 50, height: 20, translated_text: "Dois" },
  ];

  env.renderTranslationOverlays(boxes);

  // Total children in container should now be 3 (frozen-image + 2 overlays)
  assert.equal(env.container.children.length, 3);
  // Sibling #frozen-image must still be present and unharmed
  const frozenImg = env.container.children.find((c) => c.id === "frozen-image");
  assert.ok(frozenImg, "#frozen-image must remain in container");
  assert.equal(frozenImg.id, "frozen-image");

  // The 2 overlays must be in container
  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 2);
  for (const ov of overlays) {
    assert.equal(ov.parentNode, env.container, "Overlay parent must be #capture-container");
    assert.equal(ov.className, "lens-translation-box");
  }
});

test("ADV-8: Lifecycle clearing: clearTranslationOverlays() purges all .lens-translation-box without touching siblings", () => {
  const env = setupEnvironment();

  const boxes = [
    { text: "A", x: 10, y: 10, width: 50, height: 20, translated_text: "A" },
    { text: "B", x: 60, y: 10, width: 50, height: 20, translated_text: "B" },
    { text: "C", x: 110, y: 10, width: 50, height: 20, translated_text: "C" },
  ];

  env.renderTranslationOverlays(boxes);
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 3);
  assert.equal(env.container.children.length, 4); // frozen-image + 3

  // Purge
  env.clearTranslationOverlays();

  assert.equal(
    env.container.querySelectorAll(".lens-translation-box").length,
    0,
    "All .lens-translation-box must be removed"
  );
  assert.equal(
    env.container.children.length,
    1,
    "Only #frozen-image should remain in #capture-container"
  );
  assert.equal(env.container.children[0].id, "frozen-image");

  // Calling clearTranslationOverlays() on empty container is safe
  assert.doesNotThrow(() => env.clearTranslationOverlays());
  assert.equal(env.container.children.length, 1);
});

test("ADV-9: Repeated sequential calls clear previous overlays preventing accumulation or memory leaks", () => {
  const env = setupEnvironment();

  // Run batch 1 (3 items)
  env.renderTranslationOverlays([
    { text: "A", x: 1, y: 1, width: 10, height: 10, translated_text: "1" },
    { text: "B", x: 2, y: 2, width: 10, height: 10, translated_text: "2" },
    { text: "C", x: 3, y: 3, width: 10, height: 10, translated_text: "3" },
  ]);
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 3);

  // Run batch 2 (1 item)
  env.renderTranslationOverlays([
    { text: "Solo", x: 50, y: 50, width: 100, height: 30, translated_text: "Apenas Um" },
  ]);
  const overlays = env.container.querySelectorAll(".lens-translation-box");
  assert.equal(overlays.length, 1, "Should have exactly 1 overlay, not 3+1=4");
  assert.equal(overlays[0].textContent, "Apenas Um");

  // Run batch 3 (0 items)
  env.renderTranslationOverlays([]);
  assert.equal(env.container.querySelectorAll(".lens-translation-box").length, 0);
});

test("ADV-10: Click-to-copy interaction and clipboard feedback", async () => {
  const env = setupEnvironment();

  env.renderTranslationOverlays([
    { text: "Translate", x: 20, y: 20, width: 100, height: 30, translated_text: "Traduzir" },
  ]);

  const overlay = env.container.querySelectorAll(".lens-translation-box")[0];
  assert.ok(overlay.title.includes('Original: "Translate"'));
  assert.ok(overlay.title.includes("Click to copy translation"));

  let stoppedPropagation = false;
  await overlay.dispatchEvent("click", {
    stopPropagation: () => {
      stoppedPropagation = true;
    },
  });

  assert.equal(stoppedPropagation, true, "Click event must stop propagation to avoid triggering canvas selection");
  assert.equal(env.getClipboard(), "Traduzir", "Clipboard must receive translated text");
  assert.ok(overlay.classList.contains("copied"), "Overlay must receive .copied class feedback");
});
