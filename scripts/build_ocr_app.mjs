// Keep a valid worker path even when Tauri reuses a cached resource-copy step.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';

export function stageExecutable(source, destination) {
  if (!fs.existsSync(source)) throw new Error('Build the OCR runtime first: python tools/ocr_worker/build.py');
  fs.mkdirSync(path.dirname(destination), {recursive:true});
  const temporary = fs.mkdtempSync(path.join(path.dirname(destination), '.ocr-stage-'));
  try {
    const staged = path.join(temporary, path.basename(destination));
    fs.copyFileSync(source, staged);
    fs.chmodSync(staged, fs.statSync(source).mode & 0o777);
    // Replacing the directory entry preserves a running Linux worker's inode.
    // Never rename/delete the destination first: an interrupted build must leave
    // either the original or the fully copied executable at its expected path.
    fs.renameSync(staged, destination);
  } catch (error) {
    if (['EPERM','EACCES','EBUSY','ETXTBSY'].includes(error.code)) {
      throw new Error('Close DeskLens before rebuilding: the OS has locked its OCR runtime.', {cause:error});
    }
    throw error;
  } finally {
    fs.rmSync(temporary, {recursive:true, force:true});
  }
}

async function main() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const args = process.argv.slice(2);
  const targetFlag = args.indexOf('--target');
  const target = targetFlag >= 0 ? args[targetFlag+1] : process.env.CARGO_BUILD_TARGET;
  const profile = args.includes('--debug') ? 'debug' : 'release';
  const targetRoot = path.resolve(root, process.env.CARGO_TARGET_DIR || 'target');
  const directory = path.join(targetRoot, ...(target ? [target] : []), profile, 'ocr-runtime');
  const executable = process.platform === 'win32' ? 'desklens-ocr.exe' : 'desklens-ocr';
  stageExecutable(path.join(root,'build','ocr-runtime',executable), path.join(directory,executable));
  const require = createRequire(import.meta.url);
  const manifest = require.resolve('@tauri-apps/cli/package.json');
  const cli = path.resolve(path.dirname(manifest), JSON.parse(fs.readFileSync(manifest,'utf8')).bin.tauri);
  const child = spawn(process.execPath, [cli,'build','--features','ocr-v2','--config','src-tauri/tauri.ocr.conf.json',...args], {cwd:root, stdio:'inherit'});
  child.on('error', error => { console.error(error.message);process.exitCode=1; });
  child.on('exit', (code,signal) => { process.exitCode=code ?? (signal ? 1 : 0); });
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => {console.error(error.message);process.exitCode=1;});
}
