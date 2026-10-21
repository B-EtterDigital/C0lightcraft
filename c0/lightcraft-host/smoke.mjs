// Run with: PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs node c0/lightcraft-host/smoke.mjs
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, stat, mkdir, writeFile } from 'node:fs/promises';
import { resolve, extname, sep } from 'node:path';

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root = resolve('dist/c0host');
const scratch = resolve(process.env.C0LIGHT_SMOKE_DIR || '/tmp/c0light-browser');
await mkdir(scratch, { recursive: true });
const types = { '.html':'text/html', '.js':'text/javascript', '.wasm':'application/wasm', '.json':'application/json' };
const server = createServer(async (req, res) => {
  try {
    const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    const file = resolve(root, '.' + (pathname === '/' ? '/index.html' : pathname));
    if (!file.startsWith(root + sep)) { res.writeHead(403).end(); return; }
    res.setHeader('Content-Type', types[extname(file)] || 'application/octet-stream');
    res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
    res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
    res.end(await readFile(file));
  } catch { res.writeHead(404).end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const browser = await chromium.launch({ executablePath:process.env.CHROMIUM || '/usr/bin/chromium', headless:true,
  args:['--no-sandbox','--enable-unsafe-swiftshader','--use-angle=swiftshader','--disable-dev-shm-usage'] });
const errors = [];
const deadline=setTimeout(()=>{console.error('Smoke exceeded 180 seconds');void browser.close();},180000);
try {
  const page = await browser.newPage({ viewport:{ width:1280,height:900 } });
  page.on('pageerror', e => {errors.push(e.message);console.error('[browser]',e.message);});
  page.on('console', msg => {
    if (msg.type()==='error') {errors.push(msg.text());console.error('[browser]',msg.text());}
    if (msg.text().startsWith('[smoke]')) console.log(msg.text());
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.waitForFunction(() => typeof window.__lightcraftControl === 'function', null, { timeout:120000 });
  await page.waitForTimeout(100);
  await page.screenshot({path:resolve(scratch,'guest-empty.png')});
  const result = await page.evaluate(async () => {
    const check = (condition, message) => { if (!condition) throw Error(message); };
    const call = async (method, params={}) => {
      console.info('[smoke] '+method);
      const reply = JSON.parse(await window.__c0hostRequest(JSON.stringify({method,params})));
      check(reply.ok, `${method}: ${JSON.stringify(reply)}`); return reply.result;
    };
    const canvas = document.createElement('canvas'); canvas.width=12; canvas.height=8;
    const paint=canvas.getContext('2d'); paint.fillStyle='#4477aa'; paint.fillRect(0,0,12,8);
    paint.fillStyle='#eecc33'; paint.fillRect(3,2,6,4);
    const png=canvas.toDataURL('image/png').split(',')[1];
    const nativeRaf=window.requestAnimationFrame, nativeTimeout=window.setTimeout;
    const nativeCancel=window.cancelAnimationFrame, nativeClear=window.clearTimeout;
    const frames=new Map(), timers=new Map(); let token=2000000000, resumed=false;
    const restoreCancels=()=>{
      if(resumed && !frames.size && !timers.size) {
        window.cancelAnimationFrame=nativeCancel; window.clearTimeout=nativeClear;
      }
    };
    const cancel=(entries,native,id)=>{
      const entry=entries.get(id);
      if(entry) { entries.delete(id); if(entry.real!==undefined) native.call(window,entry.real); }
      else native.call(window,id);
      restoreCancels();
    };
    window.requestAnimationFrame=callback=>{const id=token++;frames.set(id,{callback});return id;};
    window.setTimeout=(...args)=>{const id=token++;timers.set(id,{args});return id;};
    window.cancelAnimationFrame=id=>cancel(frames,nativeCancel,id);
    window.clearTimeout=id=>cancel(timers,nativeClear,id);
    const formats={};
    try {
      const initial=await call('c0.inspect'); check(initial.photoCount===0,'guest must start empty');
      check(!initial.panes.top_bar && !initial.panes.start_screen && !initial.panes.community_links,'host chrome defaults');
      for (const key of ['revision','dirty','canUndo','canRedo','panes','theme','photoCount','selectedCount','activePhoto','view','collections','importing','exporting','unsaved']) {
        check(key in initial,`inspect missing ${key}`);
      }
      await call('c0.openFile',{name:'procedural.png',bytes:png});
      const query=await call('engine.execute',{command:'catalog.query'});
      const id=query.photos[0].id;
      const imported=await call('engine.execute',{command:'photo.inspect',params:{id}});
      check(Math.abs(Date.parse(imported.imported+'Z')-Date.now())<120000,'browser import clock is not current');
      await call('c0.goTo',{photo:id,view:'detail'});
      await call('ui.menu.invoke',{command:'develop.set',params:{control:'light.exposure',value:0.5}});
      const edited=await call('c0.inspect'); check(edited.photoCount===1 && edited.canUndo && edited.view==='detail','import/edit/navigation state');
      for (const format of ['jpeg','png','tiff','xmp']) {
        formats[format]=await call('c0.documentBytes',{format,photo:id});
        check(formats[format].bytes.length>0,`${format} empty bytes`);
      }
      check(JSON.stringify(await call('c0.inspect'))===JSON.stringify(edited),'byte export changed inspect state');
      const settings=await call('c0.set',{themeTokens:{chrome:'#102030',accent:'#cf6828',invalidToken:'#ffffff',text:'bad'},panels:{top_bar:true,community_links:false,filmstrip:false,unknown:false,histogram:'bad'}});
      check(settings.unknown.includes('themeTokens.invalidToken') && settings.unknown.includes('panels.unknown'),'unknown settings');
      check(settings.invalid.includes('themeTokens.text') && settings.invalid.includes('panels.histogram'),'invalid settings');
      await call('c0.cancelFileDialog');
      const bad=JSON.parse(await window.__c0hostRequest('{broken')); check(bad.ok===false,'bad JSON accepted');
      const hostile=JSON.parse(await window.__c0hostRequest(JSON.stringify({method:'ui.click',params:{x:0,y:0,count:Number.MAX_SAFE_INTEGER}})));
      check(hostile.ok===false,'hostile input accepted');
      const direct=window.lightcraft.command('__page.control',JSON.stringify({method:'ui.click',params:{count:Number.MAX_SAFE_INTEGER}}));
      await call('c0.inspect');
      check(JSON.parse(await direct).ok===false,'existing command API bypassed the control bounds');
      const failed=JSON.parse(await window.__c0hostRequest(JSON.stringify({method:'c0.openFile',params:{name:'broken.png',bytes:btoa('broken')}})));
      check(failed.ok===false && (await call('c0.inspect')).photoCount===1,'invalid file changed library');
      await call('c0.newDocument',{template:'blank'}); check((await call('c0.inspect')).photoCount===0,'newDocument not empty');
      await call('c0.openFile',{name:formats.png.name,bytes:formats.png.bytes});
      const photo=(await call('engine.execute',{command:'catalog.query'})).photos[0].id;
      await call('c0.goTo',{photo,view:'detail'});
      return { initial, state:await call('c0.inspect'), perf:(await call('ui.inspect')).perf, formats, png, settings, inlinePasses:true,
        extraGlobals:typeof window.lightcraftModule !== 'undefined' };
    } finally {
      window.requestAnimationFrame=nativeRaf; window.setTimeout=nativeTimeout;
      resumed=true;
      for (const [id,entry] of frames) entry.real=nativeRaf.call(window,time=>{
        frames.delete(id); restoreCancels(); entry.callback(time);
      });
      for (const [id,entry] of timers) entry.real=nativeTimeout.call(window,()=>{
        timers.delete(id); restoreCancels();
        const [callback,,...args]=entry.args; callback(...args);
      },entry.args[1]);
      restoreCancels();
    }
  });
  assert.equal(result.extraGlobals,false);
  // Exercise the actual cancellable browser picker and import a file from disk through it.
  const fixture=resolve(scratch,'procedural.png'); await writeFile(fixture,Buffer.from(result.png,'base64'));
  const choose=page.waitForEvent('filechooser');
  await page.evaluate(() => lightcraft.command('file.addPhotos','{}'));
  const picker=await choose;
  const pending=await page.evaluate(async () => JSON.parse(await __c0hostRequest('{"method":"c0.inspect"}')).result.fileDialog);
  assert.equal(pending,true);
  const cancel=await page.evaluate(async () => JSON.parse(await __c0hostRequest('{"method":"c0.cancelFileDialog"}')));
  assert.equal(cancel.result.cancelled,true);
  await picker.setFiles(fixture);
  await page.waitForTimeout(100);
  const count=await page.evaluate(async () => JSON.parse(await __c0hostRequest('{"method":"c0.inspect"}')).result.photoCount);
  assert.equal(count,1,'cancelled picker imported late');
  const next=page.waitForEvent('filechooser');
  await page.evaluate(() => lightcraft.command('file.addPhotos','{}'));
  const selected=await next; await selected.setFiles(fixture);
  await page.waitForFunction(async () => JSON.parse(await __c0hostRequest('{"method":"c0.inspect"}')).result.photoCount===2);
  await page.evaluate(async () => __c0hostRequest(JSON.stringify({method:'ui.menu.invoke',params:{command:'app.about'}})));
  await page.waitForTimeout(250);
  await page.screenshot({path:resolve(scratch,'guest.png')});
  // New creates a temporary library. Reload must recover the saved library left behind.
  await page.waitForFunction(async () => {
    const pending=lightcraft.command('web.stats','{}');
    await __c0hostRequest('{"method":"c0.inspect"}');
    const stats=JSON.parse(await pending);
    return !stats.dirty && !stats.flushing && stats.workers.ready===stats.workers.alive && stats.workers.remoteJobs>0;
  });
  const persistentWorkers=await page.evaluate(async()=>{
    const pending=lightcraft.command('web.stats','{}');
    await __c0hostRequest('{"method":"c0.inspect"}');
    return JSON.parse(await pending).workers;
  });
  await page.reload();
  await page.waitForFunction(() => typeof window.__lightcraftControl==='function',null,{timeout:120000});
  const recovered=await page.evaluate(async () => {
    const call=async(method,params={})=>{
      const reply=JSON.parse(await __c0hostRequest(JSON.stringify({method,params})));
      if(!reply.ok) throw Error(JSON.stringify(reply));
      return reply.result;
    };
    const state=await call('c0.inspect');
    const id=(await call('engine.execute',{command:'catalog.query'})).photos[0].id;
    const photo=await call('engine.execute',{command:'photo.inspect',params:{id}});
    const snapshot=await call('c0.documentBytes',{format:'png',photo:id});
    const pending=lightcraft.command('web.stats','{}');
    await call('c0.inspect');
    return {count:state.photoCount,exposure:photo.develop.light.exposure,bytes:snapshot.bytes.length,stats:JSON.parse(await pending)};
  });
  assert.equal(recovered.count,1,'saved library changed when leaving it');
  assert.equal(recovered.exposure,0.5,'saved edit was lost');
  assert.ok(recovered.bytes>0);
  const memory=await browser.newPage({viewport:{width:1000,height:700}});
  memory.on('pageerror',e=>errors.push(e.message));
  memory.on('console',msg=>{if(msg.type()==='error') errors.push(msg.text());});
  await memory.goto(`http://127.0.0.1:${server.address().port}/?store=memory`);
  await memory.waitForFunction(()=>typeof __lightcraftControl==='function',null,{timeout:120000});
  await memory.evaluate(async bytes=>{
    const call=async(method,params={})=>{
      const reply=JSON.parse(await __c0hostRequest(JSON.stringify({method,params})));
      if(!reply.ok) throw Error(JSON.stringify(reply));
      return reply.result;
    };
    await call('c0.openFile',{name:'memory.png',bytes});
    const id=(await call('engine.execute',{command:'catalog.query'})).photos[0].id;
    await call('c0.goTo',{photo:id,view:'detail'});
  },result.png);
  await memory.waitForFunction(async()=>{
    const pending=lightcraft.command('web.stats','{}');
    await __c0hostRequest('{"method":"c0.inspect"}');
    const stats=JSON.parse(await pending);
    return stats.workers.inlineJobs>0 && stats.rendersInFlight===0 && stats.renderQueue===0;
  },null,{timeout:30000});
  // Wasm regression: session replacement used to panic in the idle face-download destructor.
  const emptyMemory=await memory.evaluate(async()=>{
    const reply=JSON.parse(await __c0hostRequest('{"method":"c0.newDocument"}'));
    if(!reply.ok) throw Error(JSON.stringify(reply));
    return JSON.parse(await __c0hostRequest('{"method":"c0.inspect"}')).result.photoCount;
  });
  assert.equal(emptyMemory,0);
  await memory.close();
  assert.deepEqual(errors,[]);
  const summary={...result,recovered,persistentWorkers,formats:Object.fromEntries(Object.entries(result.formats).map(([k,v])=>[k,{name:v.name,bytes:Buffer.from(v.bytes,'base64').length}])),png:undefined,errors};
  await writeFile(resolve(scratch,'smoke.json'),JSON.stringify(summary,null,2)+'\n');
  console.log(JSON.stringify({ok:true,methods:7,formats:summary.formats,pausedAnimationFrames:true,pickerImport:true,cancelledPickerStayedCancelled:true,persistentReload:true,persistentWorkers,memoryRendering:true,screenshot:resolve(scratch,'guest.png'),wasm:(await stat(resolve(root,'lightcraft_web_bg.wasm'))).size}));
} finally { clearTimeout(deadline); await browser.close(); await new Promise(r => server.close(r)); }
