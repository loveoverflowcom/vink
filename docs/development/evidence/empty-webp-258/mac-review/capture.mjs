// One-off same-scenario evidence for #258. No application fixtures or source overrides.
import {spawn, execFileSync} from 'node:child_process';
import {mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const [base,out,revision,mode] = process.argv.slice(2);
mkdirSync(out,{recursive:true});
const profile=mkdtempSync(join(tmpdir(),'vink266-capture-'));
const chrome=spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',[
  '--headless=new','--remote-debugging-port=0',`--user-data-dir=${profile}`,
  '--no-first-run','--no-default-browser-check','--disable-extensions','--disable-gpu','about:blank'
],{stdio:'ignore'});
const pause=ms=>new Promise(r=>setTimeout(r,ms));
let ws,sequence=0;
const pending=new Map();
function send(method,params={},sessionId) {
  const id=++sequence;
  return new Promise((resolve,reject)=>{
    const timer=setTimeout(()=>{pending.delete(id);reject(new Error(method+' timeout'));},30000);
    pending.set(id,{resolve:v=>{clearTimeout(timer);resolve(v);},reject:e=>{clearTimeout(timer);reject(e);}});
    ws.send(JSON.stringify({id,method,params,sessionId}));
  });
}
try {
  let endpoint;
  for(let n=0;n<200;n++) {
    try {const [port,path]=readFileSync(join(profile,'DevToolsActivePort'),'utf8').split('\n');if(port&&path){endpoint=`ws://127.0.0.1:${port.trim()}${path.trim()}`;break;}}catch{}
    await pause(100);
  }
  if(!endpoint)throw new Error('Chrome did not open DevTools');
  ws=new WebSocket(endpoint);
  await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
  ws.onmessage=event=>{
    const msg=JSON.parse(event.data), waiter=pending.get(msg.id);
    if(waiter){pending.delete(msg.id);msg.error?waiter.reject(new Error(JSON.stringify(msg.error))):waiter.resolve(msg.result);}
  };
  const browser=await send('Browser.getVersion');
  const {targetId}=await send('Target.createTarget',{url:'about:blank'});
  const {sessionId}=await send('Target.attachToTarget',{targetId,flatten:true});
  const call=(method,params)=>send(method,params,sessionId);
  const evaluate=async expression=>{
    const r=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});
    if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));return r.result.value;
  };
  for(const method of ['Page.enable','Runtime.enable','Network.enable','Accessibility.enable'])await call(method);
  await call('Page.bringToFront');
  await call('Emulation.setFocusEmulationEnabled',{enabled:true});
  await call('Emulation.setEmulatedMedia',{features:[{name:'prefers-color-scheme',value:'light'}]});
  const catalog=await (await fetch(base+'/api/v1/tags')).json();
  const tag=catalog.tags.find(t=>t.label==='Thơ');
  if(!tag && !['corpus','focus'].includes(mode))throw new Error('missing real Thơ catalog');
  const records=[];
  if(mode==='focus') {
    await call('Page.addScriptToEvaluateOnNewDocument',{source:`window.__focus258=[];const focus=HTMLElement.prototype.focus;HTMLElement.prototype.focus=function(...args){window.__focus258.push({id:this.id,tag:this.tagName,connected:this.isConnected,tabIndex:this.tabIndex,time:performance.now()});return focus.apply(this,args)};document.addEventListener('focusin',e=>window.__focus258.push({event:'focusin',id:e.target.id,tag:e.target.tagName}));`});
    await call('Page.navigate',{url:base+'/'});
    for(let n=0;n<200;n++){if(await evaluate("document.documentElement.dataset.hydrated==='true' && !!document.querySelector('#public-query')"))break;await pause(100);}
    await evaluate("document.querySelector('#public-query').focus();document.querySelector('#public-query').select()");
    await call('Input.insertText',{text:'focus-probe-258'});
    const point=await evaluate("(()=>{const b=document.querySelector('#public-search button[type=submit]');b.scrollIntoView();const r=b.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()");
    for(const type of ['mousePressed','mouseReleased'])await call('Input.dispatchMouseEvent',{type,...point,button:'left',clickCount:1});
    await pause(2000);
    const trace=await evaluate("({href:location.href,active:{id:document.activeElement.id,tag:document.activeElement.tagName},region:document.querySelector('#community-stream').outerHTML,focus:window.__focus258})");
    writeFileSync(join(out,'focus-trace.json'),JSON.stringify(trace,null,2)+'\n');console.log(JSON.stringify(trace));
  }
  const routes=mode==='focus'?[]:mode==='corpus'?[['corpus','/']]:[['explore','/'],['tag','/the/'+encodeURIComponent(tag.slug)]];
  for(const [surface,route] of routes) {
    await call('Page.navigate',{url:base+route+(mode==='corpus'?'':'?q=no-match-empty-258')});
    let ready=false;
    for(let n=0;n<300;n++) {
      ready=await evaluate(`document.documentElement.dataset.hydrated==='true' && !!document.querySelector('[data-testid=${mode==='corpus'?'public-empty':'public-no-results'}]')`);
      if(ready)break;await pause(100);
    }
    if(!ready)throw new Error('empty route not ready');
    for(const theme of ['light','dark'])for(const width of [320,390,834,1440])for(const scale of [100,200]) {
      await call('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:false});
      await evaluate(`(async()=>{document.documentElement.dataset.theme='${theme}';document.documentElement.style.fontSize='${scale}%';await document.fonts.ready;document.querySelector('#community-stream').scrollIntoView({block:'start'});await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));})()`);
      await pause(350);
      const geometry=await evaluate(`(()=>{
        const box=s=>{const e=document.querySelector(s);if(!e)return null;const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom};};
        return {viewport:{width:innerWidth,height:innerHeight},theme:document.documentElement.dataset.theme,font:getComputedStyle(document.querySelector('#public-results')).fontSize,
          overflow:document.documentElement.scrollWidth-innerWidth,art:box('.vk-empty-state__art'),message:box('.vk-empty-state__message'),actions:box('.vk-empty-state__actions'),nav:box('.vk-ui-bottom-nav'),
          status:[...document.querySelectorAll('#community-stream [role=status]')].map(e=>e.textContent),visibleHeadings:[...document.querySelectorAll('#community-stream h2,#community-stream h3')].filter(e=>e.getClientRects().length).map(e=>e.textContent),
          fonts:[...document.fonts].filter(f=>f.status==='loaded').map(f=>({family:f.family,weight:f.weight})),
          imageRequests:performance.getEntriesByType('resource').filter(e=>e.name.includes('/pkg/empty/')).map(e=>({file:e.name.split('/').pop(),bytes:e.transferSize})),
          artwork:[...document.querySelectorAll('.vk-empty-state__art')].map(e=>({complete:e.complete,naturalWidth:e.naturalWidth,naturalHeight:e.naturalHeight,alt:e.alt}))};})()`);
      const file=`${surface}-${theme}-${width}-${scale}.png`;
      const {data}=await call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
      const bytes=Buffer.from(data,'base64');writeFileSync(join(out,file),bytes);
      records.push({file,revision,route,theme,width,scale,geometry,sha256:createHash('sha256').update(bytes).digest('hex')});
      if(theme==='light' && width===390 && scale===100) {
        const ax=await call('Accessibility.getFullAXTree');
        writeFileSync(join(out,`${surface}-ax.json`),JSON.stringify(ax,null,2)+'\n');
      }
      if(width<=390 && scale===200) {
        const end=await evaluate(`(()=>{
          scrollTo(0,document.documentElement.scrollHeight);
          const a=[...document.querySelectorAll('#public-results a')].at(-1),r=a.getBoundingClientRect();
          const nav=document.querySelector('.vk-ui-bottom-nav'),n=nav?.getBoundingClientRect();
          return {actionBottom:r.bottom,navTop:n?.height?n.top:innerHeight,reachable:r.bottom<=(n?.height?n.top:innerHeight)+1};})()`);
        if(!end.reachable)throw new Error('bottom navigation covers recovery '+JSON.stringify(end));
        const tail=`${surface}-${theme}-${width}-${scale}-end.png`,shot=await call('Page.captureScreenshot',{format:'png',captureBeyondViewport:false}),raw=Buffer.from(shot.data,'base64');
        writeFileSync(join(out,tail),raw);records.push({file:tail,revision,route,theme,width,scale,end,sha256:createHash('sha256').update(raw).digest('hex')});
      }
    }
  }
  const probes={};
  if(records.some(r=>r.geometry?.art)) {
    const manifest=JSON.parse(readFileSync(join(process.cwd(),'apps/vink-web/assets/empty/asset-manifest.json')));
    probes.http=[];
    for(const asset of manifest.runtime_assets){
      const response=await fetch(base+'/pkg/empty/vink-empty-'+asset.id+'.webp');
      const bytes=Buffer.from(await response.arrayBuffer());
      assert.equal(response.status,200);assert.equal(response.headers.get('content-type'),'image/webp');
      assert.equal(bytes.length,asset.bytes);assert.equal(createHash('sha256').update(bytes).digest('hex'),asset.sha256);
      probes.http.push({id:asset.id,status:response.status,mime:response.headers.get('content-type'),bytes:bytes.length,sha256:asset.sha256});
    }
    // These are deliberate decoder probes after the app request ledgers above, not app-mounted images.
    probes.decoder=await evaluate(`Promise.all(${JSON.stringify(manifest.runtime_assets.map(a=>a.id))}.map(async id=>{
      const i=new Image();i.src='/pkg/empty/vink-empty-'+id+'.webp';await i.decode();return {id,width:i.naturalWidth,height:i.naturalHeight};}))`);
    assert.ok(probes.decoder.every(i=>i.width===512&&i.height===427));
    probes.branches=[];
    for(const [path,selector] of [['/?cursor=broken','[data-testid=public-invalid]'],['/the/missing-tag-258','[data-testid=public-tag-unavailable]'],...(mode==='corpus'?[]:[['/','.vk-public-row']])]){
      await call('Page.navigate',{url:base+path});
      let ok=false;
      for(let n=0;n<200;n++){ok=await evaluate(`document.documentElement.dataset.hydrated==='true' && !!document.querySelector(${JSON.stringify(selector)})`);if(ok)break;await pause(50);}
      assert.ok(ok,path+' ready');
      const result=await evaluate(`({path:location.pathname+location.search,art:!!document.querySelector('.vk-empty-state__art'),requests:performance.getEntriesByType('resource').filter(r=>r.name.includes('/pkg/empty/')).length})`);
      assert.equal(result.art,false);assert.equal(result.requests,0);probes.branches.push(result);
    }
  }
  writeFileSync(join(out,'manifest.json'),JSON.stringify({revision,capturedAt:new Date().toISOString(),cwd:process.cwd(),browser,records,probes},null,2)+'\n');
  console.log(`PASS ${records.length} actual viewport captures at ${revision}`);
}finally {
  ws?.close();
  const exited=new Promise(resolve=>chrome.once('exit',resolve));chrome.kill();await Promise.race([exited,pause(5000)]);
  rmSync(profile,{recursive:true,force:true,maxRetries:5,retryDelay:100});
}
