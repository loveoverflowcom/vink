// #236: same-document return geometry and focus. Run via the existing CDP / scratch App.
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { writeFileSync } from 'node:fs';

const READY = "document.documentElement.dataset.hydrated==='true'";
const ROWS = "document.querySelectorAll('.vk-public-row').length";
const frames = 'new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)))';
const TOLERANCE_PX = 2;
const offline = value => ({ offline: value, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });

async function heldList(page, base, action, release, pattern = '/api/v1/works?*') {
  let signal;
  let requestId;
  const paused = new Promise(resolve => { signal = resolve; });
  const listener = event => {
    if (event.sessionId !== page.sessionId || event.method !== 'Fetch.requestPaused') return;
    requestId = event.params.requestId;
    signal(requestId);
  };
  page.cdp.listeners.push(listener);
  try {
    await page.send('Fetch.enable', { patterns: [{ urlPattern: base + pattern, requestStage: 'Request' }] });
    await action();
    let timer;
    const id = await Promise.race([paused, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('missing held list attempt')), 10000);
    })]).finally(() => clearTimeout(timer));
    const { status, body } = await release();
    await page.send('Fetch.fulfillRequest', { requestId: id, responseCode: status,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify(body)).toString('base64') });
    requestId = null;
  } finally {
    if (requestId) await page.send('Fetch.failRequest', { requestId, errorReason: 'Aborted' }).catch(() => {});
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
  }
}

