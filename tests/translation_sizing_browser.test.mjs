import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';

const chrome = ['/usr/bin/google-chrome', '/usr/bin/chromium', '/snap/bin/chromium'].find(p => fs.existsSync(p));
test('real browser fits translated paragraphs and retains overflowing text access', {skip: !chrome}, () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'desklens-sizing-'));
  try {
    let html = fs.readFileSync('src/index.html', 'utf8')
      .replace('href="styles.css"', `href="${pathToFileURL(path.resolve('src/styles.css'))}"`)
      .replace('src="main.js"', `src="${pathToFileURL(path.resolve('src/main.js'))}"`);
    const fixture = `
      window.addEventListener('load', () => {
        document.getElementById('overlay-view').classList.remove('hidden');
        document.getElementById('settings-view').classList.add('hidden');
        const text = 'Functions can contain any piece of code within their scopes, including calls to other functions! Observe the largestAndSmallest() function in image 10, which, to display the smallest and largest value in a list, calls the other functions.';
        const cases = [
          {x:171,y:418,width:776,height:65,text,translated_text:text},
          {x:171,y:520,width:200,height:40,text,translated_text:text.repeat(8)},
          {x:171,y:620,width:200,height:30,text:'Next paragraph',translated_text:'Next paragraph'},
        ];
        window.renderTranslationOverlays(cases);
        const rows = [...document.querySelectorAll('.lens-translation-box')].map(el => ({
          clientHeight:el.clientHeight,scrollHeight:el.scrollHeight,clientWidth:el.clientWidth,scrollWidth:el.scrollWidth,
          font:parseFloat(getComputedStyle(el).fontSize),display:getComputedStyle(el).display,
          overflow:getComputedStyle(el).overflowY,bottom:el.getBoundingClientRect().bottom,text:el.textContent,
        }));
        const report = document.createElement('pre');report.id='sizing-report';report.textContent=JSON.stringify(rows);document.body.appendChild(report);
      });`;
    html = html.replace('</body>', `<script>${fixture}</script></body>`);
    const input = path.join(directory, 'fixture.html'); fs.writeFileSync(input, html);
    const result = spawnSync(chrome, ['--headless','--no-sandbox','--disable-gpu','--allow-file-access-from-files',
      '--window-size=1440,900','--virtual-time-budget=1500','--dump-dom',pathToFileURL(input).href], {encoding:'utf8', timeout:30000,maxBuffer:4*1024*1024});
    assert.equal(result.status,0,result.stderr);
    const match = result.stdout.match(/<pre id="sizing-report">(.*?)<\/pre>/s);
    assert.ok(match,'App must load and render the browser fixture');
    const rows = JSON.parse(match[1]);
    assert.equal(rows[0].display,'block');
    assert.ok(rows[0].clientHeight > 0 && rows[0].clientWidth > 0);
    assert.ok(rows[0].font >= 11);
    assert.ok(rows[0].scrollHeight <= rows[0].clientHeight + 1,JSON.stringify(rows[0]));
    assert.ok(rows[0].scrollWidth <= rows[0].clientWidth + 1);
    assert.ok(rows[0].text.startsWith('Functions can contain'));
    assert.equal(rows[1].overflow,'auto');
    assert.ok(rows[1].bottom <= 616,'Long text must not cover the next paragraph');
    assert.ok(rows[1].text.length > 1000,'Overflow must retain the complete translation');
  } finally {fs.rmSync(directory,{recursive:true,force:true});}
});
