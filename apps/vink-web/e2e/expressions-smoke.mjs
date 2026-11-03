// Uses the existing CDP host and real hydrated Rust policy. No simulated playback reducer.
export async function expressionPilot(page, args, check) {
  const url = `${args.base}/thu-nghiem/bieu-cam`;
  const sample = 'Một câu thơ còn ấm ❤️‍🔥\n  lời cảm ơn vẫn nguyên vẹn 🙏 👍🏽\n\nTrang sách có cả 👩‍👩‍👧‍👦, 🇻🇳 và 1️⃣.';
  const requests = [];
  const record = m => { if (m.sessionId === page.sessionId && m.method === 'Network.requestWillBeSent') requests.push(m.params.request.url); };
  page.cdp.listeners.push(record);
  const engineRequest = u => /\/expressions\/runtime\//.test(u);
  const cell = i => `[data-expression-index="${i}"]`;
  const phase = i => `document.querySelector('${cell(i)}').dataset.phase`;
  const active = `Number(document.querySelector('#expression-pilot').dataset.activePlayers)`;
  async function open() {
    await page.goto(url);
    await page.waitFor(`document.querySelector('#expression-pilot')?.dataset.ready === 'true'`, 'expression controller');
    await page.eval(`scrollTo(0, 0)`);
  }
  async function holdAnimations() {
    const held = [];
    const listener = m => { if (m.sessionId === page.sessionId && m.method === 'Fetch.requestPaused') held.push(m.params.requestId); };
    page.cdp.listeners.push(listener);
    await page.send('Fetch.enable', { patterns: [{ urlPattern: '*/pkg/expressions/assets/*.json', requestStage: 'Request' }] });
    return async () => {
      for (const requestId of held) { try { await page.send('Fetch.continueRequest', { requestId }); } catch { /* cancelled fetch has no live interception */ } }
      await page.send('Fetch.disable');
      page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener), 1);
    };
  }
  async function click(index) { await page.click(`#expression-${index}`); }
  async function reduce(value) {
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: value ? 'reduce' : 'no-preference' }] });
  }
  console.log('Noto expressions: exact Unicode, static SSR, lazy real runtime and Rust lifecycle:');
  try {
    await page.send('Network.setCacheDisabled', { cacheDisabled: true });
    await page.viewport(1440, 1200);
    const html = await fetch(url).then(r => r.text());
    check((html.match(/data-expression-index=/g) ?? []).length === 16 && !html.includes('<canvas'), 'pilot SSR contains sixteen static named assets and no canvas');
    check(html.includes(sample) && !html.includes('/expressions/runtime/'), 'SSR preserves ZWJ/selector/modifier/flag/keycap source and contains no animation engine');
    for (const route of ['/', '/tac-pham/33333333-3333-4333-8333-333333333333']) {
      await page.goto(`${args.base}${route}`);
      await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'static route hydration');
    }
    check(!requests.some(engineRequest) && !requests.some(u => u.endsWith('/expressions/pilot.js')), 'plain Explore/reader do not request animation engine, WASM or pilot controller');

    await page.send('Emulation.setScriptExecutionDisabled', { value: true });
    await page.goto(url);
    check(await page.eval(`document.querySelectorAll('#emoji-catalog img').length === 16 && !document.querySelector('canvas') && document.querySelector('#expression-pilot').dataset.ready === 'false'`), 'actual no-JS pilot retains static artwork');
    await page.capture('expressions-no-js-1440-light');
    await page.send('Emulation.setScriptExecutionDisabled', { value: false });
    await open();
    await page.waitFor(`[...document.querySelectorAll('#emoji-catalog img')].every(i => i.complete && i.naturalWidth > 0)`, 'static Google SVG decoding');
    check(!requests.some(engineRequest), 'hydration and all static images leave animation engine/WASM unloaded');
    check(await page.eval(`document.querySelector('#unicode-sample .vk-expression-poem').textContent === ${JSON.stringify(sample)}`), 'hydration keeps the exact original Unicode and stanza layout');

    // Viewport, inherited tokens and 200% text, without introducing new state semantics.
    for (const width of [320, 390, 834, 1024, 1440]) {
      for (const theme of ['light', 'dark']) {
        await page.viewport(width, 1100);
        await page.eval(`document.documentElement.dataset.theme = '${theme}'; document.documentElement.style.fontSize = '${width === 320 ? '200%' : '100%'}'`);
        await page.eval(`new Promise(r => setTimeout(r, 300))`);
        check(await page.eval(`document.documentElement.scrollWidth <= innerWidth + 1`), `expression reflow ${width}px ${theme}${width === 320 ? ' 200%' : ''}`);
        check(await page.eval(`[...document.querySelectorAll('#emoji-catalog button')].every(b => { const r = b.getBoundingClientRect(); return r.width >= 44 && r.height >= 44 && b.scrollWidth <= b.clientWidth + 1; })`), `expression named targets ${width}px ${theme}`);
        const contrasts = await page.eval(`(() => {
          const linear = c => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
          const luminance = color => color.match(/[0-9.]+/g).slice(0, 3).map(n => linear(Number(n) / 255)).reduce((s, c, i) => s + c * [0.2126, 0.7152, 0.0722][i], 0);
          const ratio = (a, b) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
          const button = getComputedStyle(document.querySelector('#expression-0'));
          const cell = getComputedStyle(document.querySelector('[data-expression-index]'));
          const status = getComputedStyle(document.querySelector('[data-player-status]'));
          return [ratio(luminance(button.color), luminance(button.backgroundColor)), ratio(luminance(status.color), luminance(cell.backgroundColor))];
        })()`);
        check(contrasts.every(c => c >= 4.5), `expression text contrast ${theme}: ${contrasts.map(c => c.toFixed(2)).join(', ')}`);
        await page.capture(`expressions-${width}-${theme}${width === 320 ? '-200' : ''}`);
        if (width === 320) {
          await page.eval(`document.querySelector('#emoji-catalog').scrollIntoView({block:'start'});`);
          await page.capture(`expressions-320-${theme}-200-catalog`);
          await page.eval('scrollTo(0, 0)');
        }
      }
    }
    await page.viewport(1440, 1200);
    await open();
    await page.eval(`document.querySelector('#expression-0').click()`);
    check(await page.eval(`${active} === 0`) && !requests.some(engineRequest), 'untrusted scripted click cannot load the animation engine');
    await page.eval(`delete document.documentElement.dataset.theme`);
    const lightSurface = await page.eval('getComputedStyle(document.body).backgroundColor');
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] });
    check(await page.eval('getComputedStyle(document.body).backgroundColor') !== lightSurface, 'system theme follows OS dark without loading an animation');
    await page.eval(`new Promise(r => setTimeout(r, 300))`);
    await page.capture('expressions-system-dark-1440');
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
    await page.eval(`new Promise(r => setTimeout(r, 300))`);
    await page.eval(`document.querySelector('#expression-0').focus()`);
    await page.key('Enter');
    await page.waitFor(`${phase(0)} === 'playing'`, 'keyboard one-shot real dotLottie playback');
    await page.waitFor(`Number(document.querySelector('${cell(0)}').dataset.frames) >= 3`, 'actual software renderer frames');
    check(requests.some(u => /\/runtime\/dotlottie-[a-f0-9]+\.js/.test(u)) && requests.some(u => u.endsWith('.wasm') && engineRequest(u)), 'permitted keyboard activation lazily fetched self-hosted engine and matching WASM');
    await page.capture('expressions-playing-1440-light');
    await page.waitFor(`${phase(0)} === 'static' && ${active} === 0`, 'one-shot completion and slot release', 20000);
    check(await page.eval(`!document.querySelector('canvas') && document.activeElement.id === 'expression-0'`), 'completion destroys the canvas and retains keyboard focus');

    const point = await page.eval(`(() => { const b = document.querySelector('#expression-1'); b.scrollIntoView({block:'center'}); const r=b.getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2}; })()`);
    await page.send('Emulation.setTouchEmulationEnabled', { enabled: true });
    await page.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [point] });
    await page.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await page.waitFor(`${phase(1)} === 'playing'`, 'touch one-shot activation');
    await page.send('Emulation.setTouchEmulationEnabled', { enabled: false });
    await reduce(true);
    await page.waitFor(`${active} === 0 && !document.querySelector('canvas')`, 'reduced motion cancels active player');
    await reduce(false);
    check(await page.eval(`${active} === 0`), 'restoring motion does not autoplay');

    await open();
    const release = await holdAnimations();
    await click(0); await click(1); await click(2); await click(0);
    check(await page.eval(`${active} === 2 && ${phase(2)} === 'static' && document.querySelector('${cell(0)}').dataset.generation === '1' && document.querySelector('${cell(2)} [data-player-status]').textContent.includes('đủ lượt')`), 'two loading players share atomic Rust budget; third denied; duplicate activation idempotent');
    await reduce(true);
    await page.waitFor(`${active} === 0`, 'motion toggle releases loading reservations');
    await release();
    check(await page.eval(`!document.querySelector('canvas')`), 'late cancelled loads cannot create a canvas');
    const before = requests.filter(engineRequest).length;
    await click(0);
    check(await page.eval(`${active} === 0`) && requests.filter(engineRequest).length === before, 'reduced-motion activation remains static without runtime loading');
    await reduce(false);

    await open();
    const releaseRoute = await holdAnimations();
    await click(0);
    await page.waitFor(`${phase(0)} === 'loading'`, 'in-flight load before route disposal');
    await page.click('#pilot-heading a[href="/"]');
    await page.waitFor(`location.pathname === '/' && !document.querySelector('#expression-pilot')`, 'real Leptos owner disposal');
    await releaseRoute();
    await page.eval(`new Promise(r => setTimeout(r, 200))`);
    check(await page.eval(`!document.querySelector('canvas')`), 'late load after route exit cannot resurrect a disposed player');

    // Inject a newer host fact without delivering its notification, to exercise the exact
    // load-callback race. The separate tab test below still uses real Chrome visibility.
    await open();
    const releaseFacts = await holdAnimations();
    await click(0);
    await page.waitFor(`${phase(0)} === 'loading'`, 'load pending before late host fact');
    await page.eval(`(() => {
      const cell = document.querySelector('[data-expression-index="0"]');
      window.__staleFactProbe = { played: false, observer: new MutationObserver(() => {
        if (cell.dataset.phase === 'playing') window.__staleFactProbe.played = true;
      }) };
      window.__staleFactProbe.observer.observe(cell, { attributes: true, attributeFilter: ['data-phase'] });
      Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
    })()`);
    try {
      await releaseFacts();
      await page.waitFor(`${active} === 0 && ${phase(0)} === 'static'`, 'load callback rechecks newer hidden fact');
      check(await page.eval(`!window.__staleFactProbe.played && !document.querySelector('canvas')`), 'load callback cannot play on stale facts when the host notification is delayed');
    } finally {
      await page.eval(`window.__staleFactProbe.observer.disconnect(); delete window.__staleFactProbe; delete document.visibilityState;`);
    }

    // Offscreen and real tab visibility both cancel; neither return resumes automatically.
    await page.viewport(1440, 700);
    await open(); await click(0);
    await page.waitFor(`${phase(0)} === 'playing'`, 'play before visibility test');
    await page.eval(`scrollTo(0, document.body.scrollHeight)`);
    await page.waitFor(`${active} === 0`, 'IntersectionObserver offscreen cancellation');
    await page.eval('scrollTo(0, 0)');
    await click(0); await page.waitFor(`${phase(0)} === 'playing'`, 'play before real tab hide');
    // The shared host pins focus/visibility for keyboard tests. Release that emulation
    // before asking Chrome to hide the real target; no document property is overridden.
    await page.send('Emulation.setFocusEmulationEnabled', { enabled: false });
    const { targetId } = await page.cdp.send('Target.createTarget', { url: 'about:blank', background: false });
    try {
      await page.cdp.send('Target.activateTarget', { targetId });
      await page.waitFor(`document.visibilityState === 'hidden'`, 'actual hidden tab');
      check(await page.eval('document.visibilityState') === 'hidden', 'Chrome reports actual tab hidden');
      await page.waitFor(`${active} === 0`, 'visibilitychange releases active budget');
    } finally {
      await page.cdp.send('Target.closeTarget', { targetId });
      await page.send('Page.bringToFront');
      await page.send('Emulation.setFocusEmulationEnabled', { enabled: true });
    }
    check(await page.eval(`${active} === 0 && !document.querySelector('canvas')`), 'foreground return retains static rendering');

    // Qualify the actual response under a restrictive document CSP. This is test-injected,
    // not a claim that the app globally installs this policy on client navigation.
    await page.viewport(1440, 1200);
    const csp = "default-src 'none'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; connect-src 'self'; img-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; worker-src 'none'; object-src 'none'; base-uri 'none'";
    let cspError;
    const cspListener = m => {
      if (m.sessionId !== page.sessionId || m.method !== 'Fetch.requestPaused') return;
      (async () => {
        const body = await page.send('Fetch.getResponseBody', { requestId: m.params.requestId });
        await page.send('Fetch.fulfillRequest', { requestId: m.params.requestId, responseCode: m.params.responseStatusCode,
          responseHeaders: [...m.params.responseHeaders, { name: 'Content-Security-Policy', value: csp }],
          body: body.base64Encoded ? body.body : Buffer.from(body.body).toString('base64') });
      })().catch(error => { cspError = error; });
    };
    page.cdp.listeners.push(cspListener);
    await page.send('Fetch.enable', { patterns: [{ urlPattern: url, resourceType: 'Document', requestStage: 'Response' }] });
    try {
      await open(); await click(0);
      await page.waitFor(`${phase(0)} === 'playing'`, 'self-hosted renderer under same-origin CSP with worker-src none');
      check(!cspError, 'CSP response was fulfilled without a bypass');
    } finally {
      await page.send('Fetch.disable');
      page.cdp.listeners.splice(page.cdp.listeners.indexOf(cspListener), 1);
    }

    // Missing a host capability leaves the policy static without loading an engine.
    const { identifier } = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: 'delete window.IntersectionObserver;' });
    try {
      await open(); const beforeUnsupported = requests.filter(engineRequest).length;
      await click(0);
      check(await page.eval(`${active} === 0 && ${phase(0)} === 'static'`) && requests.filter(engineRequest).length === beforeUnsupported, 'unsupported visibility host uses static fallback without an engine');
    } finally { await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier }); }

    // Failures at each actual network boundary, with no external CDN escape.
    for (const boundary of ['engine', 'wasm', 'animation', 'poster']) {
      const block = { engine: '*/expressions/runtime/*.js', wasm: '*/expressions/runtime/*.wasm', animation: '*/expressions/assets/*.json', poster: '*/expressions/assets/*.svg' }[boundary];
      await page.send('Network.setBlockedURLs', { urls: [block] });
      const errorsAt = page.errors.length;
      await open();
      if (boundary === 'poster') {
        await page.waitFor(`document.querySelector('${cell(0)}').dataset.posterFailed === 'true'`, 'missing poster Unicode fallback');
        check(await page.eval(`getComputedStyle(document.querySelector('${cell(0)} .vk-expression-unicode')).display === 'grid'`), 'missing static asset reveals original Unicode');
      } else {
        await click(0);
        await page.waitFor(`${phase(0)} === 'error' && ${active} === 0`, `${boundary} failure returns to static and releases slot`, 20000);
        check(await page.eval(`!document.querySelector('canvas') && document.querySelector('#unicode-sample .vk-expression-poem').textContent === ${JSON.stringify(sample)}`), `${boundary} failure retains source/alt and destroys player`);
      }
      const expected = page.errors.splice(errorsAt);
      if (boundary === 'wasm') check(expected.every(e => /WASM|wasm|Initialization failed|Primary WASM|Buffered fallback/.test(e)), 'blocked WASM produces only expected loader diagnostics');
      else check(expected.length === 0, `no unhandled ${boundary} failure`);
      await page.send('Network.setBlockedURLs', { urls: [] });
    }
    check(!requests.some(u => !u.startsWith(args.base) && /^https?:/.test(u)), 'all expression requests remain same-origin, including missing WASM fallback');
    await open();
    check(await page.eval(`${active} === 0 && !document.querySelector('canvas')`), 'fresh mount recovers with no stale reservation');
  } finally {
    await page.send('Emulation.setScriptExecutionDisabled', { value: false });
    await page.send('Network.setBlockedURLs', { urls: [] });
    await page.send('Network.setCacheDisabled', { cacheDisabled: false });
    await reduce(false);
    await page.send('Emulation.setTouchEmulationEnabled', { enabled: false });
    await page.viewport(1440, 1000);
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(record), 1);
  }
}
