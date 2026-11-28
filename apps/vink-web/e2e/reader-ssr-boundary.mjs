// #235: progressive enhancement on the real App/PG fixture through the existing CDP Page.
// Only bootstrap assets are held or failed; publication authority stays with the real App.
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { assertNoHydrationRead, recordTransport } from './web-request-ledger.mjs';

const WORK = '33333333-3333-4333-8333-333333333333';
const PATH = '/tac-pham/' + WORK;
const TEXT = 'Phố cũ mưa giăng sợi mỏng,';
const READY = "document.documentElement.dataset.hydrated==='true'";
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

export async function readerBeforeHydration(page, { args, check }) {
  console.log('#235 Reader before hydration (real App/PG, blocked or held bootstrap assets):');
  let phase = 'blocked-bootstrap';
  const ledger = recordTransport(page, args.base, () => phase);
  const raw = { source_sha: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    dirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim(),
    boundary: 'real App/PostgreSQL; CDP bootstrap and WASM-only failure/delay; direct local origin',
    build_profile: args['build-profile'] ?? 'unknown',
    browser: await page.cdp.send('Browser.getVersion'), status: 'FAIL', phases: [], transport: ledger.rows };
  const errorsBefore = page.errors.length;
  const blockedImport = 'TypeError: Failed to fetch dynamically imported module: ' + args.base + '/pkg/vink.js';
  const assets = [args.base + '/pkg/vink.js*', args.base + '/pkg/vink_bg.wasm*'];
  const held = new Set();
  const bootstrap = message => {
    if (message.sessionId === page.sessionId && message.method === 'Fetch.requestPaused') held.add(message.params.requestId);
  };
  const flush = () => writeFileSync(join(dirname(args.state), 'reader-ssr-boundary.json'), JSON.stringify(raw, null, 2));
  const noMutation = label => check(!raw.transport.some(r => r.phase === phase && /^(POST|PUT|PATCH|DELETE)$/.test(r.method)), label);
  const assertUnavailable = async () => check(await page.eval(`!(${READY}) &&
    document.querySelector('.vk-document')?.textContent.includes(${JSON.stringify(TEXT)}) &&
    !!document.querySelector('#reader-return[href]') &&
    document.querySelector('#root-composer-send')?.getAttribute('aria-disabled')==='true' && !document.querySelector('#discussion-refresh')`),
  'complete SSR article, return link and unavailable client actions during ' + phase);
  try {
    await page.send('Network.setBlockedURLs', { urls: assets });
    await page.goto(args.base + PATH);
    check(await page.eval(`!(${READY}) && document.querySelector('.vk-document')?.textContent.includes(${JSON.stringify(TEXT)})`), 'failed bootstrap retains the complete SSR article');
    check(await page.eval(`document.querySelector('link[rel=canonical]')?.href===${JSON.stringify(args.base + PATH)} && !!document.querySelector('meta[name=description]') && !document.documentElement.outerHTML.includes('Khổ đang viết dở — chưa công khai.')`), 'unhydrated document retains canonical/description and excludes the private draft');
    check(await page.eval("document.querySelector('#root-composer-send')?.getAttribute('aria-disabled')==='true' && document.querySelector('#root-composer-send')?.type==='button'"), 'unready composer send is unavailable and has no native form submission');
    check(await page.eval("!document.querySelector('#discussion-refresh')"), 'healthy SSR has no discussion refresh control');
    await page.click('#root-composer-send');
    noMutation('clicks before bootstrap do not submit a mutation');
    await page.click('#reader-return');
    await page.waitFor("location.pathname==='/' && !!document.querySelector('#public-query')", 'native Reader return without WASM');
    check(await page.eval(`!(${READY}) && document.querySelector('#public-search')?.method==='get'`), 'native Reader return reaches the real GET search without hydration');
    await page.typeAtEnd('#public-query', 'Mưa');
    await page.eval("document.querySelector('#public-search').requestSubmit()");
    await page.waitFor("new URL(location.href).searchParams.get('q')==='Mưa' && document.readyState==='complete'", 'native GET search before hydration');
    check(await page.eval(`!(${READY}) && !!document.querySelector('.vk-public-row__link')`), 'native GET search renders results without the bootstrap');
    // As in the existing brand bootstrap checks, Leptos reports one rejected import per
    // blocked document. Account for precisely those errors; preserve any unexpected error.
    for (const deadline = Date.now() + 5000; page.errors.length < errorsBefore + 3 && Date.now() < deadline;) await sleep(50);
    const blockedErrors = page.errors.slice(errorsBefore);
    check(blockedErrors.length === 3 && blockedErrors.every(error => error === blockedImport),
      'exactly the three intentionally blocked document imports fail; no unrelated error is ignored');
    page.errors.splice(errorsBefore, 3);
    raw.expected_bootstrap_errors = blockedErrors;
    raw.phases.push({ phase, main_text: 'PASS', links_get_search: 'PASS', private_draft_absent: 'PASS', mutation: 'none' });
    await page.send('Network.setBlockedURLs', { urls: [] });

    page.cdp.listeners.push(bootstrap);
    for (const holdMs of [200, 1000]) {
      phase = 'held-bootstrap-' + holdMs;
      await page.goto('about:blank');
      await page.send('Network.clearBrowserCache');
      await page.send('Fetch.enable', { patterns: assets.map(urlPattern => ({ urlPattern, requestStage: 'Request' })) });
      await page.send('Page.navigate', { url: args.base + PATH });
      await page.waitFor("!!document.querySelector('.vk-document') && !!document.querySelector('#root-composer-send')", 'complete SSR Reader during bootstrap hold');
      // SSR DOM can become observable before the deferred import reaches Fetch. Wait for
      // the actual paused request; DOM readiness alone is not proof of an injected hold.
      for (const deadline = Date.now() + 10000; held.size === 0 && Date.now() < deadline;) await sleep(10);
      check(held.size > 0 && await page.eval(`!(${READY})`), 'bootstrap is actually paused before the click (' + holdMs + 'ms)');
      check(await page.eval("!document.querySelector('#discussion-refresh')"), 'healthy discussion has no refresh control while bootstrap is held (' + holdMs + 'ms)');
      await page.click('#root-composer-send');
      if (holdMs === 1000) {
        await page.typeAtEnd('#root-composer', 'Mưa đầu mùa 👩‍💻');
        await page.key('ArrowLeft', 8); // Shift-select one grapheme through Chrome.
        await page.eval(`window.coldDraft = document.activeElement;
          window.coldSelection = [coldDraft.selectionStart, coldDraft.selectionEnd]`);
      }
      await sleep(holdMs);
      noMutation('no mutation while bootstrap is held (' + holdMs + 'ms)');
      // Disable interception before continuing: modules/WASM requested afterwards flow normally.
      await page.send('Fetch.disable');
      held.clear();
      await page.waitFor(READY, 'hydration after held bootstrap', 120000);
      await page.waitFor("document.querySelector('#comments-mode')?.getAttribute('aria-busy')==='false' && !document.querySelector('#discussion-refresh')", 'discussion stays free of generic refresh after hydration');
      await page.eval('new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))');
      await sleep(300);
      if (holdMs === 1000) {
        const kept = await page.eval(`({
          node: document.querySelector('#root-composer') === coldDraft,
          focus: document.activeElement === coldDraft,
          text: coldDraft.value === 'Mưa đầu mùa 👩‍💻',
          selection: coldDraft.selectionStart === coldSelection[0] && coldDraft.selectionEnd === coldSelection[1],
          adopted: document.querySelector('#root-composer-format').hidden
        })`);
        check(Object.values(kept).every(Boolean),
          'hydration adopts native text and keeps node, focus and selection ' + JSON.stringify(kept));
      }
      noMutation('hydration does not replay a pre-hydration click (' + holdMs + 'ms)');
      assertNoHydrationRead(raw.transport, phase, 'reader');
      raw.phases.push({ phase, injected_hold_ms: holdMs, hydrated: 'PASS', mutation_replay: 'none', fresh_reader_gets: 0,
        observation_after_hydration_ms: 300 });
      flush();
    }

    // The document preloads WASM, so requestPaused alone can predate the JS module. Observe
    // the loader's explicit fetch without changing the request or the returned Promise.
    const observer = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `(() => {
      const fetch = window.fetch;
      window.fetch = function (input, ...rest) {
        if (typeof input === 'string' && new URL(input, location.href).pathname === '/pkg/vink_bg.wasm') {
          window.__vinkWasmInitStarted = true;
        }
        return fetch.call(this, input, ...rest);
      };
    })()` });
    try {
      for (const holdMs of [200, 1000, null]) {
        phase = holdMs === null ? 'wasm-only-503' : 'wasm-only-hold-' + holdMs;
        await page.goto('about:blank');
        await page.send('Network.clearBrowserCache');
        await page.send('Fetch.enable', { patterns: [{ urlPattern: assets[1], requestStage: 'Request' }] });
        const errorsAtStart = page.errors.length;
        await page.send('Page.navigate', { url: args.base + PATH });
        await page.waitFor('!!window.__vinkWasmInitStarted && !!document.querySelector("#reader-paper")',
          'loaded JS module enters the WASM loader');
        check(held.size > 0 && raw.transport.some(r => r.phase === phase && r.path === '/pkg/vink.js' &&
          r.response_status === 200 && r.finished_s != null), 'JS loads successfully while only WASM is paused');
        await assertUnavailable();
        await page.click('#root-composer-send');
        if (holdMs === null) {
          for (const requestId of held) {
            await page.send('Fetch.fulfillRequest', { requestId, responseCode: 503,
              responseHeaders: [{ name: 'Content-Type', value: 'application/wasm' }, { name: 'Cache-Control', value: 'no-store' }],
              body: '' });
          }
          for (const deadline = Date.now() + 5000; page.errors.length === errorsAtStart && Date.now() < deadline;) await sleep(20);
          const errors = page.errors.slice(errorsAtStart);
          check(errors.length === 1 && errors[0].includes('failed to fetch Wasm: 503') &&
            errors[0].includes(args.base + '/pkg/vink_bg.wasm'), 'only the expected wasm-bindgen 503 reaches Runtime.exceptionThrown');
          page.errors.splice(errorsAtStart, 1);
          await assertUnavailable();
          noMutation('WASM init failure leaves native HTML and submits no mutation');
          assertNoHydrationRead(raw.transport, phase, 'reader');
          raw.phases.push({ phase, wasm_status: 503, js_loaded: true, loader_entered: true, hydrated: false,
            main_text: 'PASS', mutation: 'none', fresh_reader_gets: 0 });
          await page.send('Fetch.disable');
        } else {
          await sleep(holdMs);
          await assertUnavailable();
          noMutation('WASM hold submits no mutation');
          await page.send('Fetch.disable');
          await page.waitFor(READY + " && document.querySelector('#comments-mode')?.getAttribute('aria-busy')==='false' && !document.querySelector('#discussion-refresh')",
            'release of WASM hydrates the discussion', 120000);
          await sleep(300);
          noMutation('WASM recovery does not replay pre-hydration clicks');
          assertNoHydrationRead(raw.transport, phase, 'reader');
          raw.phases.push({ phase, injected_hold_ms: holdMs, js_loaded: true, loader_entered: true,
            hydrated: true, mutation_replay: 'none', fresh_reader_gets: 0, observation_after_hydration_ms: 300 });
        }
        held.clear();
        flush();
      }
    } finally {
      await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: observer.identifier });
    }
    raw.status = 'PASS';
  } finally {
    await page.send('Network.setBlockedURLs', { urls: [] });
    await page.send('Fetch.disable');
    ledger.stop();
    page.cdp.listeners = page.cdp.listeners.filter(listener => listener !== bootstrap);
    flush();
  }
}

