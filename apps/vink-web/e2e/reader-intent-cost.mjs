// #239 control A: cost of the real navigation and actual reads during uncommitted intent.
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { recordTransport } from './web-request-ledger.mjs';

export async function readerIntentCost(page, {args, check}) {
  if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(args.base)) throw new Error('loopback scratch only');
  const url = args.base + '/?q=kiem%20thu';
  let phase = 'load';
  const ledger = recordTransport(page,args.base,()=>phase);
  const raw = { source_sha:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
    browser:await page.cdp.send('Browser.getVersion'), transport:ledger.rows, samples:[], status:'FAIL' };
  const flush = () => writeFileSync(join(dirname(args.state),'reader-intent-cost.json'),JSON.stringify(raw,null,2));
  try {
    await page.viewport(1280,900);
    for (let i=0;i<10;i++) {
      phase = 'load-' + i;
      await page.goto('about:blank');
      await page.send('Network.clearBrowserCache');
      await page.goto(url);
      await page.waitFor("document.documentElement.dataset.hydrated==='true' && document.querySelector('.vk-public-row__link')",'fresh guest discovery');
      const a = '.vk-public-row__link';
      const point = await page.eval(`(() => {const el=document.querySelector('${a}'); el.scrollIntoView({block:'center'});
        const r=el.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`);
      phase = 'abandon-' + i;
      if (i%2===0) await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',...point});
      else {
        await page.eval("document.querySelector('.vk-public-row__link').focus()");
        await page.key('Tab');
      }
      await new Promise(resolve=>setTimeout(resolve,300));
      await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:1,y:1});
      const attempted = ledger.rows.filter(r=>r.phase===phase && r.kind==='reader');
      check(!attempted.length,'no speculative Reader after an abandoned pointer/focus');
      await page.eval(`document.addEventListener('click', function start(e){
        if (!e.target.closest('.vk-public-row__link')) return;
        const t=performance.now();
        const observer=new MutationObserver(()=>{if(document.querySelector('.vk-reader .vk-document')){
          window.__intentArticle=performance.now()-t;observer.disconnect();}});
        observer.observe(document.body,{subtree:true,childList:true});
        document.removeEventListener('click',start,true);
      },true)`);
      phase = 'navigate-' + i;
      if (i%2===0) await page.click(a);
      else { await page.eval("document.querySelector('.vk-public-row__link').focus()"); await page.key('Enter'); }
      await page.waitFor("window.__intentArticle>0 && !!document.querySelector('.vk-document')",'real Reader mount');
      const sample=await page.eval(`(() => {
        const [r]=performance.getEntriesByType('resource').filter(e=>new URL(e.name).pathname.endsWith('/reader'));
        return {click_article_ms:window.__intentArticle,read_ms:r.duration,ttfb_ms:r.responseStart-r.requestStart,
          encoded_body_bytes:r.encodedBodySize,transfer_bytes:r.transferSize,decoded_body_bytes:r.decodedBodySize,
          paragraphs:document.querySelectorAll('.vk-document p').length};
      })()`);
      check(ledger.rows.filter(r=>r.phase===phase && r.kind==='reader').length===1 && sample.decoded_body_bytes>0,
        'one real public Reader on committed cold key');
      raw.samples.push({iteration:i,intent:i%2===0?'pointer':'focus/keyboard',...sample});
      flush();
    }
    raw.status='PASS';
  } finally { ledger.stop();flush(); }
}