export async function discoveryReturnPosition(page, { args, check }) {
  const git = (...argv) => execFileSync('git', argv, { encoding: 'utf8' }).trim();
  const out = join(dirname(args.state), 'discovery-return-position.json');
  const raw = {
    issue: 236, source_sha: git('rev-parse', 'HEAD'), dirty: git('status', '--porcelain'),
    browser: await page.cdp.send('Browser.getVersion'), build_profile: args['build-profile'] ?? 'unknown',
    boundary: 'loopback App/PostgreSQL guest, #100/#234 synthetic corpus; labelled CDP controls only',
    tolerance_css_px: TOLERANCE_PX, status: 'FAIL', cases: [], cycles: [], post_gc: [],
  };
  const result = (case_id, pass, evidence = {}) => raw.cases.push({ case_id, pass, ...evidence });
  const flush = () => writeFileSync(out, JSON.stringify(raw, null, 2));
  const navigate = href => page.eval(`(() => {
    const a = document.createElement('a'); a.href = ${JSON.stringify(href)};
    document.body.append(a); a.click(); a.remove();
  })()`);
  const refocus = selector => page.eval(`document.querySelector(${JSON.stringify(selector)}).focus()`);

  try {
    await page.viewport(1280, 900);
    const tagsResponse = await fetch(args.base + '/api/v1/tags');
    check(tagsResponse.ok, 'real scratch catalog for the return journey');
    const { tags } = await tagsResponse.json();
    const first = '/?q=kiem%20thu' + ['Thơ', 'Ký ức'].map(label =>
      '&tag=' + encodeURIComponent(tags.find(tag => tag.label === label).slug)).join('');
    await page.goto(args.base + first);
    await page.waitFor(`${READY} && ${ROWS}===12`, 'first filtered page');
    await page.click('#public-next');
    await page.waitFor(`new URLSearchParams(location.search).has('cursor') && ${ROWS}===12`, 'bounded second page with a real cursor');
    const scope = await page.eval('location.pathname+location.search');
    const anchor = await page.eval("document.querySelectorAll('.vk-public-row')[6].id");
    const href = scope + '#' + anchor;
    const link = '#' + anchor + ' .vk-public-row__link';
    const readerPath = await page.eval(`new URL(document.querySelector(${JSON.stringify(link)}).href).pathname`);
    const read = `location.pathname===${JSON.stringify(readerPath)} && !!document.querySelector('#reader-paper')`;
    const arrived = `location.pathname+location.search+location.hash===${JSON.stringify(href)} && !!document.querySelector(${JSON.stringify(link)})`;
    const settle = async () => { await page.eval('document.fonts.ready.then(()=>true)'); await page.eval(frames); };
    const timeOrigin = await page.eval('performance.timeOrigin');
    const geometry = () => page.eval(`(() => {
      const anchor = document.getElementById(${JSON.stringify(anchor)});
      const a = anchor.querySelector('.vk-public-row__link');
      const scroller = document.scrollingElement;
      const margin = parseFloat(getComputedStyle(anchor).scrollMarginTop) || 0;
      const desired = Math.max(0, Math.min(scroller.scrollHeight - scroller.clientHeight,
        scroller.scrollTop + anchor.getBoundingClientRect().top - margin));
      const rect = a.getBoundingClientRect();
      const bar = document.querySelector('.vk-ui-bottom-nav');
      const viewportBottom = bar?.getClientRects().length ? bar.getBoundingClientRect().top : innerHeight;
      return { focused: document.activeElement===a, visible: rect.top>=0 && rect.bottom<=viewportBottom,
        error_px: Math.abs(desired-scroller.scrollTop), anchor_top_px: anchor.getBoundingClientRect().top,
        scroll_margin_px: margin, width: innerWidth, height: innerHeight, dpr: devicePixelRatio,
        root_font_px: parseFloat(getComputedStyle(document.documentElement).fontSize),
        overflow_px: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        retained_reader: document.querySelectorAll('#reader-paper').length,
        explore_instances: document.querySelectorAll('#explore').length, same_document: performance.timeOrigin===${timeOrigin} };
    })()`);
    const fits = g => g.visible && g.focused && g.error_px <= TOLERANCE_PX && g.overflow_px <= 1 &&
      !g.retained_reader && g.explore_instances === 1 && g.same_document;
    const leave = async keyboard => {
      if (keyboard) { await refocus(link); await page.key('Enter'); }
      else await page.click(link);
      await page.waitFor(read, 'native link reaches Reader in the same document');
      if (!await page.eval(`performance.timeOrigin===${timeOrigin} && !document.querySelector('#explore') &&
        document.querySelector('#reader-return')?.getAttribute('href')===${JSON.stringify(href)}`)) {
        throw new Error('Reader must keep q, both tags, cursor and row, and dispose the Explore tree');
      }
    };
    const restore = async history => {
      if (history) await page.eval('history.back()');
      else await page.click('#reader-return');
      await page.waitFor(arrived, 'exact URL and saved row are back');
      await settle();
      return geometry();
    };

    // A modifier opens a new tab and never edits the original history entry.
    const cdpPages = async () => (await page.cdp.send('Target.getTargets')).targetInfos.filter(t => t.type === 'page');
    const existing = new Set((await cdpPages()).map(t => t.targetId));
    await page.click(link, page.mod);
    let tab;
    for (const deadline = Date.now() + 10000; !tab && Date.now() < deadline;) {
      tab = (await cdpPages()).find(t => !existing.has(t.targetId) && t.url.startsWith(args.base + readerPath));
      if (!tab) await new Promise(resolve => setTimeout(resolve, 50));
    }
    result('modified-link-new-tab', !!tab && await page.eval(`location.pathname+location.search+location.hash===${JSON.stringify(scope)}`));
    if (tab) await page.cdp.send('Target.closeTarget', { targetId: tab.targetId });
    await page.send('Page.bringToFront');

    await leave(true);
    const keyboardBack = await restore(true);
    result('keyboard-back', fits(keyboardBack), keyboardBack);
    await page.capture('return-position-desktop', { boundary: 'real-public-api', scenario: '#236-keyboard-history-return' });
    await page.eval('history.forward()');
    await page.waitFor(read, 'Forward opens the same Reader');
    const forwardBack = await restore(true);
    result('forward-then-back', fits(forwardBack), forwardBack);

    await leave(false);
    await page.viewport(390, 844);
    await page.eval("document.documentElement.style.fontSize='200%'");
    await page.send('Emulation.setEmulatedMedia', { features: [
      { name: 'prefers-color-scheme', value: 'light' }, { name: 'prefers-reduced-motion', value: 'reduce' },
    ] });
    const narrow = await restore(false);
    result('resize-text200-reduced-motion', fits(narrow), { ...narrow, text_scale: 'root font CSS override' });
    await page.capture('return-position-narrow-text200', { boundary: 'real-public-api', scenario: '#236-return-after-resize' });
    await page.eval("document.documentElement.style.fontSize=''");
    await page.viewport(1280, 900);
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });

    // Real local files with distinct experiment URLs. The public row currently has no cover;
    // reserve the test image's box before restoration, as required by the media layout contract.
    await leave(false);
    const media = new Map();
    const collect = event => {
      if (event.sessionId === page.sessionId && event.method === 'Fetch.requestPaused')
        media.set(event.params.resourceType, event.params.requestId);
    };
    page.cdp.listeners.push(collect);
    try {
      await page.send('Fetch.enable', { patterns: [{urlPattern: args.base + '/pkg/*?return-probe', requestStage:'Response'}] });
      await page.eval(`(() => {
        const style = document.createElement('style');
        style.id='return-probe-style';
        style.textContent="@font-face{font-family:ReturnProbe;src:url('/pkg/fonts/literata-variable.woff2?return-probe')} .vk-public-row__link{font-family:ReturnProbe,serif}";
        document.head.append(style);
        const observer=new MutationObserver(() => {
          const row=document.querySelector('.vk-public-row');
          if (!row) return;
          observer.disconnect();
          const img=new Image(32,32); img.id='return-probe-image'; img.alt='';
          img.src='/pkg/brand/vpen-r1-mark-light-32.png?return-probe'; row.append(img);
        });
        observer.observe(document.body,{subtree:true,childList:true});
      })()`);
      await page.click('#reader-return');
      await page.waitFor(arrived, 'return before font or image settles');
      const deadline=Date.now()+10000;
      while (media.size<2 && Date.now()<deadline) await new Promise(r=>setTimeout(r,20));
      check(media.has('Font') && media.has('Image'), 'both late media responses are pending');
      await page.eval(frames);
      const before=await geometry();
      for (const requestId of media.values()) await page.send('Fetch.continueResponse',{requestId});
      await page.eval("Promise.all([document.fonts.ready,document.querySelector('#return-probe-image').decode()]).then(()=>true)");
      await settle();
      const after=await geometry();
      result('late-font-and-sized-image', fits(before) && fits(after), { before, after,
        decoded:await page.eval("document.fonts.check('16px ReturnProbe') && document.querySelector('#return-probe-image').naturalWidth===32") });
    } finally {
      await page.send('Fetch.disable');
      page.cdp.listeners=page.cdp.listeners.filter(l=>l!==collect);
      await page.eval("document.querySelector('#return-probe-style')?.remove();document.querySelector('#return-probe-image')?.remove()");
    }

    // Temporarily offline, a recent navigation snapshot can render; controls still own authority.
    await leave(false);
    await page.send('Network.emulateNetworkConditions', offline(true));
    try {
      const warmOffline = await restore(true);
      result('offline-warm-return', fits(warmOffline) && await page.eval('navigator.onLine===false'), warmOffline);
    } finally { await page.send('Network.emulateNetworkConditions', offline(false)); }

    for (let i = 1; i <= 50; i++) {
      const start = performance.now();
      await leave(false);
      const geometry = await restore(i % 2 === 0);
      raw.cycles.push({ iteration: i, method: i % 2 === 0 ? 'back' : 'return-link', pass: fits(geometry),
        wall_ms: performance.now() - start, ...geometry });
      if ([5, 10, 20, 30, 40, 50].includes(i)) {
        await page.send('HeapProfiler.collectGarbage');
        raw.post_gc.push({ iteration: i, ...await page.send('Memory.getDOMCounters'),
          heap: await page.send('Runtime.getHeapUsage'),
          read_owner: await page.eval("import('/pkg/vink.js').then(m=>m.public_read_counts ? JSON.parse(m.public_read_counts()) : null)") });
        flush();
      }
    }
    result('repeat-50-no-retained-views', raw.cycles.every(cycle => cycle.pass));

    const owners = raw.post_gc.map(sample => sample.read_owner);
    if (owners.every(Boolean)) {
      const counts = ['observers','resource_tickets','read_event_subscriptions','scheduler_timers','waiters','flights'];
      result('bounded-read-owner', counts.every(k => owners.every(o => o[k] === owners[0][k])) &&
        owners[0].observers===1 && owners[0].scheduler_timers<=1 && owners[0].flights===0);
    }

    const listed = await (await fetch(args.base + '/api/v1/works' + scope.slice(1))).json();
    check(listed.works.some(work => 'work-' + work.work_id === anchor), 'real oracle page contains the saved row');
    const someOther = listed.works.find(work => 'work-' + work.work_id !== anchor);
    const changed = { ...listed, works: listed.works.filter(work => work !== someOther) };
    // A background result changes the list while the reader uses the search field. Restoring
    // again on list membership elsewhere is visible as a wrong focus and scroll jump.
    await heldList(page, args.base, () => page.eval(`(() => {
      const now = performance.now(); performance.now = () => now + 121000;
      window.dispatchEvent(new Event('focus'));
    })()`), async () => {
      await page.typeAtEnd('#public-query', ' — đang gõ');
      return { status: 200, body: changed };
    });
    await page.waitFor(`${ROWS}===11`, 'the delayed updated list is actually applied');
    await page.eval(frames);
    result('late-refresh-keeps-search-focus', await page.eval("document.activeElement?.id==='public-query' && document.querySelector('#public-query').value.endsWith(' — đang gõ')"), { injection: 'clock +121s; held altered list response (anchor still present)' });
    await page.capture('return-position-after-delayed-read', { boundary: 'CDP-held-list', scenario: '#236-late-refresh-focus' });

    // Hold B, return to the cached A with its exact cursor, then deliver a different list.
    await heldList(page, args.base, () => navigate('/?q=scope-b#community-stream'), async () => {
      await navigate(href);
      await page.waitFor(arrived, 'cached A before obsolete B is released');
      await settle();
      return { status:200, body:{ ...listed, works:[] } };
    }, '/api/v1/works?q=scope-b*');
    await settle();
    const afterABA = await geometry();
    result('query-A-B-A-late', await page.eval(arrived) && fits(afterABA) && await page.eval(ROWS+'===11'), afterABA);

    // Force pressure through actual distinct list keys, then revisit A offline. A network
    // attempt and unavailable state distinguish eviction from a warm offline render.
    for (let i=0;i<18;i++) {
      const q='pressure-'+i;
      await navigate('/?q='+q);
      await page.waitFor(`location.search==='?q=${q}' && document.querySelector('[data-testid=public-empty]')`, 'distinct query settled');
    }
    await page.send('Network.emulateNetworkConditions', offline(true));
    try {
      await navigate(href);
      await page.waitFor("!!document.querySelector('[data-testid=public-unavailable]')", 'evicted A is unavailable offline');
      await settle();
      result('evicted-offline-fallback', await page.eval(`location.search+location.hash===${JSON.stringify(href.slice(1))} &&
        !document.getElementById(${JSON.stringify(anchor)}) && document.activeElement?.id==='community-stream'`));
    } finally { await page.send('Network.emulateNetworkConditions', offline(false)); }

    for (const status of [200, 503]) {
      // Cold client navigation from a new document: no tab snapshot to invent a result.
      await page.goto(args.base + '/toi');
      await page.waitFor(READY, 'fresh utility document');
      await heldList(page, args.base, () => navigate(href), async () => ({ status, body: status === 200 ?
        { ...listed, works: listed.works.filter(work => 'work-' + work.work_id !== anchor) } :
        { error: { code: 'dependency_unavailable' } } }));
      await page.waitFor(status === 200 ? `${ROWS}===11` : "!!document.querySelector('[data-testid=public-unavailable]')", 'missing row settles');
      await page.eval(frames);
      result(status === 200 ? 'missing-row-fallback' : 'unavailable-page-fallback', await page.eval(`
        !document.getElementById(${JSON.stringify(anchor)}) && document.activeElement?.id==='community-stream' &&
        document.activeElement?.getAttribute('aria-label')==='Tác phẩm khám phá'`), { injection: 'only list transport', status });
    }
    check(raw.cases.every(c => c.pass), 'return-position regression: ' + raw.cases.filter(c => !c.pass).map(c => c.case_id).join(', '));
    raw.status = 'PASS';
  } catch (error) {
    raw.error = error.message;
    throw error;
  } finally { flush(); }
}