// Companion guest run on the same canonical hot work as #234.
export async function readerGuestHistory(page, {args,check}) {
  if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(args.base)) throw new Error('loopback scratch only');
  const reader='/tac-pham/e4202094-9563-02e5-519d-6f8fa6a1af4c';
  const article="document.querySelector('.vk-document')?.textContent.includes('Một dòng công khai dùng riêng')";
  let phase='javascript-disabled';
  const ledger=recordTransport(page,args.base,()=>phase);
  const raw={source_sha:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
    browser:await page.cdp.send('Browser.getVersion'),transport:ledger.rows,status:'FAIL'};
  try {
    await page.send('Network.setBlockedURLs',{urls:[args.base+'/pkg/vink.js*',args.base+'/pkg/vink_bg.wasm*']});
    await page.send('Emulation.setScriptExecutionDisabled',{value:true});
    await page.goto(args.base+reader);
    check(await page.eval(`${article} && !(${READY}) &&
      !document.querySelector('#discussion-refresh') &&
      document.querySelector('link[rel=canonical]')?.href===${JSON.stringify(args.base+reader)}`),'full canonical Reader with JS disabled');
    await page.eval("document.querySelector('#reader-return').focus()");
    const enter=async()=>{for(const type of ['keyDown','keyUp'])await page.send('Input.dispatchKeyEvent',{
      type,key:'Enter',code:'Enter',windowsVirtualKeyCode:13,...(type==='keyDown'?{text:'\r'}:{})});};
    await enter();
    await page.waitFor("location.pathname==='/' && document.querySelector('#public-search')?.method==='get'",'native Reader return');
    await page.eval("document.querySelector('#public-query').focus()");
    await page.send('Input.insertText',{text:'kiem thu'});
    await enter();
    await page.waitFor("new URL(location.href).searchParams.get('q')==='kiem thu' && document.querySelectorAll('.vk-public-row').length>0",'native GET results');
    raw.no_js=true;
    await page.send('Emulation.setScriptExecutionDisabled',{value:false});
    await page.send('Network.setBlockedURLs',{urls:[]});
    phase='fresh-reader';
    await page.goto(args.base+reader);
    await page.waitFor(`${READY} && ${article}`,'fresh guest Reader');
    await sleep(300);
    assertNoHydrationRead(raw.transport,phase,'reader');
    const start=await page.eval('performance.timeOrigin');
    await page.goto(args.base+'/toi');
    await page.waitFor(READY,'leave guest Reader document');
    phase='history-reader';
    await page.eval('history.back()');
    await page.waitFor(`${READY} && ${article} && performance.timeOrigin!==${start} && performance.getEntriesByType('navigation')[0].type==='back_forward'`, 'unknown-age Reader document');
    await page.waitFor("performance.getEntriesByType('resource').some(r=>new URL(r.name).pathname.endsWith('/reader')) && document.querySelector('#reader-freshness')?.dataset.refreshing==='false'", 'Reader revalidated');
    check(raw.transport.filter(r=>r.phase===phase && r.kind==='reader').length===1,'one history Reader verification');
    raw.history_verifications=1;
    raw.status='PASS';
  }finally{
    await page.send('Emulation.setScriptExecutionDisabled',{value:false});
    await page.send('Network.setBlockedURLs',{urls:[]});ledger.stop();
    writeFileSync(join(dirname(args.state),'reader-guest-history.json'),JSON.stringify(raw,null,2));
  }
}
