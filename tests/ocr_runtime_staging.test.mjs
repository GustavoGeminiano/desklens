import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawn,spawnSync} from 'node:child_process';
import {once} from 'node:events';
import {stageExecutable} from '../scripts/build_ocr_app.mjs';

test('worker is restored when incremental build skips resource copying', () => {
  const directory=fs.mkdtempSync(path.join(os.tmpdir(),'OCR espaço '));
  try {
    const source=path.join(directory,'source');fs.writeFileSync(source,'complete-runtime');
    const dest=path.join(directory,'target','desklens-ocr');
    stageExecutable(source,dest);assert.equal(fs.readFileSync(dest,'utf8'),'complete-runtime');
    assert.throws(()=>stageExecutable(path.join(directory,'missing'),dest));
    assert.equal(fs.readFileSync(dest,'utf8'),'complete-runtime','Failed preparation retains the working executable');
    assert.deepEqual(fs.readdirSync(path.dirname(dest)),['desklens-ocr']);
  } finally {fs.rmSync(directory,{recursive:true,force:true});}
});

test('atomic staging replaces an active Linux executable without an absent path', {skip:process.platform!=='linux'}, async () => {
  const directory=fs.mkdtempSync(path.join(os.tmpdir(),'ocr-running-'));
  let child;
  try {
    const dest=path.join(directory,'desklens-ocr');fs.copyFileSync(process.execPath,dest);fs.chmodSync(dest,0o755);
    child=spawn(dest,['-e','process.stdout.write("ready");setInterval(()=>{},1000)']);
    await once(child.stdout,'data');
    const originalInode=fs.statSync(dest).ino;
    stageExecutable(process.execPath,dest);
    assert.notEqual(fs.statSync(dest).ino,originalInode,'Publish a new inode rather than overwrite running code');
    assert.doesNotThrow(()=>process.kill(child.pid,0),'The existing worker must stay alive');
    const result=spawnSync(dest,['--version'],{encoding:'utf8'});
    assert.equal(result.status,0);assert.equal(result.stdout.trim(),process.version);
  } finally {
    if(child){child.kill();await once(child,'exit');}
    fs.rmSync(directory,{recursive:true,force:true});
  }
});
