// #229: an opt-in phase of the existing CDP harness. Only synthetic fixtures from the
// SQLx scratch database. Timings are observations, never mobile SLAs or CI thresholds.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

export async function documentBudget(page, { args, check }) {
  const dir = dirname(args.state);
  const fixtures = JSON.parse(readFileSync(join(dir, 'fixtures.json'), 'utf8'));
  const results = { status: 'RUNNING', boundary: 'desktop Chrome headless; synthetic CDP input; local debug WASM', reader: [], editor: [] };
  const flush = () => writeFileSync(join(dir, 'browser.json'), JSON.stringify(results, null, 2));
  const view = `document.querySelector('.cm-content').cmTile.root.view`;
  const ready = `document.documentElement.dataset.hydrated === 'true'`;
  const observer = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `
    window.__budgetHydratedAt=null;
    new MutationObserver(()=>{if(document.documentElement?.dataset.hydrated==='true' && window.__budgetHydratedAt===null)
      window.__budgetHydratedAt=performance.now();}).observe(document,{subtree:true,attributes:true,attributeFilter:['data-hydrated']});
  ` });
  await page.send('Performance.enable');
  const requests = new Map(), transfer = [];
  const listener = event => {
    if (event.sessionId !== page.sessionId) return;
    if (event.method === 'Page.javascriptDialogOpening' && event.params.type === 'beforeunload') {
      page.send('Page.handleJavaScriptDialog', { accept: true }).catch(() => {});
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
              inline_script_sizes:scripts.map(s=>({id:s.id,type:s.type,bytes:bytes(s.textContent)})),
              dom_elements:document.querySelectorAll('*').length,
              font_size:getComputedStyle(document.documentElement).fontSize,
              horizontal_overflow:document.documentElement.scrollWidth>innerWidth+1,
              resources:performance.getEntriesByType('resource').map(r=>({name:new URL(r.name).pathname,
                duration:r.duration,encodedBodySize:r.encodedBodySize,decodedBodySize:r.decodedBodySize,transferSize:r.transferSize})) };
          })()`);
          const expectedScalars = fixture.document.blocks.flatMap(b => b.lines).reduce((n, s) => n + [...s].length, 0)
            + fixture.document.blocks.reduce((n, b) => n + b.lines.length - 1, 0);
          check(metrics.blocks === fixture.document.blocks.length && metrics.body_text_scalars === expectedScalars,
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
      const source = fixture.document.blocks.map(b => b.lines.join('\n')).join('\n\n');
      await page.eval(`window.__budgetExpected=${JSON.stringify(source)};window.__budgetView=${view}`);
      check(await page.eval(`__budgetView.state.doc.toString()===__budgetExpected`), `${fixture.case}: actual loaded buffer retains every line`);
      const snapshots = await page.eval(`(async()=>{const m=await import('/pkg/vink-editor.js');return Array.from({length:5},()=>{
        const t=performance.now(),text=m.getText({view:__budgetView,destroyed:false});
        return {ms:performance.now()-t,utf8_bytes:new TextEncoder().encode(text).length,utf16_units:text.length};});})()`);
      const row = { case: fixture.case, snapshots, dom: await page.send('Memory.getDOMCounters'), v8_heap: await page.send('Runtime.getHeapUsage'), scenes: [] };
      for (const [width, scale] of [[1440, 100], [390, 100], [390, 200], [1440, 200]]) {
        await page.viewport(width);
        const scene = await page.eval(`(async()=>{document.documentElement.style.fontSize='${16 * scale / 100}px';
          const v=__budgetView,mid=Math.floor(v.state.doc.length/2);v.dispatch({selection:{anchor:mid,head:mid+1}});
          await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
          const held=v.state.selection.toJSON(),raw=v.state.doc.toString();
          document.querySelector('#preview').click();await new Promise(r=>requestAnimationFrame(r));
          document.querySelector('#draft-preview').close();await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
          return {same_view:v===${view},same_text:raw===v.state.doc.toString(),same_selection:JSON.stringify(held)===JSON.stringify(v.state.selection.toJSON()),
            rendered_cm_lines:document.querySelectorAll('.cm-line').length,total_lines:v.state.doc.lines,
            horizontal_overflow:document.documentElement.scrollWidth>innerWidth+1};})()`);
        check(scene.same_view && scene.same_text && scene.same_selection, `${fixture.case}/${width}/${scale}: preview retains buffer, view and selection anchor`);
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
      const before = await page.draft(fixture.work_id);
      await page.eval('window.__budgetBeforeSave=__budgetView.state.doc.toString()');
      const saveStart = performance.now();
      await page.click('#save');
      const tooLarge = Buffer.byteLength(JSON.stringify({ expected_version: before.version, title: before.title, document: fixture.document })) > 2 * 1024 * 1024;
      await page.waitFor(`['saved','failed','conflict'].includes(document.querySelector('[data-testid=save-status]')?.dataset.status)`, 'budget save result', 120000);
      row.manual_save_wall_ms = performance.now() - saveStart;
      row.save_status = await page.status();
      const after = await page.draft(fixture.work_id);
      check(tooLarge ? after.version === before.version : after.version === before.version + 1,
        `${fixture.case}: manual save ${tooLarge ? 'transport rejection preserves server version' : 'ACK advances server version'}`);
      check(await page.eval(`__budgetView===${view} && __budgetView.state.doc.toString()===__budgetBeforeSave`), `${fixture.case}: exact live buffer survives save outcome`);
      row.stale_status = await page.eval(`(async()=>{const r=await fetch('/api/v1/drafts/${fixture.work_id}',{method:'PUT',headers:{'content-type':'application/json'},
        body:JSON.stringify({expected_version:1,title:'[Mẫu #229]',document:{schema_version:1,blocks:[{lines:['Xung đột thử nghiệm']}]}})});return r.status;})()`);
      check(row.stale_status === 409, `${fixture.case}: stale saved-version remains 409`);
      if (!tooLarge) {
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
