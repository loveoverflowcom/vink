// #229: an opt-in phase of the existing CDP harness. Only synthetic fixtures from the
// SQLx scratch database. Timings are observations, never mobile SLAs or CI thresholds.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { paragraphs } from './document.mjs';

export async function documentBudget(page, { args, check }) {
  const dir = dirname(args.state);
  const fixtures = JSON.parse(readFileSync(join(dir, 'fixtures.json'), 'utf8'));
  const results = { status: 'RUNNING', source_sha: execFileSync('git', ['rev-parse', 'HEAD'], {encoding:'utf8'}).trim(), boundary: 'desktop Chrome headless; synthetic CDP input; local debug WASM', reader: [], editor: [] };
  const flush = () => writeFileSync(join(dir, 'browser.json'), JSON.stringify(results, null, 2));
  const view = `document.querySelector('.cm-content').cmTile.root.view`;
  const ready = `document.documentElement.dataset.hydrated === 'true'`;
  const observer = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `
    window.__budgetHydratedAt=null;
    new MutationObserver(()=>{if(document.documentElement?.dataset.hydrated==='true' && window.__budgetHydratedAt===null)
      window.__budgetHydratedAt=performance.now();}).observe(document,{subtree:true,attributes:true,attributeFilter:['data-hydrated']});
  ` });
  await page.send('Performance.enable');
  await page.send('Network.enable', { maxPostDataSize: 8 * 1024 * 1024 });
  const requests = new Map(), transfer = [], writes = [];
  let lastPutRequestId;
  const listener = event => {
    if (event.sessionId !== page.sessionId) return;
    if (event.method === 'Page.javascriptDialogOpening' && event.params.type === 'beforeunload') {
      page.send('Page.handleJavaScriptDialog', { accept: true }).catch(() => {});
    }
    if (event.method === 'Network.requestWillBeSent' && event.params.request.method === 'PUT') {
      const r = event.params.request;
      lastPutRequestId = event.params.requestId;
      writes.push({path:new URL(r.url).pathname,post_data_bytes:r.postData === undefined ? null : Buffer.byteLength(r.postData)});
    }
    if (event.method === 'Network.responseReceived') requests.set(event.params.requestId, {
      type: event.params.type, url: new URL(event.params.response.url).pathname,
      status: event.params.response.status, fromDiskCache: !!event.params.response.fromDiskCache,
    });
    if (event.method === 'Network.loadingFinished' && requests.has(event.params.requestId)) {
      transfer.push({ ...requests.get(event.params.requestId), encodedDataLength: event.params.encodedDataLength });
      requests.delete(event.params.requestId);
    }
  };
  page.cdp.listeners.push(listener);
  try {
    results.browser = await page.cdp.send('Browser.getVersion');
    for (const fixture of fixtures) {
      const scenes = fixture.case.endsWith('-200000')
        ? [[1440, 100], [390, 100], [390, 200], [1440, 200]] : [[1440, 100]];
      for (const [width, scale] of scenes) {
        await page.viewport(width);
        // Text preference uses the real root font-size path, without changing production code.
        await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `
          try { localStorage.setItem('vink-text-size','${scale}'); } catch {}
          document.addEventListener('DOMContentLoaded',()=>document.documentElement.style.fontSize='${16 * scale / 100}px',{once:true});
        ` }).then(async injection => {
          transfer.length = 0;
          const start = performance.now();
          await page.goto(`${args.base}/tac-pham/${fixture.work_id}`);
          await page.waitFor(`${ready} && document.querySelector('.vk-reader .vk-document')`, 'budget reader', 120000);
          await page.eval('document.fonts.ready.then(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))))');
          const readyWallMs = performance.now() - start;
          const metrics = await page.eval(`(() => {
            const body=document.querySelector('.vk-reader .vk-document');
            const nav=performance.getEntriesByType('navigation')[0].toJSON();
            const scripts=[...document.scripts].filter(s=>!s.src);
            const bytes=s=>new TextEncoder().encode(s).length;
            return { navigation:nav, hydrated_flag_ms:window.__budgetHydratedAt,
              hydration_after_response_ms:window.__budgetHydratedAt-nav.responseEnd,
              blocks:body.children.length, body_text_scalars:[...body.textContent].length,
              inline_script_bytes:scripts.reduce((n,s)=>n+bytes(s.textContent),0),
              hydration_resource_script_bytes:scripts.filter(s=>s.textContent.includes('__RESOLVED_RESOURCES')).reduce((n,s)=>n+bytes(s.textContent),0),
              inline_script_sizes:scripts.map(s=>({id:s.id,type:s.type,bytes:bytes(s.textContent)})),
              dom_elements:document.querySelectorAll('*').length,
              font_size:getComputedStyle(document.documentElement).fontSize,
              horizontal_overflow:document.documentElement.scrollWidth>innerWidth+1,
              resources:performance.getEntriesByType('resource').map(r=>({name:new URL(r.name).pathname,
                duration:r.duration,encodedBodySize:r.encodedBodySize,decodedBodySize:r.decodedBodySize,transferSize:r.transferSize})) };
          })()`);
          const shown = paragraphs(fixture.document);
          const expectedScalars = shown.flatMap(b => b.lines).reduce((n, s) => n + [...s].length, 0)
            + shown.reduce((n, b) => n + b.lines.length - 1, 0);
          check(metrics.blocks === shown.length && metrics.body_text_scalars === expectedScalars,
            `${fixture.case}/${width}/${scale}: Reader renders every block/scalar`);
          const heap = await page.send('Runtime.getHeapUsage');
          const dom = await page.send('Memory.getDOMCounters');
          results.reader.push({ case: fixture.case, width, scale, ready_wall_ms: readyWallMs, ...metrics,
            v8_heap: heap, dom, network: [...transfer], cache_state: results.reader.length ? 'warm process and bundle cache; new document URL or reload' : 'fresh browser profile, cold bundle cache' });
          flush();
          if (fixture.case === 'poem_tabs-200000') {
            await page.capture(`budget-reader-${width}-${scale}`, { synthetic_corpus: fixture.case });
          }
          await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: injection.identifier });
        });
      }
      // Actual desk mount/load, CodeMirror virtual DOM, production getText and manual save.
      await page.viewport(1440);
      await page.goto(`${args.base}/ban-viet/${fixture.work_id}`);
      await page.waitFor(`${ready} && document.querySelector('.cm-content[contenteditable=true]')`, 'budget editor', 120000);
      const loadedHeap = await page.send('Runtime.getHeapUsage');
      const loadedDom = await page.send('Memory.getDOMCounters');
      const source = fixture.document.source;
      await page.eval(`window.__budgetExpected=${JSON.stringify(source)};window.__budgetView=${view};void 0`);
      check(await page.eval(`__budgetView.state.doc.toString()===__budgetExpected`), `${fixture.case}: actual loaded buffer retains every line`);
      const snapshots = await page.eval(`(async()=>{const m=await import('/pkg/vink-editor.js');return Array.from({length:5},()=>{
        const t=performance.now(),text=m.getText({view:__budgetView,destroyed:false});
        return {ms:performance.now()-t,utf8_bytes:new TextEncoder().encode(text).length,utf16_units:text.length};});})()`);
      const row = { case: fixture.case, snapshots, dom: loadedDom, v8_heap: loadedHeap, scenes: [] };
      if (fixture.case === 'emoji-200000') {
        row.oversized_cache = await page.eval(String.raw`(async()=>{
          const bundle=await(await fetch('/pkg/vink.js')).text();
          const path=bundle.match(/['"](\.\/snippets\/[^'"]+\/public-cache\/indexeddb.js)['"]/)[1];
          const pc=await import(new URL(path,location.origin+'/pkg/vink.js'));
          const notOpened=!(await indexedDB.databases()).some(d=>d.name===pc.DATABASE);
          const work=location.pathname.split('/').pop();
          const publication=await fetch('/api/v1/works/'+work+'/publication').then(r=>r.json());
          const text=JSON.stringify({schema:1,origin:location.origin,policy:'public-snapshot-v1',key:{Reader:work},
            provenance:'origin_response',validated_at_ms:Date.now(),payload:{kind:'published',data:publication}});
          const adapters=[];
          for(const Adapter of [pc.MemoryStorage,pc.NoOpStorage,pc.IndexedDbStorage]) {
            const storage=new Adapter(),ticket=await storage.run({kind:'capture'});
            const ack=await storage.run({kind:'write',ticket,text});
            const restored=await storage.run({kind:'read',ticket,key:pc.keyOf({Reader:work})});
            adapters.push({adapter:Adapter.name,durable_ack:ack===true,miss:restored===null});storage.close?.();
          }
          return {production_gate_kept_closed:notOpened,bytes:new TextEncoder().encode(text).length,
            record_limit:pc.LIMITS.record,decode_miss:pc.decode(text)===null,adapters};
        })()`);
        check(row.oversized_cache.production_gate_kept_closed && row.oversized_cache.decode_miss
          && row.oversized_cache.adapters.every(a=>!a.durable_ack && a.miss), 'oversized valid public record: closed rollout gate, cache miss, no fake durable ACK');
      }
      for (const [width, scale] of [[1440, 100], [390, 100], [390, 200], [1440, 200]]) {
        await page.viewport(width);
        const scene = await page.eval(`(async()=>{document.documentElement.style.fontSize='${16 * scale / 100}px';
          const v=__budgetView,mid=Math.floor(v.state.doc.length/2);v.focus();
          await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
          v.dispatch({selection:{anchor:mid,head:mid+1}});
          await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
          const held=v.state.selection.toJSON(),raw=v.state.doc.toString();
          const options=document.querySelector('#work-options');
          options.open=!options.open;await new Promise(r=>requestAnimationFrame(r));
          options.open=!options.open;await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
          return {same_view:v===${view},same_text:raw===v.state.doc.toString(),same_selection:JSON.stringify(held)===JSON.stringify(v.state.selection.toJSON()),
            rendered_cm_lines:document.querySelectorAll('.cm-line').length,total_lines:v.state.doc.lines,
            horizontal_overflow:document.documentElement.scrollWidth>innerWidth+1};})()`);
        check(scene.same_view && scene.same_text && scene.same_selection, `${fixture.case}/${width}/${scale}: article options retain buffer, view and selection anchor`);
        row.scenes.push({ width, scale, ...scene });
      }
      // A real server save, followed by an intentionally stale version. Rejected requests must
      // leave the live buffer available. No transport response is mocked or intercepted here.
      await page.eval(`__budgetView.focus();__budgetView.dispatch({selection:{anchor:0}})`);
      await page.send('Input.imeSetComposition', { text: 'ư', selectionStart: 1, selectionEnd: 1 });
      await page.send('Input.insertText', { text: 'ư' });
      await page.waitFor(`!${view}.composing`, 'composition commit');
      // Remove one original scalar to keep the already maximal corpus at its domain boundary.
      await page.eval(`(() => {const v=__budgetView;const from=1;const cp=v.state.doc.sliceString(from).codePointAt(0);v.dispatch({changes:{from,to:from+(cp>65535?2:1),insert:''}});})()`);
      await page.typeAtEnd('#draft-title', '  ');
      const before = await page.draft(fixture.work_id);
      await page.eval('window.__budgetBeforeSave=__budgetView.state.doc.toString()');
      const saveStart = performance.now();
      const priorWrites = writes.length;
      await page.click('#save');
      await page.waitFor(`['saved','failed','conflict'].includes(document.querySelector('[data-testid=save-status]')?.dataset.status)`, 'budget save result', 120000);
      row.manual_save_wall_ms = performance.now() - saveStart;
      row.save_status = await page.status();
      row.manual_save_requests = writes.slice(priorWrites);
      row.ime_simulation = 'CDP composition followed by committed insertion';
      const after = await page.draft(fixture.work_id);
      check(after.version === before.version + 1 && row.save_status === 'saved',
        `${fixture.case}: valid document receives ACK and advances server version`);
      const liveSource = await page.eval('__budgetBeforeSave');
      check(after.document.format === 'default' && after.document.source === liveSource,
        `${fixture.case}: exact source including NFD survives manual save`);
      check(await page.eval(`__budgetView===${view} && __budgetView.state.doc.toString()===__budgetBeforeSave`), `${fixture.case}: exact live buffer survives save outcome`);
      check(row.manual_save_requests.length === 1 && row.manual_save_requests[0].path === `/api/v1/drafts/${fixture.work_id}`,
        `${fixture.case}: manual save sends one draft-content PUT`);
      const { postData } = await page.send('Network.getRequestPostData', { requestId: lastPutRequestId });
      const sent = JSON.parse(postData);
      check(sent.document.source === liveSource && sent.document.format === 'default',
        `${fixture.case}: request carries the exact editor source`);
      check(sent.title === before.title && after.title === before.title
        && await page.eval(`document.querySelector('#draft-title').value`) === before.title + '  ',
        `${fixture.case}: request and stored title omit padding while the live input retains it`);
      if (fixture.case === 'tiny_blocks-200000') {
        await page.eval(`(()=>{const v=__budgetView;v.dispatch({changes:{from:v.state.doc.length,insert:'X'}});window.__budgetRejected=v.state.doc.toString();})()`);
        await page.click('#save'); await page.waitStatus('failed');
        check(await page.eval(`__budgetView.state.doc.toString()===__budgetRejected`), 'domain refusal retains the full 200001-scalar live buffer');
        check(JSON.stringify(await page.draft(fixture.work_id)) === JSON.stringify(after), 'failed manual save keeps exact durable document/version');
        await page.eval(`(()=>{const v=__budgetView;v.dispatch({changes:{from:v.state.doc.length-1,to:v.state.doc.length,insert:''}});})()`);
        row.failed_save_preserves_buffer = true;
      }
      row.stale_status = await page.eval(`(async()=>{const r=await fetch('/api/v1/drafts/${fixture.work_id}',{method:'PUT',headers:{'content-type':'application/json'},
        body:JSON.stringify({expected_version:1,title:'[Mẫu #229]',document:{format:'default',source:'Xung đột thử nghiệm'}})});return r.status;})()`);
      check(row.stale_status === 409, `${fixture.case}: stale saved-version remains 409`);
      {
        const external = await page.eval(`(async()=>{const d=await fetch('/api/v1/drafts/${fixture.work_id}').then(r=>r.json());
          const r=await fetch('/api/v1/drafts/${fixture.work_id}',{method:'PUT',headers:{'content-type':'application/json'},
            body:JSON.stringify({expected_version:d.version,title:d.title,document:d.document})});return r.status;})()`);
        check(external === 200, 'a second writer advances the durable version');
        await page.eval(`(()=>{const v=__budgetView;v.dispatch({changes:{from:0,to:1,insert:'Z'}});window.__budgetBeforeSave=v.state.doc.toString();})()`);
        await page.click('#save');await page.waitStatus('conflict');
        check(await page.eval(`__budgetView.state.doc.toString()===__budgetBeforeSave`), `${fixture.case}: UI conflict preserves exact unsaved text`);
        row.ui_conflict_preserves_buffer = true;
      }
      row.final_v8_heap = await page.send('Runtime.getHeapUsage');
      row.final_heap_includes_test_expected_and_preserved_buffer_copies = true;
      results.editor.push(row);flush();
      // The listener accepts native beforeunload only in this synthetic scratch context.
    }
    results.status = 'PASS';
    results.not_run = ['physical mobile hardware', 'OS Telex/VNI IME', 'Safari/Firefox', 'browser process RSS/total WASM memory', 'OS/PostgreSQL page-cache cold restart'];
    flush();
  } finally {
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener), 1);
    await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: observer.identifier });
  }
}
