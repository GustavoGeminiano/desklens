import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

const deferred = () => { let resolve, reject; const promise = new Promise((a,b) => { resolve=a; reject=b; }); return {promise,resolve,reject}; };
const flush = async () => { for(let i=0;i<8;i++) await Promise.resolve(); };
function environment(recognize, translate = async () => ({boxes:[]}), cropper = null) {
  class Element {
    constructor() { this.style={}; this.dataset={}; this.children=[]; this.events={}; this.value=''; this.options=[]; this.attributes={}; this.classes=new Set(); this.classList={add:(x)=>this.classes.add(x),remove:(x)=>this.classes.delete(x),contains:(x)=>this.classes.has(x),toggle:(x,on)=>on?this.classes.add(x):this.classes.delete(x)}; }
    set innerHTML(value) { this.html=value; if(value==='') this.children=[]; }
    get innerHTML() { return this.html||''; }
    set className(value) { this.class=value; }
    get className() { return this.class||''; }
    appendChild(child) { child.parentNode=this; this.children.push(child); }
    remove() { this.parentNode.children=this.parentNode.children.filter(x=>x!==this); }
    querySelectorAll(selector) { return this.children.filter(x=>x.className.includes(selector.slice(1))); }
    setAttribute(key,value) { this.attributes[key]=value; }
    addEventListener(name,fn) { this.events[name]=fn; }
    closest() { return null; }
    async fire(name,event={}) { return this.events[name]?.({target:this,stopPropagation(){},preventDefault(){},...event}); }
  }
  const nodes=new Map(); const el=(id)=>{ if(!nodes.has(id))nodes.set(id,new Element());return nodes.get(id); };
  el('ocr-mode-select').value='auto'; el('source-lang-select').value='auto';el('target-lang-select').value='pt';
  const events={};const calls=[];const timers=[];let id=0;let copied='';
  const window={innerWidth:1000,innerHeight:800,addEventListener:(name,fn)=>events[name]=fn,removeEventListener(){},__TAURI__:{core:{invoke:async(name,args)=>{
    calls.push({name,args});
    if(name==='crop_selection') return cropper ? cropper(++id) : {crop_id:++id,data_url:'data:image/png;base64,test',image_width:400,image_height:200};
    if(name==='extract_text_from_crop') return recognize(args);
    if(name==='copy_text_to_clipboard'){copied=args.text;return;}
    if(name==='translate_on_screen')return translate(args);
  }}}};
  const context={window,document:{getElementById:el,createElement:()=>new Element(),body:new Element(),elementFromPoint:()=>null},navigator:{language:'pt',clipboard:{writeText:async text=>copied=text}},localStorage:{getItem(){return null;},setItem(){}},console,setTimeout:fn=>timers.push(fn),clearTimeout(){}};
  vm.runInNewContext(fs.readFileSync('src/main.js','utf8'),context);events.DOMContentLoaded();
  const select=async()=>{await el('selection-layer').fire('mousedown',{clientX:10,clientY:10});events.mousemove({clientX:210,clientY:110});await events.mouseup();await flush();};
  return {el,events,calls,timers,window,select,clipboard:()=>copied};
}
function result(text='i = 1', uncertain=false) {
  let offset=0;
  const words=text.split(' ').map((word,index)=>{const start=offset;offset+=word.length+1;return {text:word,x:1+index*20,y:1,width:word==='i'?1:12,height:18,start_utf16:start,end_utf16:offset-1};});
  return {full_text:text,image_width:400,image_height:200,uncertain,lines:[{text,words}],boxes:[{text,x:1,y:1,width:80,height:18}]};
}

test('one canonical recognition powers highlights, copy and translation; narrow edge glyphs remain selectable',async()=>{
  const env=environment(async()=>result());await env.select();
  assert.equal(env.el('crop-text-highlights').children.length,3);
  assert.equal(env.el('crop-text-highlights').children[0].style.width,'0.5px');
  await env.el('ocr-action-btn').fire('click');
  assert.equal(env.clipboard(),'i = 1');
  await env.el('translate-action-btn').fire('click');
  assert.equal(env.calls.filter(x=>x.name==='extract_text_from_crop').length,1);
  assert.equal(env.calls.find(x=>x.name==='extract_text_from_crop').args.mode,'auto');
  assert.equal(env.calls.find(x=>x.name==='extract_text_from_crop').args.cropId,1);
});

test('late OCR from crop A cannot replace crop B or the preview',async()=>{
  const a=deferred();const b=deferred();const env=environment(args=>args.cropId===1?a.promise:b.promise);
  await env.select();await env.select();b.resolve(result('fresh'));await flush();a.resolve(result('stale'));await flush();
  assert.equal(env.el('ocr-preview-text').value,'fresh');
  assert.ok(env.el('crop-text-highlights').children[0].title.includes('fresh'));
});

