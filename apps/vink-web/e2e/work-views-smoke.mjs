// Epic #323 / #343 Chrome E2E verification against real `vink-app` (author + guest origins)
// and real scratch PostgreSQL, plus real CSR standalone under `vink-identity`.

const SAMPLE_WORK = '33333333-3333-4333-8333-333333333333';
const DRAFT_LINE = 'Phố cũ mưa giăng sợi mỏng,';
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function readAuthorWorkViews(authorBase, workId = SAMPLE_WORK) {
  const resp = await fetch(`${authorBase}/api/v1/author/works?limit=100`);
  if (!resp.ok) {
    throw new Error(`GET /api/v1/author/works returned ${resp.status}`);
  }
  const data = await resp.json();
  const card = data.works?.find((w) => w.work_id === workId);
  if (!card) {
    throw new Error(`work ${workId} missing from author works response`);
  }
  return card.views;
}

function attachViewLedger(page, origin) {
  const rows = [];
  const byId = new Map();
  const matchView = (url) => {
    try {
      const parsed = new URL(url);
      return parsed.origin === origin && /^\/api\/v1\/works\/[^/]+\/views$/.test(parsed.pathname);
    } catch {
      return false;
    }
  };
  const listener = ({ sessionId, method, params }) => {
    if (sessionId !== page.sessionId) return;
    if (method === 'Network.requestWillBeSent' && matchView(params.request.url) && params.request.method === 'POST') {
      const entry = {
        requestId: params.requestId,
        url: params.request.url,
        postData: params.request.postData ?? null,
        status: null,
        extraInfoReceived: false,
        cacheControl: null,
        setCookie: null,
      };
      byId.set(params.requestId, entry);
      rows.push(entry);
    } else if (method === 'Network.responseReceivedExtraInfo' && byId.has(params.requestId)) {
      const entry = byId.get(params.requestId);
      entry.status = params.statusCode;
      entry.extraInfoReceived = true;
      for (const [k, v] of Object.entries(params.headers ?? {})) {
        const lower = k.toLowerCase();
        if (lower === 'cache-control') entry.cacheControl = v;
        if (lower === 'set-cookie') entry.setCookie = v;
      }
    } else if (method === 'Network.responseReceived' && byId.has(params.requestId)) {
      const entry = byId.get(params.requestId);
      if (entry.status === null) entry.status = params.response.status;
    }
  };
  page.cdp.listeners.push(listener);
  return {
    rows,
    stop() {
      const idx = page.cdp.listeners.indexOf(listener);
      if (idx >= 0) page.cdp.listeners.splice(idx, 1);
    },
  };
}

