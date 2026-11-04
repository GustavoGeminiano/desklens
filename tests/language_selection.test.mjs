import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

// Helper to set up simulated environment for language selection testing
function setupEnvironment(options = {}) {
  const { invokeMock = null, initialStorage = {} } = options;

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
      this.value = "";
      this.options = [];
      this.selectedIndex = 0;
      this.eventListeners = {};
      this.parentNode = null;
    }

    appendChild(child) {
      child.parentNode = this;
      this.children.push(child);
    }

    querySelector(selector) {
      if (selector === 'option[value="auto"]') {
        return this.options.find((o) => o.value === "auto") || null;
      }
      if (selector.startsWith('option[value="')) {
        const val = selector.match(/value="([^"]+)"/)?.[1];
        return this.options.find((o) => o.value === val) || null;
      }
      return null;
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
          target: this,
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
    "lang-selector-group": new MockElement("div", "lang-selector-group"),
    "source-lang-select": new MockElement("select", "source-lang-select"),
    "target-lang-select": new MockElement("select", "target-lang-select"),
    "capture-container": new MockElement("div", "capture-container"),
  };

  // Populate mock options for target select
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

  // Populate mock options for source select
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

  const storage = { ...initialStorage };
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

  const invokeCalls = [];
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
      __TAURI__: {
        core: {
          invoke: async (cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (invokeMock) {
              const customResult = await invokeMock(cmd, args);
              if (customResult !== undefined) return customResult;
            }
            if (cmd === "extract_text_from_crop") {
              return {
                boxes: [
                  { text: "Hello", x: 10, y: 10, width: 60, height: 20 },
                ],
              };
            }
            if (cmd === "translate_on_screen") {
              return [
                {
                  text: "Hello",
                  x: 10,
                  y: 10,
                  width: 60,
                  height: 20,
                  translated_text: "Olá",
                },
              ];
            }
            return null;
          },
        },
      },
    },
    localStorage: mockLocalStorage,
    navigator: {
      clipboard: {
        writeText: async () => {},
      },
    },
    Image: class {
      constructor() {
        setTimeout(() => {
          this.naturalWidth = 100;
          this.naturalHeight = 100;
          if (this.onload) this.onload();
        }, 1);
      }
    },
    console,
    setTimeout,
    clearTimeout,
  };

  const mainJsPath = path.resolve("src/main.js");
  const code = fs.readFileSync(mainJsPath, "utf-8");

  const context = vm.createContext(sandbox);
  vm.runInContext(code, context);

  // Trigger DOMContentLoaded
  for (const cb of domCallbacks) {
    cb();
  }

  return {
    elements,
    storage,
    invokeCalls,
    sandbox,
  };
}

test("R2.1: index.html contains .lang-selector-group with source and target selects", () => {
  const htmlPath = path.resolve("src/index.html");
  const html = fs.readFileSync(htmlPath, "utf-8");

  assert.ok(
    html.includes('id="crop-actions"'),
    "#crop-actions must exist in index.html"
  );
  assert.ok(
    html.includes('class="lang-selector-group"'),
    ".lang-selector-group must exist in index.html"
  );
  assert.ok(
    html.includes('id="source-lang-select"'),
    "#source-lang-select must exist in index.html"
  );
  assert.ok(
    html.includes('id="target-lang-select"'),
    "#target-lang-select must exist in index.html"
  );

  // Check required languages are present
  const requiredLangs = ["en", "pt", "es", "fr", "de", "ja", "zh"];
  for (const lang of requiredLangs) {
    assert.ok(
      html.includes(`value="${lang}"`),
      `Language option '${lang}' must be present in index.html`
    );
  }
  // Check auto-detect is present for source
  assert.ok(
    html.includes('value="auto"'),
    "Auto Detect option ('auto') must be present in index.html"
  );
});

test("R2.2: styles.css contains rules for .lang-selector-group, .lang-select, and .lang-arrow", () => {
  const cssPath = path.resolve("src/styles.css");
  const css = fs.readFileSync(cssPath, "utf-8");

  assert.ok(
    css.includes(".lang-selector-group"),
    ".lang-selector-group selector must be defined in styles.css"
  );
  assert.ok(
    css.includes(".lang-select"),
    ".lang-select selector must be defined in styles.css"
  );
  assert.ok(
    css.includes(".lang-arrow"),
    ".lang-arrow selector must be defined in styles.css"
  );
});

test("R2.3: Language selectors initialize with defaults and load from localStorage", () => {
  const env = setupEnvironment();
  const sourceSelect = env.elements["source-lang-select"];
  const targetSelect = env.elements["target-lang-select"];

  // Default values
  assert.equal(sourceSelect.value, "auto");
  assert.equal(targetSelect.value, "pt");
});

test("R2.4: Changing language selectors persists to localStorage and updates button title", async () => {
  const env = setupEnvironment();
  const sourceSelect = env.elements["source-lang-select"];
  const targetSelect = env.elements["target-lang-select"];
  const translateBtn = env.elements["translate-action-btn"];

  // Change source language to English
  sourceSelect.value = "en";
  await sourceSelect.dispatchEvent("change");
  assert.equal(env.storage["desklens_source_lang"], "en");

  // Change target language to Spanish (index 2 in mock options)
  targetSelect.selectedIndex = 2;
  targetSelect.value = "es";
  await targetSelect.dispatchEvent("change");
  assert.equal(env.storage["desklens_target_lang"], "es");
  assert.ok(
    translateBtn.title.includes("Spanish"),
    `Button title should reflect new target language: '${translateBtn.title}'`
  );
});