test('closing the overlay cancels pending OCR effects and delayed copy labels',async()=>{
  const pending=deferred();const env=environment(()=>pending.promise);await env.select();await env.el('close-overlay-btn').fire('click');pending.resolve(result('stale'));await flush();
  assert.equal(env.el('crop-text-highlights').children.length,0);
  assert.equal(env.el('ocr-preview-text').value,'');
  assert.equal(env.el('ocr-status').textContent,'');
});

test('uncertain recognition requires review before copy and Copy anyway uses unchanged canonical text',async()=>{
  const env=environment(async()=>result('i = 1',true));await env.select();await env.el('ocr-action-btn').fire('click');
  assert.equal(env.clipboard(),'');assert.equal(env.el('ocr-preview').open,true);
  await env.el('ocr-copy-anyway-btn').fire('click');assert.equal(env.clipboard(),'i = 1');
});

test('mode switch invalidates pending recognition without accepting its old text',async()=>{
  const old=deferred();const env=environment(args=>args.mode==='auto'?old.promise:Promise.resolve(result('code')));await env.select();
  env.el('ocr-mode-select').value='code';await env.el('ocr-mode-select').fire('change');await flush();old.resolve(result('old'));await flush();
  assert.equal(env.el('ocr-preview-text').value,'code');
  assert.deepEqual(env.calls.filter(x=>x.name==='extract_text_from_crop').map(x=>x.args.mode),['auto','code']);
});

test('late translation after closing cannot resurrect an overlay',async()=>{
  const translated=deferred();const env=environment(async()=>result(),()=>translated.promise);await env.select();const inFlight=env.el('translate-action-btn').fire('click');await flush();
  await env.el('close-overlay-btn').fire('click');translated.resolve({boxes:[{text:'i = 1',translated_text:'old',x:1,y:1,width:80,height:18}]});await inFlight;
  assert.equal(env.el('capture-container').children.length,0);
  assert.equal(env.el('ocr-action-btn').innerHTML,'<span>📝</span> Copy Text');
});

test('selection copies exact UTF-16 intervals including punctuation, Unicode and whitespace',async()=>{
  const env=environment(async()=>result());
  const canonicalText='😀 café  !=\n    β';
  const items=[{text:'café',canonicalText,start_utf16:3,end_utf16:7},{text:'β',canonicalText,start_utf16:15,end_utf16:16}];
  assert.equal(env.window.joinPillItems(items),canonicalText.slice(3,16));
});

test('retry creates a fresh crop after an OCR error',async()=>{
  let calls=0;const env=environment(async()=>{if(++calls===1)throw new Error('Worker unavailable');return result('recovered');});await env.select();
  assert.match(env.el('ocr-status').textContent,/Worker unavailable/);
  await env.el('ocr-retry-btn').fire('click');await flush();
  assert.equal(env.el('ocr-preview-text').value,'recovered');
  assert.equal(env.calls.filter(x=>x.name==='crop_selection').length,2);
});


test('out-of-order crop responses cannot replace the most recent selection',async()=>{
  const a=deferred();const b=deferred();
  const env=environment(async args=>result(args.cropId===2?'new crop':'old crop'),undefined,id=>id===1?a.promise:b.promise);
  const first=env.select();await flush();const second=env.select();await flush();
  b.resolve({crop_id:2,data_url:'new-image',image_width:400,image_height:200});await second;
  a.resolve({crop_id:1,data_url:'old-image',image_width:400,image_height:200});await first;await flush();
  assert.equal(env.el('crop-preview-thumb').src,'new-image');
  assert.ok(env.calls.some(x=>x.name==='clear_crop_session' && x.args.cropId===1));
  assert.equal(env.el('ocr-preview-text').value,'new crop');
  assert.equal(env.calls.filter(x=>x.name==='extract_text_from_crop').length,1);
});

test('old copy timers cannot overwrite status for a later selection',async()=>{
  const env=environment(async()=>result());await env.select();await env.el('ocr-action-btn').fire('click');
  const oldTimer=env.timers.at(-1);await env.select();env.el('ocr-action-btn').innerHTML='New operation';oldTimer();
  assert.equal(env.el('ocr-action-btn').innerHTML,'New operation');
});

