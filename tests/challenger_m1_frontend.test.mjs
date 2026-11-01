import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

test("CHALLENGER: Frontend aspect-ratio filter in src/main.js:488 drops single-character words 'I', '1', '!'", () => {
  const code = fs.readFileSync(path.resolve(process.cwd(), "src/main.js"), "utf8");

  // Verify the exact defensive guard in src/main.js
  assert.match(code, /word\.height\s*>\s*word\.width\s*\*\s*3\.2\s*&&\s*word\.text\.length\s*<=\s*3/);

  // Test filter logic against real OCR dimensions for vertical characters
  function shouldFilterWord(word) {
    if (!word.text || !word.width || !word.height) return true;
    if (word.height > word.width * 3.2 && word.text.length <= 3) return true;
    if (word.width < 3 || word.height < 3) return true;
    const isEdgeBorder = (word.x <= 2 || word.y <= 2) && (word.height > 30 && word.width < 15);
    if (isEdgeBorder) return true;
    return false;
  }

  // Realistic OCR word bounding boxes (font size 18px-24px):
  const words = [
    { text: "I", width: 5.0, height: 20.0, x: 50, y: 50 },     // ratio 4.0
    { text: "1", width: 5.0, height: 20.0, x: 70, y: 50 },     // ratio 4.0
    { text: "!", width: 4.0, height: 20.0, x: 90, y: 50 },     // ratio 5.0
    { text: "l", width: 4.0, height: 20.0, x: 110, y: 50 },    // ratio 5.0
    { text: "[", width: 5.0, height: 20.0, x: 130, y: 50 },    // ratio 4.0
    { text: "|", width: 3.5, height: 20.0, x: 150, y: 50 },    // ratio 5.71
    { text: "said", width: 35.0, height: 20.0, x: 170, y: 50 },// ratio 0.57
  ];

  const results = words.map((w) => ({
    text: w.text,
    ratio: w.height / w.width,
    filtered: shouldFilterWord(w),
  }));

  console.log("[CHALLENGER M1 FRONTEND VERIFICATION]", results);

  // Assert that valid tokens 'I', '1', '!', 'l', '[', '|' are all discarded:
  assert.equal(shouldFilterWord(words[0]), true, "'I' is discarded as vertical margin artifact");
  assert.equal(shouldFilterWord(words[1]), true, "'1' is discarded as vertical margin artifact");
  assert.equal(shouldFilterWord(words[2]), true, "'!' is discarded as vertical margin artifact");
  assert.equal(shouldFilterWord(words[3]), true, "'l' is discarded as vertical margin artifact");
  assert.equal(shouldFilterWord(words[4]), true, "'[' is discarded as vertical margin artifact");
  assert.equal(shouldFilterWord(words[5]), true, "'|' is discarded as vertical margin artifact");
  assert.equal(shouldFilterWord(words[6]), false, "'said' is retained");
});