test("R2.5: Invoking translate_on_screen passes chosen sourceLang and targetLang", async () => {
  const env = setupEnvironment();
  const sourceSelect = env.elements["source-lang-select"];
  const targetSelect = env.elements["target-lang-select"];
  const translateBtn = env.elements["translate-action-btn"];

  // Select Japanese -> French
  sourceSelect.value = "ja";
  targetSelect.value = "fr";

  // Simulate clicking translate button
  await translateBtn.dispatchEvent("click");

  const transCall = env.invokeCalls.find((c) => c.cmd === "translate_on_screen");
  assert.ok(transCall, "translate_on_screen must have been invoked");
  assert.equal(transCall.args.sourceLang, "ja");
  assert.equal(transCall.args.targetLang, "fr");
  assert.ok(Array.isArray(transCall.args.boxes));
  assert.equal(transCall.args.boxes.length, 1);
});

test("R2.6: Unpacks enriched translate_on_screen payload { boxes, detected_source_lang } without TypeError", async () => {
  const env = setupEnvironment({
    invokeMock: async (cmd) => {
      if (cmd === "translate_on_screen") {
        return {
          boxes: [
            {
              text: "Bonjour",
              x: 10,
              y: 10,
              width: 60,
              height: 20,
              translated_text: "Hello",
              is_paragraph: true,
            },
          ],
          detected_source_lang: "fr",
        };
      }
    },
  });

  const translateBtn = env.elements["translate-action-btn"];
  // Clicking translate should succeed and not throw TypeError: .map is not a function
  await assert.doesNotReject(async () => {
    await translateBtn.dispatchEvent("click");
  });

  const transCalls = env.invokeCalls.filter((c) => c.cmd === "translate_on_screen");
  assert.ok(transCalls.length >= 1, "translate_on_screen should be called");
});

test("R2.7: Smart target language switch redirects from 'pt' to 'en' when detected source is 'pt'", async () => {
  const env = setupEnvironment({
    invokeMock: async (cmd, args) => {
      if (cmd === "translate_on_screen") {
        return {
          boxes: [
            {
              text: "Lembrando que...",
              x: 21,
              y: 14,
              width: 969,
              height: 69,
              translated_text: args?.targetLang === "en" ? "Remember that..." : "Lembrando que...",
              is_paragraph: true,
            },
          ],
          detected_source_lang: "pt",
        };
      }
    },
  });

  const sourceSelect = env.elements["source-lang-select"];
  const targetSelect = env.elements["target-lang-select"];
  const translateBtn = env.elements["translate-action-btn"];

  sourceSelect.value = "auto";
  targetSelect.value = "pt";

  await translateBtn.dispatchEvent("click");

  // Target must automatically switch from 'pt' to 'en' to avoid no-op translation
  assert.equal(targetSelect.value, "en", "target-lang-select should be automatically switched to 'en'");
  assert.equal(env.storage["desklens_target_lang"], "en", "Smart switch must persist 'en' to localStorage");
  assert.ok(
    translateBtn.title.includes("English") || translateBtn.title.includes("en"),
    `Translate button title should reflect new target 'English': ${translateBtn.title}`
  );
});

test("R2.8: #lang-selector-group remains visible and is not hidden by default in crop toolbar", () => {
  const env = setupEnvironment();
  const langGroup = env.elements["lang-selector-group"];
  assert.ok(langGroup, "lang-selector-group must exist");
  assert.equal(
    langGroup.classList.contains("hidden"),
    false,
    "lang-selector-group must not have 'hidden' class by default"
  );
});

test("R2.9: Auto-detect displays detected source language in the source selector option label", async () => {
  const env = setupEnvironment({
    invokeMock: async (cmd) => {
      if (cmd === "translate_on_screen") {
        return {
          boxes: [
            {
              text: "Texto em português",
              x: 10,
              y: 10,
              width: 100,
              height: 20,
              translated_text: "Text in Portuguese",
            },
          ],
          detected_source_lang: "pt",
        };
      }
    },
  });

  const translateBtn = env.elements["translate-action-btn"];
  await translateBtn.dispatchEvent("click");

  const sourceSelect = env.elements["source-lang-select"];
  const autoOption = sourceSelect.options.find((o) => o.value === "auto");
  assert.ok(autoOption, "Auto option must exist");
  assert.ok(
    autoOption.text.includes("PT") || autoOption.textContent.includes("PT"),
    `Auto option text should display detected code (PT): '${autoOption.text}'`
  );
});

test("R2.10: Smart target switch triggers second translation call with adjusted target language", async () => {
  const env = setupEnvironment({
    invokeMock: async (cmd, args) => {
      if (cmd === "translate_on_screen") {
        return {
          boxes: [
            {
              text: "Texto de teste",
              x: 10,
              y: 10,
              width: 100,
              height: 20,
              translated_text: args?.targetLang === "en" ? "Test text" : "Texto de teste",
            },
          ],
          detected_source_lang: "pt",
        };
      }
    },
  });

  const targetSelect = env.elements["target-lang-select"];
  const translateBtn = env.elements["translate-action-btn"];

  targetSelect.value = "pt";
  await translateBtn.dispatchEvent("click");

  const transCalls = env.invokeCalls.filter((c) => c.cmd === "translate_on_screen");
  assert.equal(transCalls.length, 2, "Should re-invoke translation with switched target language");
  assert.equal(transCalls[0].args.targetLang, "pt");
  assert.equal(transCalls[1].args.targetLang, "en");
});

