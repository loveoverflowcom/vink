// Product Reader, populated loopback scratch publication. Same helper runs at both revisions.
import { writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { createHash } from 'node:crypto';

const path = '/tac-pham/33333333-3333-4333-8333-333333333333';
const native = '#root-composer';
const rich = '[data-vink-editor="comment"] .cm-content';
const activation = '#root-composer-format';
const runtime = "performance.getEntriesByType('resource').filter(e => new URL(e.name).pathname.endsWith('/vink-editor.js'))";
const corpus = Array.from({length:200}, () => 'Mưa **đêm** 👩‍💻 thơ').join('\n').padEnd(4000, 'ơ');
const source = [...corpus].slice(0, 4000).join('') + 'ơ'.repeat(Math.max(0,4000-[...corpus].length));
const frames = 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))';

export async function commentBudget(page, {args, check}) {
  if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(args.base)) throw new Error('scratch loopback required');
  const out = {browser:await page.cdp.send('Browser.getVersion'), corpus_sha256:createHash('sha256').update(source).digest('hex'),
    scalars:[...source].length, lines:source.split('\n').length, ready:[], input:[], caret:[], network:[]};
  await page.viewport(390,844);
  await page.send('Network.enable');
  const listen = msg => {
    if (msg.sessionId === page.sessionId && msg.method === 'Network.requestWillBeSent') {
      out.network.push({method:msg.params.request.method,path:new URL(msg.params.request.url).pathname});
    }
  };
  page.cdp.listeners.push(listen);
  const {identifier} = await page.send('Page.addScriptToEvaluateOnNewDocument',{source:`
    window.vkTiming = {native:null,rich:null,shifts:[],long:[]};
    new MutationObserver(() => {
      const t = window.vkTiming;
      if (t.native===null && document.querySelector('textarea#root-composer')) t.native=performance.now();
      if (t.rich===null && document.querySelector('${rich}')) t.rich=performance.now();
    }).observe(document,{childList:true,subtree:true});
    for (const [name,key] of [['layout-shift','shifts'],['longtask','long']]) {
      new PerformanceObserver(list => { for (const e of list.getEntries()) window.vkTiming[key].push({at:e.startTime,value:e.value??e.duration,recent:e.hadRecentInput}); }).observe({type:name,buffered:true});
    }`});
  try {
    for(let pair=0;pair<7;pair++) for(const cache of ['cold','warm']) {
      if(cache==='cold') await page.send('Network.clearBrowserCache');
      await page.goto('about:blank');
      const start=out.network.length;
      await page.goto(args.base+path);
      await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('.vk-discussion-comment') && document.querySelector('#root-composer')", 'populated Reader');
      await page.eval("document.querySelector('#feedback-heading').scrollIntoView()");
      await page.eval(frames);
      const requests=out.network.slice(start);
      check(!requests.some(r=>r.path.endsWith('/vink-editor.js')), 'Reader has zero CodeMirror requests before interaction');
      if(pair===0 && cache==='cold') await page.capture('618-guest-before-interaction',{target:'#feedback-heading'});
      const supported=await page.eval("!!document.querySelector('"+activation+"')");
      if(supported) {
        await page.eval("document.querySelector('#root-composer-format').addEventListener('click',e=>vkTiming.intent=e.timeStamp,{once:true,capture:true})");
        await page.click(activation);
        await page.waitFor("!!document.querySelector('"+rich+"')", 'compact root ready');
      }
      const row=await page.eval(`({native:vkTiming.native,rich:vkTiming.rich,activationMs:vkTiming.rich===null?null:vkTiming.rich-vkTiming.intent,resources:${runtime}.map(e=>({size:e.transferSize,duration:e.duration})),shifts:vkTiming.shifts,long:vkTiming.long})`);
      if(pair>=2) out.ready.push({cache,...row,readRequests:requests});
      if(pair===6 && cache==='warm') {
        const selector=supported?rich:native;
        await page.eval("document.querySelector("+JSON.stringify(selector)+").focus()");
        await page.send('Input.insertText',{text:source});
        await page.eval(frames);
        // Reader records a foreground view after five seconds even with no edits.
        // Settle the actual view candidate before measuring the typing-only interval.
        await page.waitFor("performance.getEntriesByType('resource').some(r=>new URL(r.name).pathname.endsWith('/views'))",'foreground view settled before typing');
        const initial=out.network.length;
        for(let i=0;i<45;i++){
          await page.eval(`document.addEventListener('beforeinput',e=>{const t=e.timeStamp;let endInput=null;document.addEventListener('input',()=>queueMicrotask(()=>{endInput=performance.now()-t}),{once:true});window.sample=null;requestAnimationFrame(()=>{const raf=performance.now()-t;setTimeout(()=>window.sample={inputMs:endInput,raf,post:performance.now()-t},0)})},{once:true})`);
          await page.send('Input.insertText',{text:'ơ'});
          await page.waitFor('window.sample!=null','input paint sample');
          if(i>=5) out.input.push(await page.eval('window.sample'));
          await page.key('Backspace');
          await page.eval(`document.addEventListener('keydown',e=>{const t=e.timeStamp;window.caretSample=null;requestAnimationFrame(()=>window.caretSample=performance.now()-t)},{once:true})`);
          await page.key('ArrowLeft');
          if(i>=5) out.caret.push(await page.eval('window.caretSample'));
          await page.key('ArrowRight');
        }
        out.requestsDuringTyping=out.network.slice(initial);
        out.after=await page.eval('vkTiming');
        out.dom=await page.send('Memory.getDOMCounters');
        await page.capture('618-root-editing',{target:selector});
      }
    }
    writeFileSync(join(dirname(args.state),'comment-budget.json'),JSON.stringify(out,null,2));
    check(!out.requestsDuringTyping.length,'no network activity during typing');
  } finally {
    await page.send('Page.removeScriptToEvaluateOnNewDocument',{identifier});
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(listen),1);
    await page.viewport(1440,900);
  }
}
