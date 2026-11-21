// #295: real SSR, Chrome and native Identity against disposable PostgreSQL databases.
import { fill, HYDRATED, q } from './auth-smoke.mjs';
import { authorFixtureSql } from './author-fixtures.mjs';

const MODE = '/?mode=authors';
const SEARCH = '/api/v1/authors';
const INPUT = '#author-search-query';
const HAS_ROWS = 'document.querySelectorAll("#author-search-list > li").length';
const row = handle => '#search-author-' + handle;
const currentAuthor = 'tac-gia-thu-01';
const secondPageAuthor = 'tac-gia-thu-15';
const pen = 'Đặng Minh Hòa';
const queryUrl = (query) => MODE + '&q=' + encodeURIComponent(query);
const ENDPOINTS = {
  register: '/api/v1/auth/password/register',
  principal: '/api/v1/auth/principal',
  session: '/api/v1/auth/session',
};

function seed() {
  authorFixtureSql(`BEGIN;
    CREATE TEMP TABLE author_lookup ON COMMIT DROP AS
     SELECT md5('author-search-web-' || i)::uuid AS id,
            'tac-gia-thu-' || lpad(i::text, 2, '0') AS handle
       FROM generate_series(1, 16) i;
    INSERT INTO principals (id, display_name)
      SELECT id, 'Đặng Minh Hòa' FROM author_lookup;
    INSERT INTO principal_bindings (principal_id, authority, subject_id)
      SELECT id, 'vink-identity', id::text FROM author_lookup;
    INSERT INTO author_handle_names (principal_id, handle)
      SELECT id, handle FROM author_lookup;
    INSERT INTO author_handle_names (principal_id, handle)
      SELECT id, 'ten-cu-rieng' FROM author_lookup WHERE handle = 'tac-gia-thu-01';
    INSERT INTO author_profiles (principal_id, current_handle)
      SELECT id, handle FROM author_lookup;
    UPDATE author_handle_rollout SET ready = true;
   COMMIT;`);
}

async function enter(page) {
  // A native GET can remove the current CDP execution context within this key press.
  for (const type of ['keyDown', 'keyUp']) {
    await page.send('Input.dispatchKeyEvent', {
      type, key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13,
      ...(type === 'keyDown' ? { text: '\r' } : {}),
    });
  }
}

async function back(page) {
  const { entries, currentIndex } = await page.send('Page.getNavigationHistory', {});
  await page.send('Page.navigateToHistoryEntry', { entryId: entries[currentIndex - 1].id });
}

async function intercept(page, pattern, handler) {
  const listener = message => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    handler(message.params);
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: pattern, requestStage: 'Request' }] });
  return async () => {
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener), 1);
    await page.send('Fetch.disable');
  };
}

async function backThroughUnhydratedProfile(page, base, path, selector, query) {
  await page.send('Network.setCacheDisabled', { cacheDisabled: true });
  const heldWasm = [];
  const releaseWasm = await intercept(page, base + '/pkg/vink_bg.wasm*', ({ requestId }) => heldWasm.push(requestId));
  try {
    await back(page);
    await page.waitFor(`location.pathname === ${JSON.stringify(path)} && ${q(selector)} && !(${HYDRATED})`,
      'first Back reaches profile HTML before hydration at ' + path);
    await back(page);
    await page.waitFor(`location.pathname === '/' && new URL(location.href).searchParams.get('q') === ${JSON.stringify(query)}`,
      'second Back reaches the original author-search URL');
  } finally {
    for (const requestId of heldWasm) await page.send('Fetch.continueRequest', { requestId }).catch(() => {});
    await releaseWasm();
    await page.send('Network.setCacheDisabled', { cacheDisabled: false });
  }
}

function fetchInPage(page, path, options = {}) {
  return page.eval(`(async () => {
    const response = await fetch(${JSON.stringify(path)}, ${JSON.stringify(options)});
    return { status: response.status, body: await response.text() };
  })()`);
}

async function provision(page, name) {
  const post = (path, payload) => fetchInPage(page, path, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload),
  });
  const register = await post(ENDPOINTS.register, {
    username: 'author_lookup_' + name,
    password: 'synthetic-browser-password-for-' + name,
  });
  if (register.status !== 200) throw new Error('password fixture registration: ' + register.status);
  const principal = await post(ENDPOINTS.principal, { pen_name: 'Độc giả thử nghiệm ' + name });
  if (principal.status !== 201) throw new Error('password fixture binding: ' + principal.status);
}