test('language change invalidates an in-flight translation without redoing OCR',async()=>{
  const first=deferred();let translationCalls=0;
  const env=environment(async()=>result(),async()=>++translationCalls===1?first.promise:{boxes:[{text:'i = 1',translated_text:'new language',x:1,y:1,width:80,height:18}]});
  await env.select();const old=env.el('translate-action-btn').fire('click');await flush();
  env.el('target-lang-select').value='fr';await env.el('target-lang-select').fire('change');await flush();
  first.resolve({boxes:[{text:'i = 1',translated_text:'old language',x:1,y:1,width:80,height:18}]});await old;await flush();
  assert.equal(env.el('capture-container').children[0].textContent,'new language');
  assert.equal(env.calls.filter(x=>x.name==='extract_text_from_crop').length,1);
});


test('translation sends complete visual lines instead of one box per word', async () => {
  const lines = ['Industry pioneers build smarter AI agents.', 'Designed for enterprise applications.'];
  const doc = result(lines.join('\n'));
  doc.lines = lines.map((text, index) => ({text, x: 10, y: 10 + index * 22, width: 350, height: 18,
    words: text.split(' ').map((text, word) => ({text, x: 10 + word * 45, y: 10 + index * 22, width: 40, height: 18}))}));
  delete doc.boxes;
  const env = environment(async () => doc, async ({boxes}) => ({boxes: [{...boxes[0], height: 40, translated_text: 'Pioneiros criam agentes de IA mais inteligentes para empresas.'}]}));
  await env.select();
  await env.el('translate-action-btn').fire('click');
  const request = env.calls.find(call => call.name === 'translate_on_screen');
  assert.equal(request.args.boxes.length, 2);
  assert.equal(request.args.boxes[0].text, lines[0]);
  assert.equal(request.args.boxes[1].text, lines[1]);
  const overlays = env.el('capture-container').children;
  assert.equal(overlays.length, 1);
  assert.equal(overlays[0].textContent, 'Pioneiros criam agentes de IA mais inteligentes para empresas.');
});


test('Search Image retains the selected crop until the backend snapshots it', async () => {
  const env = environment(async () => result());
  await env.select();
  const start = env.calls.length;
  await env.el('search-action-btn').fire('click');
  const actions = env.calls.slice(start);
  assert.equal(actions.filter(call => call.name === 'search_google_lens').length, 1);
  assert.equal(actions.filter(call => call.name === 'hide_overlay' || call.name === 'clear_crop_session').length, 0,
    'Hiding or clearing before the backend handles search destroys its input image');
});

test('dragging the toolbar preserves crop and survives late OCR status updates', async () => {
  const recognition = deferred();
  const env = environment(() => recognition.promise);
  await env.select();
  const grip = env.el('toolbar-drag-handle');
  const bar = env.el('crop-actions');
  const cropBefore = {...env.el('crop-box').style};
  await grip.fire('pointerdown', {button:0,pointerId:1,clientX:30,clientY:130});
  env.events.pointermove({pointerId:1,clientX:300,clientY:400,preventDefault(){}});
  assert.equal(bar.style.left,'272px');
  assert.equal(bar.style.top,'380px');
  env.events.pointerup({pointerId:1});
  recognition.resolve(result());await flush();
  assert.equal(bar.style.left,'272px');
  assert.equal(bar.style.top,'380px');
  assert.deepEqual(env.el('crop-box').style,cropBefore);
  assert.equal(env.calls.filter(call=>call.name==='crop_selection').length,1);
  assert.equal(env.calls.filter(call=>call.name==='extract_text_from_crop').length,1);
  env.window.innerWidth=600;env.events.resize();
  assert.equal(bar.style.left,'58px','Keep menu within the resized viewport');
  await env.select();
  assert.notEqual(bar.style.top,'380px','A new selection restores automatic placement');
});

test('toolbar drag clamps edges and stops on pointer cancellation', async () => {
  const env=environment(async()=>result());await env.select();
  const bar=env.el('crop-actions');const grip=env.el('toolbar-drag-handle');
  await grip.fire('pointerdown',{button:0,pointerId:5,clientX:30,clientY:130});
  const before=bar.style.left;
  env.events.pointermove({pointerId:9,clientX:9000,clientY:9000,preventDefault(){}});
  assert.equal(bar.style.left,before);
  env.events.pointermove({pointerId:5,clientX:9000,clientY:9000,preventDefault(){}});
  assert.equal(bar.style.left,'458px');assert.equal(bar.style.top,'732px');
  env.events.pointermove({pointerId:5,clientX:-9000,clientY:-9000,preventDefault(){}});
  assert.equal(bar.style.left,'2px');assert.equal(bar.style.top,'2px');
  env.events.pointercancel({pointerId:5});
  env.events.pointermove({pointerId:5,clientX:300,clientY:400,preventDefault(){}});
  assert.equal(bar.style.left,'2px');assert.equal(bar.style.top,'2px');
});