async function waitForCount(predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${label}`);
}

export async function workViewsE2E(_rootPage, { cdp, openPage, args, check }) {
  const guestBase = args.base;
  const authorBase = args['author-base'];
  if (!authorBase) throw new Error('missing --author-base for work views E2E');
  console.log('Epic #323 / #343 work views intake (real Chrome + real App/PG author & guest):');

  const readerPath = `/tac-pham/${SAMPLE_WORK}`;
  const readerApiPath = `/api/v1/works/${SAMPLE_WORK}/reader`;

  // Capture strong ETag on public /reader before any view event.
  const initialReader = await fetch(`${guestBase}${readerApiPath}`);
  const initialEtag = initialReader.headers.get('etag');
  check(
    initialReader.status === 200 && Boolean(initialEtag) && !initialReader.headers.get('set-cookie'),
    'public /reader carries strong ETag and sets no guest cookie',
  );
  const beforeViews = await readAuthorWorkViews(authorBase);
  const startCount = beforeViews?.count ?? 0;

  // 1. Author self-view >= 5s -> +0 and no guest cookie.
  {
    const { browserContextId } = await cdp.send('Target.createBrowserContext');
    const authorPage = await openPage(browserContextId);
    const ledger = attachViewLedger(authorPage, authorBase);
    try {
      await authorPage.goto(`${authorBase}${readerPath}`);
      await authorPage.waitFor(`document.documentElement.dataset.hydrated==='true' && document.visibilityState==='visible'`, 'author Reader hydrated visible');
      await sleep(5500);
      await waitForCount(() => ledger.rows.some((r) => r.status === 204 && r.extraInfoReceived), 4000, 'author view intake POST');
      const req = ledger.rows[0];
      check(req.status === 204 && req.cacheControl === 'no-store' && !req.setCookie, 'author view POST returns 204 no-store without guest cookie');
      const afterAuthor = await readAuthorWorkViews(authorBase);
      check(afterAuthor.state === 'counted' && (afterAuthor.count ?? 0) === startCount, `author self-view >= 5s does not increment (${afterAuthor.count})`);
      check(authorPage.errors.length === 0, 'author view flow has no console errors');
    } finally {
      ledger.stop();
      await cdp.send('Target.disposeBrowserContext', { browserContextId });
    }
  }

  // 2. No-JS SSR HTML + speculative Sec-Purpose: prefetch -> +0 and private number never in HTML/reader.
  const ssrHtml = await (await fetch(`${guestBase}${readerPath}`)).text();
  check(!ssrHtml.includes('counted_since') && !ssrHtml.includes('Lượt xem'), 'SSR HTML never contains private view counts');
  const prefetch = await fetch(`${guestBase}/api/v1/works/${SAMPLE_WORK}/views`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Origin: guestBase,
      'Sec-Purpose': 'prefetch',
    },
    body: JSON.stringify({ event_id: '0193a000-0000-7000-8000-000000000901' }),
  });
  check(prefetch.status === 204 && !prefetch.headers.get('set-cookie'), 'Sec-Purpose prefetch returns 204 without Set-Cookie');
  check((await readAuthorWorkViews(authorBase)).count === startCount, 'prefetch request does not count');

  // 3-5. Guest context: < 5s early leave -> 0 POSTs; >= 5s -> +1 & __Host-vink_guest; revisit within 24h -> +0 & cookie not renewed; ETag unchanged.
  {
    const { browserContextId } = await cdp.send('Target.createBrowserContext');
    const guest1 = await openPage(browserContextId);
    const ledger = attachViewLedger(guest1, guestBase);
    try {
      // Short stay (< 5s): leave at ~1.5s.
      await guest1.goto(`${guestBase}${readerPath}`);
      await guest1.waitFor(`document.documentElement.dataset.hydrated==='true' && document.querySelector('.vk-document')?.textContent.includes(${JSON.stringify(DRAFT_LINE)})`, 'guest1 short visit ready');
      await sleep(1500);
      await guest1.click('#reader-return');
      await guest1.waitFor(`location.pathname==='/'`, 'guest1 left Reader early');
      await sleep(1000);
      check(ledger.rows.length === 0, 'staying < 5s emits zero POST /views');
      check((await readAuthorWorkViews(authorBase)).count === startCount, '< 5s stay keeps count unchanged');

      // Full stay (>= 5s).
      await guest1.goto(`${guestBase}${readerPath}`);
      await guest1.waitFor(`document.documentElement.dataset.hydrated==='true'`, 'guest1 full visit ready');
      await sleep(5200);
      await waitForCount(() => ledger.rows.length === 1 && ledger.rows[0].status === 204 && ledger.rows[0].extraInfoReceived, 4000, 'guest1 first counted view');
      const { cookies } = await guest1.send('Network.getCookies', { urls: [guestBase] });
      const guestCookie = cookies.find((c) => c.name === '__Host-vink_guest');
      check(Boolean(guestCookie?.httpOnly && guestCookie?.secure && guestCookie?.path === '/'), 'Chrome stored HttpOnly Secure __Host-vink_guest cookie');
      check((await readAuthorWorkViews(authorBase)).count === startCount + 1, 'guest >= 5s increments author count to +1');

      // Strong ETag on /reader remains invariant after view count incremented.
      const condReader = await fetch(`${guestBase}${readerApiPath}`, { headers: { 'If-None-Match': initialEtag } });
      check(condReader.status === 304, 'public /reader ETag remains valid (304) after view count increments');

      // Return within 24h via link + >= 5s -> deduped (+0), cookie not renewed.
      await guest1.click('#reader-return');
      await guest1.waitFor(`location.pathname==='/' && document.querySelector('a[href^="${readerPath}"]')`, 'Explore before revisit');
      await guest1.click(`a[href^="${readerPath}"]`);
      await guest1.waitFor(`document.querySelector('.vk-reader')`, 'guest1 revisited Reader');
      await sleep(5500);
      await waitForCount(() => ledger.rows.length >= 2 && ledger.rows[1].status === 204 && ledger.rows[1].extraInfoReceived, 4000, 'guest1 revisit POST settled');
      check(!ledger.rows[1].setCookie, 'valid __Host-vink_guest cookie is not renewed within 24h');
      const cookiesAfter = (await guest1.send('Network.getCookies', { urls: [guestBase] })).cookies.find((c) => c.name === '__Host-vink_guest');
      check(cookiesAfter?.value === guestCookie.value, 'same guest cookie value preserved on 24h return');
      check((await readAuthorWorkViews(authorBase)).count === startCount + 1, '24h repeat visit does not increment');
      check(guest1.errors.length === 0, 'guest1 flow has no console errors');
    } finally {
      ledger.stop();
      await cdp.send('Target.disposeBrowserContext', { browserContextId });
    }
  }

  // 6. Background-opened tab (OQ-3B) + hidden pause/resume: cumulative >= 5s counts +1.
  {
    const { browserContextId } = await cdp.send('Target.createBrowserContext');
    const guest2 = await openPage(browserContextId);
    const ledger = attachViewLedger(guest2, guestBase);
    const { identifier } = await guest2.send('Page.addScriptToEvaluateOnNewDocument', {
      source: `(() => {
        window.__vinkVis = 'hidden';
        Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => window.__vinkVis });
        window.__setVis = (next) => { window.__vinkVis = next; document.dispatchEvent(new Event('visibilitychange')); };
      })()`,
    });
    try {
      await guest2.goto(`${guestBase}${readerPath}`);
      await guest2.waitFor(`document.documentElement.dataset.hydrated==='true' && document.visibilityState==='hidden'`, 'background-opened tab hydrated');
      await sleep(5400);
      check(ledger.rows.length === 0, 'OQ-3B: background-opened tab does not count even after 5.4s');

      // Show for 2.2s (< 5s) -> still 0; hide for 3.5s -> still 0; show for 3.3s (>= 5s total) -> +1.
      await guest2.eval(`window.__setVis('visible')`);
      await sleep(2200);
      check(ledger.rows.length === 0, '2.2s foreground does not emit early');
      await guest2.eval(`window.__setVis('hidden')`);
      await sleep(3500);
      check(ledger.rows.length === 0, 'hidden gap pauses accumulator');
      await guest2.eval(`window.__setVis('visible')`);
      await sleep(3400);
      await waitForCount(() => ledger.rows.length === 1 && ledger.rows[0].status === 204, 4000, 'cumulative foreground view');
      check((await readAuthorWorkViews(authorBase)).count === startCount + 2, 'pause/resume cumulative >= 5s counts +1');
      check(guest2.errors.length === 0, 'guest2 pause/resume flow has no console errors');
    } finally {
      await guest2.send('Page.removeScriptToEvaluateOnNewDocument', { identifier });
      ledger.stop();
      await cdp.send('Target.disposeBrowserContext', { browserContextId });
    }
  }

  // 7. Transient 503 failure retries with SAME event_id and recovers to +1 total.
  {
    const { browserContextId } = await cdp.send('Target.createBrowserContext');
    const guest3 = await openPage(browserContextId);
    const ledger = attachViewLedger(guest3, guestBase);
    let droppedOnce = false;
    const postedIds = [];
    const fetchListener = (message) => {
      if (message.sessionId !== guest3.sessionId || message.method !== 'Fetch.requestPaused') return;
      const { requestId, request } = message.params;
      if (request.method === 'POST' && request.url.endsWith(`/works/${SAMPLE_WORK}/views`)) {
        const body = request.postData ? JSON.parse(request.postData) : {};
        if (body.event_id) postedIds.push(body.event_id);
        if (!droppedOnce) {
          droppedOnce = true;
          guest3.send('Fetch.fulfillRequest', {
            requestId,
            responseCode: 503,
            responseHeaders: [
              { name: 'Content-Type', value: 'application/json' },
              { name: 'Cache-Control', value: 'no-store' },
            ],
            body: Buffer.from(JSON.stringify({ error: { code: 'dependency_unavailable', message: 'retry' } })).toString('base64'),
          }).catch(() => {});
          return;
        }
      }
      guest3.send('Fetch.continueRequest', { requestId }).catch(() => {});
    };
    guest3.cdp.listeners.push(fetchListener);
    try {
      await guest3.send('Fetch.enable', {
        patterns: [{ urlPattern: `${guestBase}/api/v1/works/${SAMPLE_WORK}/views`, requestStage: 'Request' }],
      });
      await guest3.goto(`${guestBase}${readerPath}`);
      await guest3.waitFor(`document.documentElement.dataset.hydrated==='true'`, 'guest3 ready for retry');
      await sleep(5200);
      await waitForCount(() => droppedOnce, 4000, 'first attempt hit 503');
      // Allow 503 response delivery into WASM so 1s retry_timer is actively armed before waking.
      await sleep(150);
      // P1 regression: online/focus/visibilitychange during 1s backoff must NOT abort retry timer.
      await guest3.eval(`
        window.dispatchEvent(new Event('online'));
        window.dispatchEvent(new Event('focus'));
        document.dispatchEvent(new Event('visibilitychange'));
      `);
      await waitForCount(() => postedIds.length >= 2 && ledger.rows.some((r) => r.status === 204 && r.extraInfoReceived), 5000, 'bounded retry recovery');
      check(postedIds[0] === postedIds[1], 'retry after 503 with lifecycle wakes reuses exact same UUIDv7 event_id');
      check((await readAuthorWorkViews(authorBase)).count === startCount + 3, 'retry recovers to +1 without blocking Reader');
      check(guest3.errors.length === 0, 'retry flow raises zero browser/console errors');
    } finally {
      await guest3.send('Fetch.disable').catch(() => {});
      const idx = guest3.cdp.listeners.indexOf(fetchListener);
      if (idx >= 0) guest3.cdp.listeners.splice(idx, 1);
      ledger.stop();
      await cdp.send('Target.disposeBrowserContext', { browserContextId });
    }
  }

  // 8. E-37 ("Xem bản mới") and Hidden replacement cancel without counting (0 POST /views).
  {
    const { browserContextId } = await cdp.send('Target.createBrowserContext');
    const guest4 = await openPage(browserContextId);
    const ledger = attachViewLedger(guest4, guestBase);
    let overridePub = null;
    const { identifier } = await guest4.send('Page.addScriptToEvaluateOnNewDocument', {
      source: `(() => {
        window.__vinkVis = 'visible';
        Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => window.__vinkVis });
        window.__setVis = (next) => { window.__vinkVis = next; document.dispatchEvent(new Event('visibilitychange')); };
        let offset = 0;
        const origNow = performance.now.bind(performance);
        const origDateNow = Date.now.bind(Date);
        performance.now = () => origNow() + offset;
        Date.now = () => origDateNow() + offset;
        window.__vinkAdvance = (ms) => { offset += ms; };
      })()`,
    });
    const pubListener = (message) => {
      if (message.sessionId !== guest4.sessionId || message.method !== 'Fetch.requestPaused') return;
      const { requestId, request } = message.params;
      if (
        request.method === 'GET' &&
        overridePub &&
        (request.url.includes(`/api/v1/works/${SAMPLE_WORK}/reader`) ||
          request.url.includes(`/api/v1/works/${SAMPLE_WORK}/publication`))
      ) {
        guest4.send('Fetch.fulfillRequest', {
          requestId,
          responseCode: overridePub.status,
          responseHeaders: [
            { name: 'Content-Type', value: 'application/json' },
            { name: 'Cache-Control', value: 'no-store' },
          ],
          body: Buffer.from(JSON.stringify(overridePub.body)).toString('base64'),
        }).catch(() => {});
        return;
      }
      guest4.send('Fetch.continueRequest', { requestId }).catch(() => {});
    };
    guest4.cdp.listeners.push(pubListener);
    try {
      const pubResp = await fetch(`${guestBase}/api/v1/works/${SAMPLE_WORK}/reader`);
      const originalPub = await pubResp.json();
      await guest4.send('Fetch.enable', {
        patterns: [{ urlPattern: `${guestBase}/api/v1/works/${SAMPLE_WORK}/*`, requestStage: 'Request' }],
      });
      await guest4.goto(`${guestBase}${readerPath}`);
      await guest4.waitFor(`document.documentElement.dataset.hydrated==='true'`, 'guest4 hydrated');
      await sleep(1800);
      await guest4.eval(`window.__setVis('hidden'); window.__vinkAdvance(140000);`);
      overridePub = {
        status: 200,
        body: {
          ...originalPub,
          status: 'published',
          revision_id: '55555555-5555-4555-8555-555555555555',
          title: 'Bản mới thay thế trong kiểm thử',
        },
      };
      await guest4.eval(`window.__setVis('visible'); window.dispatchEvent(new Event('focus'));`);
      await guest4.waitFor(`Boolean(document.querySelector('[data-testid=reader-update] button'))`, 'update banner E-37 shown');
      await guest4.click('[data-testid=reader-update] button');
      await guest4.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '55555555-5555-4555-8555-555555555555'`, 'Xem bản mới applied');
      await sleep(5400);
      check(ledger.rows.length === 0, 'E-37 Xem bản mới cancels view visit permanently (no POST even after 5.4s)');
      check((await readAuthorWorkViews(authorBase)).count === startCount + 3, 'author view total remains unchanged after E-37');
      check(guest4.errors.length === 0, 'E-37 cancel flow has no console errors');
    } finally {
      await guest4.send('Page.removeScriptToEvaluateOnNewDocument', { identifier }).catch(() => {});
      await guest4.send('Fetch.disable').catch(() => {});
      const idx = guest4.cdp.listeners.indexOf(pubListener);
      if (idx >= 0) guest4.cdp.listeners.splice(idx, 1);
      ledger.stop();
      await cdp.send('Target.disposeBrowserContext', { browserContextId });
    }
  }

  // 9. Author-hidden replacement page + background republish (OQ-3A) stay uncounted (0 POST /views).
  {
    const { browserContextId } = await cdp.send('Target.createBrowserContext');
    const guest5 = await openPage(browserContextId);
    const ledger = attachViewLedger(guest5, guestBase);
    let serveHidden = true;
    const { identifier } = await guest5.send('Page.addScriptToEvaluateOnNewDocument', {
      source: `(() => {
        let offset = 0;
        const origNow = performance.now.bind(performance);
        const origDateNow = Date.now.bind(Date);
        performance.now = () => origNow() + offset;
        Date.now = () => origDateNow() + offset;
        window.__vinkAdvance = (ms) => { offset += ms; };
      })()`,
    });
    const readerListener = (message) => {
      if (message.sessionId !== guest5.sessionId || message.method !== 'Fetch.requestPaused') return;
      const { requestId, request } = message.params;
      if (request.method === 'GET' && serveHidden && request.url.includes(`/api/v1/works/${SAMPLE_WORK}/reader`)) {
        guest5.send('Fetch.fulfillRequest', {
          requestId,
          responseCode: 200,
          responseHeaders: [
            { name: 'Content-Type', value: 'application/json' },
            { name: 'Cache-Control', value: 'no-store' },
          ],
          body: Buffer.from(JSON.stringify({ status: 'hidden', work_id: SAMPLE_WORK })).toString('base64'),
        }).catch(() => {});
        return;
      }
      guest5.send('Fetch.continueRequest', { requestId }).catch(() => {});
    };
    guest5.cdp.listeners.push(readerListener);
    try {
      await guest5.send('Fetch.enable', {
        patterns: [{ urlPattern: `${guestBase}/api/v1/works/${SAMPLE_WORK}/reader*`, requestStage: 'Request' }],
      });
      await guest5.goto(`${guestBase}/`);
      await guest5.waitFor(
        `document.documentElement.dataset.hydrated==='true' && Boolean(document.querySelector('a[href^="${readerPath}"]'))`,
        'Explore hydrated before hidden Reader open',
      );
      await guest5.click(`a[href^="${readerPath}"]`);
      await guest5.waitFor(`Boolean(document.querySelector('[data-testid=reader-hidden]'))`, 'hidden replacement rendered');
      // Now simulate background republish while tab stays on this route (OQ-3A).
      serveHidden = false;
      await guest5.eval(`window.__vinkAdvance(140000); window.dispatchEvent(new Event('focus'));`);
      await guest5.waitFor(`Boolean(document.querySelector('.vk-document'))`, 'background republish rendered after Hidden');
      await sleep(5400);
      check(ledger.rows.length === 0, 'OQ-3A: Hidden -> background republish emits 0 POST /views without intentional reopen');
      check((await readAuthorWorkViews(authorBase)).count === startCount + 3, 'author view total remains unchanged after OQ-3A');
      check(guest5.errors.length === 0, 'hidden replacement & OQ-3A check has no console errors');
    } finally {
      await guest5.send('Page.removeScriptToEvaluateOnNewDocument', { identifier }).catch(() => {});
      await guest5.send('Fetch.disable').catch(() => {});
      const idx = guest5.cdp.listeners.indexOf(readerListener);
      if (idx >= 0) guest5.cdp.listeners.splice(idx, 1);
      ledger.stop();
      await cdp.send('Target.disposeBrowserContext', { browserContextId });
    }
  }

  // 10. Integrated #514 Desk UI check: real author Desk renders the live counted views total from PG.
  {
    const { browserContextId } = await cdp.send('Target.createBrowserContext');
    const deskPage = await openPage(browserContextId);
    try {
      await deskPage.goto(`${authorBase}/ban-viet`);
      await deskPage.waitFor(
        `document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy')==='false' && Boolean(document.querySelector('#draft-row-${SAMPLE_WORK} [data-testid=author-work-views]'))`,
        'author Desk hydrated with real work views',
      );
      const expected = String(startCount + 3);
      check(
        await deskPage.eval(`(() => {
          const row = document.querySelector('#draft-row-${SAMPLE_WORK}');
          const views = row?.querySelector('[data-testid=author-work-views]');
          const icons = [...row.querySelectorAll('[data-testid=author-work-metrics] [data-icon]')].map(el => el.dataset.icon);
          return views?.dataset.viewsState === 'counted' &&
            views.title.startsWith(${JSON.stringify(`${expected} lượt xem`)}) &&
            !row.querySelector('.vk-author-work__reader') &&
            icons.join(',') === 'pen-flight,chat-bubble-outline,visibility-outline';
        })()`),
        `#514 real Desk UI displays live counted views (${expected}) in Thấm -> Nhận xét -> Lượt xem order with no Read link`,
      );
      await deskPage.click('#views-explainer-trigger');
      await deskPage.waitFor(`document.querySelector('#views-explainer')?.open === true`, 'Desk explainer dialog opened');
      check(
        await deskPage.eval(`document.querySelector('#views-explainer')?.textContent.includes('5 giây')`),
        '#514 real Desk explainer shows 5s threshold from shared policy',
      );
      check(deskPage.errors.length === 0, 'author Desk real views flow has no console errors');
    } finally {
      await cdp.send('Target.disposeBrowserContext', { browserContextId });
    }
  }
}

export async function standaloneWorkViewCheck(page, { args, check }) {
  const base = args.base;
  const path = `/tac-pham/${SAMPLE_WORK}`;
  const ledger = attachViewLedger(page, base);
  try {
    // Wait after short visit (< 5s) -> 0 POSTs.
    await page.goto(`${base}${path}`);
    await page.waitFor(`Boolean(document.querySelector('.vk-document'))`, 'standalone short visit loaded');
    await sleep(1200);
    await page.click('#reader-return');
    await page.waitFor(`location.pathname === '/'`, 'standalone early exit');
    await sleep(800);
    check(ledger.rows.length === 0, 'standalone CSR: staying < 5s emits 0 POST /views');

    // Logged-in second reader (>= 5s) -> POST /views 204 no-store without minting guest cookie.
    await page.goto(`${base}${path}`);
    await page.waitFor(`Boolean(document.querySelector('.vk-document'))`, 'standalone full visit loaded');
    await sleep(5300);
    await waitForCount(() => ledger.rows.length === 1 && ledger.rows[0].status === 204 && ledger.rows[0].extraInfoReceived, 4000, 'standalone CSR member view POST');
    check(
      ledger.rows[0].status === 204 && ledger.rows[0].cacheControl === 'no-store' && !ledger.rows[0].setCookie,
      'standalone CSR: authenticated non-author member counts via 204 no-store without guest cookie',
    );

    // E-41: hold the real session read after cookie revocation. The signal alone must
    // cancel the current visit before the timer can send a POST under the guest scope.
    await page.goto(`${base}${path}`);
    await page.waitFor(`Boolean(document.querySelector('.vk-document') && document.querySelector('#root-composer'))`,
      'standalone Reader body and signed-in composer ready');
    let pendingSession = false;
    const sessionPath = '/api/v1/auth/session';
    const holdSession = ({ sessionId, method, params }) => {
      if (sessionId !== page.sessionId || method !== 'Fetch.requestPaused') return;
      if (params.request.method === 'GET' && new URL(params.request.url).pathname === sessionPath) {
        pendingSession = true;
        return;
      }
      page.send('Fetch.continueRequest', { requestId: params.requestId }).catch(() => {});
    };
    page.cdp.listeners.push(holdSession);
    try {
      await page.send('Fetch.enable', {
        patterns: [{ urlPattern: `${base}${sessionPath}`, requestStage: 'Response' }],
      });
      const revoked = await page.eval(`fetch('${sessionPath}', { method: 'DELETE' }).then(r => r.status)`);
      check(revoked === 204, 'standalone CSR E-41: real Identity session is revoked');
      await page.eval(`(() => {
        const channel = new BroadcastChannel('vink-session');
        channel.postMessage('changed');
        channel.close();
      })()`);
      await waitForCount(() => pendingSession, 4000, 'held session recheck after the change signal');
      await sleep(5300);
      check(ledger.rows.length === 1, 'standalone CSR E-41: session signal cancels the visit before the authority recheck settles');
    } finally {
      page.cdp.listeners.splice(page.cdp.listeners.indexOf(holdSession), 1);
      await page.send('Fetch.disable');
    }
  } finally {
    ledger.stop();
  }
}