export async function authorSearch(page, { args, check, textContrast }) {
  seed();
  const base = args.base;
  const exact = await fetch(base + queryUrl('@' + currentAuthor));
  const html = await exact.text();
  check(exact.status === 200 && exact.headers.get('cache-control') === 'no-store'
    && html.includes('href="/@' + currentAuthor + '"') && html.includes(pen)
    && !html.includes('href="/@tac-gia-thu-02"'), 'SSR exact Handle is a standalone public row');
  const alias = await fetch(base + '/@ten-cu-rieng', { redirect: 'manual' });
  const aliasSearch = await fetch(base + SEARCH + '?q=%40ten-cu-rieng').then(r => r.json());
  check(alias.status === 301 && alias.headers.get('location') === '/@' + currentAuthor
    && aliasSearch.authors.length === 0, 'historical Handle resolves from its URL and stays outside search');

  await page.send('Network.setBlockedURLs', { urls: [base + '/pkg/vink.js*', base + '/pkg/vink_bg.wasm*'] });
  await page.send('Emulation.setScriptExecutionDisabled', { value: true });
  try {
    await page.goto(base + MODE);
    await page.waitFor(q(INPUT), 'SSR author search form before Web bundle');
    await fill(page, INPUT, pen);
    await enter(page);
    await page.waitFor(`new URL(location.href).searchParams.get('q') === ${JSON.stringify(pen)} && ${HAS_ROWS} === 12`, 'native GET without hydration');
    check(await page.eval(`!(${HYDRATED}) && ${q('#author-search-more')}.href.includes('cursor=')`),
      'native form and paginated link usable without JavaScript');
  } finally {
    await page.send('Emulation.setScriptExecutionDisabled', { value: false });
    await page.send('Network.setBlockedURLs', { urls: [] });
  }

  await page.goto(base + '/');
  const entry = '#explore-mode-authors';
  await page.waitFor(`${HYDRATED} && ${q(entry)}`, 'author search mode in Khám phá');
  check(await page.eval(`${q('#explore-mode-works')}.parentElement === ${q(entry)}.parentElement
    && ${q('#explore-mode-following')}.parentElement === ${q(entry)}.parentElement`),
    'work, following and author modes share one navigation region');
  await page.click(entry);
  await page.waitFor(`${HYDRATED} && ${HAS_ROWS} === 12`, 'guest author suggestions');
  const guest = await fetch(base + SEARCH + '?q=' + encodeURIComponent(pen)).then(r => r.text());
  await page.eval(`${q(INPUT)}.focus()`);
  await page.send('Input.imeSetComposition', { text: 'Đặng', selectionStart: 4, selectionEnd: 4 });
  await page.eval(`${q(INPUT)}.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'Enter', isComposing: true, bubbles: true, cancelable: true
  })); ${q('#author-search')}.requestSubmit()`);
  check(await page.eval(`!new URL(location.href).searchParams.has('q') && ${HAS_ROWS} === 12`),
    'IME keeps the current view and ignores submit before commit');
  await page.send('Input.insertText', { text: pen });
  await fill(page, INPUT, pen);
  await enter(page);
  await page.waitFor(`new URL(location.href).searchParams.get('q') === ${JSON.stringify(pen)} && ${HAS_ROWS} === 12`,
    'Vietnamese query after IME commit');
  await page.waitFor(`document.activeElement?.id === 'author-search-results-heading'`, 'focus after submitting search');

  let held;
  let oldNetworkId;
  let resolveOldFinished;
  const old = new Promise(resolve => { held = resolve; });
  const oldFinished = new Promise(resolve => { resolveOldFinished = resolve; });
  const observeOld = message => {
    if (message.sessionId === page.sessionId && message.params?.requestId === oldNetworkId
        && ['Network.loadingFinished', 'Network.loadingFailed'].includes(message.method)) {
      resolveOldFinished();
    }
  };
  page.cdp.listeners.push(observeOld);
  const releaseOld = await intercept(page, base + SEARCH + '?*', ({ requestId, networkId, request }) => {
    if (new URL(request.url).searchParams.get('q') === 'truy van cu') {
      oldNetworkId = networkId;
      held(requestId);
    } else page.send('Fetch.continueRequest', { requestId }).catch(() => {});
  });
  await fill(page, INPUT, 'truy van cu');
  await enter(page);
  const oldRequest = await Promise.race([old, new Promise((_, reject) =>
    setTimeout(() => reject(new Error('old query was not intercepted')), 5000))]);
  await fill(page, INPUT, '@' + currentAuthor);
  await enter(page);
  await page.waitFor(`${q(row(currentAuthor))} && ${HAS_ROWS} === 1`, 'new query resolves while old transport waits');
  await page.send('Fetch.continueRequest', { requestId: oldRequest }).catch(() => {});
  await Promise.race([oldFinished, new Promise((_, reject) =>
    setTimeout(() => reject(new Error('delayed query did not finish or cancel')), 5000))]);
  page.cdp.listeners.splice(page.cdp.listeners.indexOf(observeOld), 1);
  await releaseOld();
  check(await page.eval(`(async () => {
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return new URL(location.href).searchParams.get('q') === '@${currentAuthor}'
      && ${HAS_ROWS} === 1 && ${q(row(currentAuthor))};
  })()`),
    'a delayed old response cannot change the committed scope');

  await fill(page, INPUT, pen);
  await enter(page);
  await page.waitFor(`${HAS_ROWS} === 12 && ${q('#author-search-more')}`, 'fresh query resets cursor');
  const failAppend = await intercept(page, base + SEARCH + '?*', ({ requestId, request }) => {
    if (new URL(request.url).searchParams.has('cursor')) {
      page.send('Fetch.fulfillRequest', {
        requestId, responseCode: 503,
        responseHeaders: [{name: 'Content-Type', value: 'application/json'}],
        body: Buffer.from('{"error":{"code":"dependency_unavailable"}}').toString('base64'),
      }).catch(() => {});
    } else page.send('Fetch.continueRequest', { requestId }).catch(() => {});
  });
  await page.eval(`${q('#author-search-more')}.focus()`);
  await enter(page);
  await page.waitFor(`document.querySelector('[role=alert]')?.textContent.includes('Chưa tải thêm')`, 'append outage has recovery');
  check(await page.eval(`${HAS_ROWS} === 12 && document.activeElement.id === 'author-search-more'`),
    'append outage preserves the prior page and keeps the retry focused');
  await failAppend();
  await enter(page);
  await page.waitFor(`${HAS_ROWS} === 16 && ${q(row(secondPageAuthor))}`, 'real second page after retry');
  await page.eval(`${q(row(secondPageAuthor))}.focus()`);
  await enter(page);
  await page.waitFor(`location.pathname === '/@${secondPageAuthor}' && ${q('#author-profile-heading')}`,
    'profile of second-page author without works');
  await back(page);
  await page.waitFor(`location.search.includes('mode=authors') && ${q(row(secondPageAuthor))}
    && document.activeElement?.id === 'search-author-${secondPageAuthor}'`, 'Back restores mode, query, appended row and keyboard focus');
  check(await page.eval(`new URL(location.href).searchParams.get('q') === ${JSON.stringify(pen)}`),
    'Back keeps exact Vietnamese search query');

  await page.eval(`${q('#author-search-first')}.focus()`);
  await enter(page);
  await page.waitFor(`${HAS_ROWS} === 12 && ${q(row(currentAuthor))}`, 'first page action clears restored cursor');

  await page.goto(base + queryUrl(pen));
  await page.waitFor(`${HYDRATED} && ${HAS_ROWS} === 12`, 'restore first page before auth');
  await provision(page, 'alpha');
  await page.goto(base + queryUrl(pen));
  const action = `document.querySelector(${JSON.stringify(row(currentAuthor))})?.closest('li').querySelector('button')`;
  await page.waitFor(`${action}?.textContent.trim() === 'Theo dõi'`, 'first real session gets the Follow button');
  check(await page.eval(`(() => {
    const paths = performance.getEntriesByType('resource').map(entry => new URL(entry.name).pathname);
    return paths.filter(path => path === '/api/v1/me/follows/membership').length === 1
      && !paths.some(path => path.endsWith('/relationship/me'));
  })()`), 'the visible author page uses one membership batch and no per-row reads');
  await page.eval(`${action}.focus()`);
  await enter(page);
  await page.waitFor(`${action}?.textContent.trim() === 'Bỏ theo dõi'`,
    'Follow writes through the Graph owner and reflects committed membership');
  const relationship = await fetchInPage(page, SEARCH + '/' + currentAuthor + '/relationship/me');
  check(relationship.status === 200 && JSON.parse(relationship.body).following,
    'the first viewer’s confirmed membership is persisted by Graph');
  check((await fetchInPage(page, SEARCH + '?q=' + encodeURIComponent(pen))).body === guest,
    'first viewer gets the same public JSON bytes, order and cursor as the guest');

  check((await fetchInPage(page, ENDPOINTS.session, { method: 'DELETE' })).status === 204, 'first session signs out');
  await provision(page, 'beta');
  await page.goto(base + queryUrl(pen));
  await page.waitFor(`${action}?.textContent.trim() === 'Theo dõi'`, 'second real session sees only its own Follow state');
  check((await fetchInPage(page, SEARCH + '?q=' + encodeURIComponent(pen))).body === guest,
    'second viewer gets identical public search bytes after a private Follow');
  check(await page.eval(`!document.querySelector(${JSON.stringify(row(currentAuthor))}).contains(${action})`),
    'profile link and Follow remain distinct keyboard targets');
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  check([currentAuthor, 'tac-gia-thu-02'].every(handle => nodes.some(node =>
    node.role?.value === 'link' && node.name?.value === `Mở hồ sơ ${pen} (@${handle})`)),
    'AX names distinguish identical pen names by Handle');
  check(nodes.some(node => node.role?.value === 'button' && node.name?.value === 'Theo dõi'
      && node.description?.value?.includes('@' + currentAuthor)),
    'AX describes Follow in the context of its author');

  for (const width of [320, 390, 834, 1024, 1440]) {
    for (const theme of ['light', 'dark']) {
      for (const scale of [100, 200]) {
        await page.viewport(width);
        const layout = await page.eval(`(async () => {
          const root = document.documentElement;
          root.dataset.theme = '${theme}';
          root.style.fontSize = '${scale}%';
          await document.fonts.ready;
          await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
          await Promise.all(document.getAnimations().filter(animation =>
            Number.isFinite(animation.effect?.getComputedTiming().endTime))
            .map(animation => animation.finished.catch(() => {})));
          const link = ${q(row(currentAuthor))};
          const targets = [link, ${q(INPUT)}, ${action}];
          return root.scrollWidth <= root.clientWidth + 1 && targets.every(element => {
            const rect = element.getBoundingClientRect();
            return rect.width > 0 && rect.left >= -1 && rect.right <= root.clientWidth + 1;
          });
        })()`);
        check(layout, `author search reflow ${width}px / ${theme} / ${scale}%`);
        if (width === 390 && scale === 100) {
          await textContrast(page, theme, 'author-search', [INPUT, '#author-search-results-heading',
            row(currentAuthor) + ' .vk-ui-byline__name', '#search-follow-' + currentAuthor]);
        }
        if (width === 320 && theme === 'dark' && scale === 200
            || width === 1440 && theme === 'light' && scale === 100) {
          await page.capture(`author-search-${width}-${theme}-${scale}`, {
            boundary: 'Chrome + native Identity + scratch App PG', scenario: 'authors', textScale: scale,
          });
        }
      }
    }
  }
  await page.viewport(1280);
  await page.click('#explore-mode-following');
  const emptyFeedLink = '[data-feed-state=no-follows] a[href="/?mode=authors"]';
  await page.waitFor(q(emptyFeedLink), 'empty Following Feed offers author search');
  await page.click(emptyFeedLink);
  await page.waitFor(`${q(INPUT)} && ${HAS_ROWS} === 12 && location.search === '?mode=authors'`,
    'Feed’s author link opens the public suggestions in the shared route');

  // Revisit a native profile document, then traverse again before its WASM can hydrate.
  // This reproduces the two-document history timing reported on #293.
  const rapidQuery = "dang minh hoa";
  await page.goto(base + queryUrl(rapidQuery));
  await page.waitFor(`${HYDRATED} && ${q(row(currentAuthor))}`, 'first search document is ready');
  await page.eval(`${q(row(currentAuthor))}.focus()`);
  await enter(page);
  await page.waitFor(`location.pathname === '/@${currentAuthor}' && ${q('#author-profile-heading')}`, 'profile in history');
  await page.goto(base + queryUrl('khong co but danh nay'));
  await page.waitFor(q('#author-search-empty'), 'second search document is ready');
  await backThroughUnhydratedProfile(page, base, '/@' + currentAuthor, '#author-profile-heading', rapidQuery);
  await page.waitFor(`${HYDRATED} && !${q('#author-profile-heading')} && ${q(row(currentAuthor))}
    && document.activeElement?.id === 'search-author-${currentAuthor}'`, 'rapid Back restores author mode, DOM and focus');

  await page.viewport(1280);
  await page.goto(base + queryUrl(pen));
  await page.waitFor(`${HYDRATED} && ${q(row(currentAuthor))}`, 'search before profile retirement fixture');
  await page.eval(`${q(row(currentAuthor))}.focus()`);
  await enter(page);
  await page.waitFor(`location.pathname === '/@${currentAuthor}' && ${q('#author-profile-heading')}`, 'open current Handle before retirement');
  // Commit the browser fixture at the Social fence before cleanup. The Rust adapter and its
  // retry are exercised by the PostgreSQL suite.
  authorFixtureSql(`BEGIN;
    INSERT INTO social_retirements (principal_id, operation_id)
      SELECT principal_id, 'f720c1fb-5548-4d54-95de-15278fdddd81'
        FROM author_profiles WHERE current_handle = 'tac-gia-thu-01';
    UPDATE author_profiles SET current_handle = NULL WHERE current_handle = 'tac-gia-thu-01';
    COMMIT;`);
  const oldPath = '/@ten-cu-rieng';
  const retired = await fetch(base + oldPath, { redirect: 'manual' });
  check(retired.status === 410 && retired.headers.get('cache-control') === 'no-store'
    && !retired.headers.has('location') && !(await retired.text()).includes(pen),
    'an old URL has a retirement page and drops identity and redirect');
  await page.eval(`setTimeout(() => location.replace(${JSON.stringify(oldPath)}), 0); true`);
  await page.waitFor(`${HYDRATED} && location.pathname === ${JSON.stringify(oldPath)} && ${q('#profile-unavailable')}`,
    'the old Handle renders a retirement profile document in Chrome');
  await page.goto(base + queryUrl('khong co but danh nay'));
  await page.waitFor(q('#author-search-empty'), 'second search document after the retirement profile is ready');
  await backThroughUnhydratedProfile(page, base, oldPath, '#profile-unavailable', pen);
  await page.waitFor(`${HYDRATED} && new URL(location.href).searchParams.get('mode') === 'authors'
    && !${q('#profile-unavailable')} && ${q('#author-search-list')} && !${q(row(currentAuthor))}
    && document.activeElement?.id === 'author-search-results-heading'`, 'rapid Back from the retired alias restores author results and fallback focus');
  // A forward traversal must still land on the 410 document, with the alias reserved.
  const { entries, currentIndex } = await page.send('Page.getNavigationHistory', {});
  await page.send('Page.navigateToHistoryEntry', { entryId: entries[currentIndex + 1].id });
  await page.waitFor(`${HYDRATED} && location.pathname === ${JSON.stringify(oldPath)} && ${q('#profile-unavailable')}`,
    'Forward from search keeps the historical URL on the retirement page');
  await back(page);
  await page.waitFor(`new URL(location.href).searchParams.get('q') === ${JSON.stringify(pen)}
    && ${q('#author-search-list')} && !${q(row(currentAuthor))}
    && document.activeElement?.id === 'author-search-results-heading'`, 'Back excludes retired author and focuses results');
  authorFixtureSql('UPDATE author_handle_rollout SET ready = false');
  const unavailable = await fetch(base + MODE);
  const unavailableHtml = await unavailable.text();
  check(unavailable.status === 503 && unavailableHtml.includes('Chưa tải được danh sách tác giả.')
    && !unavailableHtml.includes('Chưa có gợi ý tác giả.'), 'SSR unavailable and legitimate empty stay distinct');
  authorFixtureSql('UPDATE author_handle_rollout SET ready = true');
}
