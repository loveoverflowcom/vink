// #295 and #354: real SSR, Chrome and native Identity against disposable PostgreSQL databases.
import { fill, HYDRATED, q } from './auth-smoke.mjs';
import { authorFixtureSql } from './author-fixtures.mjs';

const MODE = '/?mode=authors';
const SEARCH = '/api/v1/authors';
const INPUT = '#author-search-query';
const HEADING = '#author-search-results-heading';
const NOTE = '#author-search-note';
const EMPTY = '#author-search-empty';
const IN_WORKS = 'a#author-search-in-works';
const CLEAR = 'a#author-search-clear';
const ERROR = '#author-search-error';
const LOADING = '#author-search-loading';
const NOTICE = '#author-search-notice';
const SELF = '.vk-author-search__self';
const COPY = {
  suggestions: 'Gợi ý tác giả',
  matches: query => `Tác giả khớp “${query}”`,
  note: 'Ai cũng thấy cùng danh sách gợi ý này.',
  empty: query => `Chưa có tác giả khớp “${query}”.`,
  emptySuggestions: 'Chưa có gợi ý tác giả.',
  hint: 'Thử bút danh khác, hoặc nhập chính xác @handle.',
  inWorks: 'Tìm trong tác phẩm',
  clear: 'Xóa tìm kiếm',
  unavailable: 'Chưa tìm được lúc này',
  unavailableDetail: 'Kết quả chưa tải xong – chưa thể nói là không có tác giả.',
  retry: 'Thử lại',
  loading: 'Đang tìm tác giả…',
  notice: 'Một hồ sơ không còn truy cập được; danh sách đã cập nhật.',
  self: 'Bạn',
};
const textOf = selector => `${q(selector)}?.textContent.trim()`;
const hasCopy = (selector, copy) => `${textOf(selector)} === ${JSON.stringify(copy)}`;
const searchParam = key => `new URL(location.href).searchParams.get(${JSON.stringify(key)})`;
const HAS_ROWS = 'document.querySelectorAll("#author-search-list > li").length';
const row = handle => '#search-author-' + handle;
const currentAuthor = 'tac-gia-thu-01';
const secondPageAuthor = 'tac-gia-thu-15';
const pen = 'Đặng Minh Hòa';
const absentQuery = 'Không có bút danh này';
const queryUrl = (query) => MODE + '&q=' + encodeURIComponent(query);
const ENDPOINTS = {
  register: '/api/v1/auth/password/register',
  principal: '/api/v1/auth/principal',
  session: '/api/v1/auth/session',
  ownProfile: '/api/v1/me/profile',
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

async function discoveryModeStability(page, base, check) {
  const works = '#explore-mode-works';
  const following = '#explore-mode-following';
  const authors = '#explore-mode-authors';
  const mode = selector => `${q(selector)}?.getAttribute('aria-current') === 'page'`;
  const activate = async selector => {
    // Keep the current scroll position. The click helper centres its target even when visible.
    await page.eval(`${q(selector)}.focus({preventScroll: true})`);
    await page.key('Enter');
  };
  for (const [width, scale] of [[1280, 100], [390, 100], [390, 150], [320, 150], [320, 200]]) {
    await page.viewport(width, 850);
    await page.goto(base + '/');
    await page.waitFor(`${HYDRATED} && ${q(following)}`, 'shared discovery header');
    const labelFont = 'parseFloat(getComputedStyle(document.querySelector(".vk-explore-modes a")).fontSize)';
    const regularLabelFont = await page.eval(labelFont);
    await page.eval(`(() => {
      const root = document.documentElement;
      if (${scale} === 150) root.dataset.text = '150';
      if (${scale} === 200) root.style.fontSize = '200%';
    })()`);
    await page.send('Emulation.setEmulatedMedia', { features: [
      { name: 'prefers-reduced-motion', value: width === 320 ? 'reduce' : 'no-preference' },
    ] });
    // The root style can resolve before descendant rem lengths are laid out.
    await page.waitFor(`Math.abs(${labelFont} - ${regularLabelFont * scale / 100}) < 0.05`,
      `mode labels reach the ${scale}% text scale before measuring their position`);
    await page.eval("document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))");
    await page.eval(`window.discoverySnapshot = (() => {
      const selector = 'header.vk-explore__top, #explore-heading h1, .vk-brand__lockup, .vk-explore-modes, .vk-explore-modes a';
      const nodes = [...document.querySelectorAll(selector)];
      const rects = nodes.map(node => node.getBoundingClientRect());
      const origin = performance.timeOrigin;
      return () => {
        const after = [...document.querySelectorAll(selector)];
        const shift = Math.max(...after.flatMap((node, index) => {
          const landed = node.getBoundingClientRect();
          return ['top', 'left', 'right', 'height'].map(edge => Math.abs(landed[edge] - rects[index][edge]));
        }));
        return {
          shift,
          scrollY,
          sameNodes: nodes.length === after.length && nodes.every((node, index) => node === after[index]),
          sameDocument: performance.timeOrigin === origin,
          fits: document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1,
          wordsIntact: [...document.querySelectorAll('.vk-explore-modes a')].every(link => {
            const text = [...link.childNodes].find(node => node.nodeType === Node.TEXT_NODE);
            return Boolean(text) && [...text.textContent.matchAll(/\\S+/g)].every(word => {
              const range = document.createRange();
              range.setStart(text, word.index);
              range.setEnd(text, word.index + word[0].length);
              const lines = [...range.getClientRects()];
              return lines.every(line => Math.abs(line.top - lines[0].top) <= 1);
            });
          }),
        };
      };
    })()`);
    const scaleLabel = scale === 200 ? 'stress 200% (root font-size override)' : `app CSS ${scale}%`;
    const stable = async label => {
      const got = await page.eval('window.discoverySnapshot()');
      check(got.sameNodes && got.sameDocument && got.fits && got.wordsIntact && got.shift <= 1,
        `#473 ${width}px / ${scaleLabel} ${label} (${JSON.stringify(got)})`);
    };

    await page.eval(`${q(works)}.focus()`);
    await page.key('Tab');
    check(await page.eval(`document.activeElement === ${q(following)} && ${q(following)}.matches(':focus-visible')`),
      `#473 ${width}px keyboard reaches Following`);
    await page.key('Enter');
    await page.waitFor(`${mode(following)} && ${q('[data-feed-state=guest]')}`, 'guest Following selected');
    await stable('Following');

    const held = [];
    const release = await intercept(page, base + SEARCH + '*', params => held.push(params.requestId));
    try {
      await activate(authors);
      await page.waitFor(`${mode(authors)} && ${q(LOADING)}`, 'Authors selected while its response is pending');
      const deadline = Date.now() + 5000;
      while (held.length === 0 && Date.now() < deadline) {
        await new Promise(resolve => setTimeout(resolve, 25));
      }
      check(held.length > 0, '#473 author request held at the browser transport');
      await stable('Authors loading');
      if (width === 390) {
        await page.capture('discover-shell-authors-loading-390', {
          requiredTargets: ['#explore-heading h1', '.vk-brand', authors, LOADING],
        });
      }
      if (width === 320) {
        check(await page.eval(`getComputedStyle(${q('.vk-explore-modes')}, '::after').transitionProperty === 'none'`),
          '#473 reduced motion moves the indicator immediately');
      }
      await activate(works);
      await page.waitFor(`${mode(works)} && ${q('#public-results')}`, 'Works selected before Authors resolves');
      for (const requestId of held.splice(0)) {
        await page.send('Fetch.continueRequest', { requestId }).catch(() => {});
      }
      await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
      check(await page.eval(`!${q(HEADING)} && ${mode(works)}`), '#473 late Authors result stays outside Works');
      await stable('Works after a rapid switch');
    } finally {
      for (const requestId of held) await page.send('Fetch.continueRequest', { requestId }).catch(() => {});
      await release();
    }
    await page.eval('history.back()');
    await page.waitFor(`${mode(authors)} && ${HAS_ROWS} === 12`, 'Back restores the Authors URL and results');
    await stable('Back to Authors');
    await page.eval('history.forward()');
    await page.waitFor(mode(works), 'Forward restores Works selection');
    await stable('Forward to Works');
  }
  await page.send('Emulation.setEmulatedMedia', { features: [] });
  await page.viewport(1280);
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

async function waitForPresentation(page, expression, label, presentation) {
  try {
    await page.waitFor(expression, label, 8000);
  } catch (error) {
    if (error.message !== `timed out waiting for ${label}`) throw error;
    presentation(false, error.message);
    return;
  }
  presentation(true, label);
}

async function emptyPresentation(page, base, presentation) {
  await fill(page, INPUT, absentQuery);
  await enter(page);
  await page.waitFor(`${searchParam('q')} === ${JSON.stringify(absentQuery)}
    && ${q(EMPTY)} && ${HAS_ROWS} === 0`, 'search succeeds with zero matching authors');
  const empty = await page.eval(textOf(EMPTY));
  presentation(await page.eval(`${q(EMPTY)}.getAttribute('role') === 'status'`)
    && empty.includes(COPY.empty(absentQuery)) && empty.includes(COPY.hint),
    `S2 empty success has the query and hint (found ${JSON.stringify(empty)})`);

  const canSearchWorks = await page.eval(`Boolean(${q(IN_WORKS)})`);
  presentation(await page.eval(hasCopy(IN_WORKS, COPY.inWorks)), 'S2 empty success offers Tìm trong tác phẩm');
  if (canSearchWorks) {
    await page.click(IN_WORKS);
    await waitForPresentation(page, `location.pathname === '/' && ${searchParam('mode')} !== 'authors'
      && ${searchParam('q')} === ${JSON.stringify(absentQuery)}
      && ${q('#public-query')}?.value === ${JSON.stringify(absentQuery)}`,
      'S2 works search receives the same query', presentation);
    await page.goto(base + queryUrl(absentQuery));
    await page.waitFor(`${HYDRATED} && ${q(EMPTY)}`, 'return to the empty author query before clearing');
  }

  const canClear = await page.eval(`Boolean(${q(CLEAR)})`);
  presentation(await page.eval(hasCopy(CLEAR, COPY.clear)), 'S2 empty success offers Xóa tìm kiếm');
  if (canClear) {
    await page.click(CLEAR);
    await waitForPresentation(page, `location.search === '?mode=authors' && ${hasCopy(HEADING, COPY.suggestions)}
      && ${HAS_ROWS} === 12 && ${q(INPUT)}?.value === '' && document.activeElement === ${q(INPUT)}`,
      'S2 clear returns to suggestions and focuses the author search field', presentation);
  }
}

function unavailableAnswer(page, requestId) {
  return page.send('Fetch.fulfillRequest', {
    requestId, responseCode: 503,
    responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
    body: Buffer.from('{"error":{"code":"dependency_unavailable"}}').toString('base64'),
  });
}

async function initialOutage(page, base, check, presentation) {
  const query = '@' + secondPageAuthor;
  const release = await intercept(page, base + SEARCH + '?*', ({ requestId, request }) => {
    if (new URL(request.url).searchParams.get('q') === query) {
      unavailableAnswer(page, requestId).catch(() => {});
    } else {
      page.send('Fetch.continueRequest', { requestId }).catch(() => {});
    }
  });
  try {
    await fill(page, INPUT, query);
    await enter(page);
    await page.waitFor(q(ERROR), 'first author search page receives the injected 503');
    check(await page.eval(`!${q(EMPTY)} && ${HAS_ROWS} === 0`),
      'initial author search outage is distinct from a successful empty result');
    const error = await page.eval(textOf(ERROR));
    presentation(await page.eval(`${q(ERROR)}.getAttribute('role') === 'alert'`)
        && error.includes(COPY.unavailable) && error.includes(COPY.unavailableDetail),
      `S3 initial outage explains that results are unavailable (found ${JSON.stringify(error)})`);
  } finally {
    await release();
  }

  // The previous SSR view rendered its retry as a sibling of the alert.
  const retry = `${ERROR} a, ${ERROR} + a`;
  check(await page.eval(hasCopy(retry, COPY.retry)), 'initial outage offers a Thử lại link');
  await page.click(retry);
  await waitForPresentation(page, `${q(row(secondPageAuthor))} && ${HAS_ROWS} === 1 && !${q(ERROR)}`,
    'S3 Thử lại restores the exact Handle result after the injected 503', presentation);
}

function assertNotice(page, presentation) {
  return waitForPresentation(page, `(() => {
    const notices = document.querySelectorAll(${JSON.stringify(NOTICE)});
    return notices.length === 1 && notices[0].matches('p[role="status"]')
      && notices[0].textContent.trim() === ${JSON.stringify(COPY.notice)};
  })()`, 'S5 Back announces the removed profile exactly once', presentation);
}

async function openOwnRow(page, base) {
  const profile = await fetchInPage(page, ENDPOINTS.ownProfile);
  if (profile.status !== 200) throw new Error('own Profile fixture read: ' + profile.status);
  const { handle } = JSON.parse(profile.body);
  if (!handle) throw new Error('own Profile fixture has no public Handle');
  const url = base + queryUrl('@' + handle);
  await page.goto(url);
  await page.waitFor(`${HYDRATED} && ${q(row(handle))} && ${HAS_ROWS} === 1`, 'exact provisioned Handle result is hydrated');
  return { url, handle };
}

async function longHandleReflow(page, base, presentation) {
  const handle = 'zzztacgiakiemthudongtieudevaketqua0123';
  const id = 'f211b8b0-31d4-4393-8203-25c1c4bd08cf';
  // Add this author after pagination and retirement checks so their fixture counts stay stable.
  authorFixtureSql(`BEGIN;
    INSERT INTO principals (id, display_name) VALUES ('${id}', 'Tác giả kiểm tra reflow');
    INSERT INTO principal_bindings (principal_id, authority, subject_id)
      VALUES ('${id}', 'vink-identity', '${id}');
    INSERT INTO author_handle_names (principal_id, handle) VALUES ('${id}', '${handle}');
    INSERT INTO author_profiles (principal_id, current_handle) VALUES ('${id}', '${handle}');
    COMMIT;`);

  const query = '@' + handle;
  const link = q(row(handle));
  await page.goto(base + queryUrl(query));
  await page.waitFor(`${HYDRATED} && ${link} && ${HAS_ROWS} === 1`, 'exact unbroken Handle fixture');
  for (const [width, theme, scale] of [[320, 'dark', 200], [390, 'light', 100]]) {
    await page.viewport(width);
    const layout = await page.eval(`(async () => {
      const root = document.documentElement;
      root.dataset.theme = '${theme}';
      root.style.fontSize = '${scale}%';
      await document.fonts.ready;
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const heading = ${q(HEADING)};
      const row = ${link}.closest('li');
      const handle = row.querySelector('.vk-ui-byline__handle');
      const edges = ({ left, right }) => [left, right];
      const textEdges = node => {
        const range = document.createRange();
        range.selectNodeContents(node);
        return Array.from(range.getClientRects(), edges);
      };
      return {
        clientWidth: root.clientWidth, scrollWidth: root.scrollWidth,
        heading: edges(heading.getBoundingClientRect()),
        headingText: textEdges(heading),
        row: edges(row.getBoundingClientRect()),
        handleText: textEdges(handle),
        fullText: heading.textContent.includes(${JSON.stringify(query)})
          && handle.textContent.trim() === ${JSON.stringify(query)},
      };
    })()`);
    const inside = ([left, right]) => right > left && left >= -1 && right <= layout.clientWidth + 1;
    const bounds = [layout.heading, layout.row, ...layout.headingText, ...layout.handleText];
    presentation(layout.fullText && layout.scrollWidth <= layout.clientWidth + 1 && bounds.every(inside),
      `S6 exact long @handle fits the heading and row at ${width}px/${theme}/${scale}% (${JSON.stringify(layout)})`);
  }
}

export async function authorSearch(page, { args, check, textContrast }) {
  // Run every presentation contract against both baseline and candidate using the same scenario.
  // Collect only presentation failures so the legacy fixture and interaction checks stay fail-fast.
  const failures = [];
  const presentation = (condition, label) => {
    if (condition) check(condition, label);
    else {
      console.error(`  FAIL ${label}`);
      failures.push(label);
    }
  };
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

  await discoveryModeStability(page, base, check);
  await page.goto(base + '/');
  const entry = '#explore-mode-authors';
  await page.waitFor(`${HYDRATED} && ${q(entry)}`, 'author search mode in Khám phá');
  check(await page.eval(`${q('#explore-mode-works')}.parentElement === ${q(entry)}.parentElement
    && ${q('#explore-mode-following')}.parentElement === ${q(entry)}.parentElement`),
    'work, following and author modes share one navigation region');
  await page.click(entry);
  await page.waitFor(`${HYDRATED} && ${HAS_ROWS} === 12`, 'guest author suggestions');
  presentation(await page.eval(`${hasCopy(HEADING, COPY.suggestions)}
    && ${q(NOTE)}?.tagName === 'P' && ${hasCopy(NOTE, COPY.note)}`),
    'S1 suggestions have their own heading and public-list note');
  const guest = await fetch(base + SEARCH + '?q=' + encodeURIComponent(pen)).then(r => r.text());
  await page.click('#open-public-search');
  await page.waitFor(`document.activeElement === ${q(INPUT)}`, 'author search launcher focuses its field');
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

  const heading = await page.eval(textOf(HEADING));
  presentation(heading === COPY.matches(pen),
    `S1 heading includes the committed Vietnamese query (found ${JSON.stringify(heading)})`);

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
  await page.waitFor(q(LOADING), 'delayed author query exposes a pending state');
  const loading = await page.eval(textOf(LOADING));
  presentation(loading === COPY.loading,
    `S4 delayed query says Đang tìm tác giả… (found ${JSON.stringify(loading)})`);
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

  const handleSelector = row(currentAuthor) + ' .vk-ui-byline__handle';
  const nameSelector = row(currentAuthor) + ' .vk-ui-byline__name';
  presentation(await page.eval(`${hasCopy(handleSelector, '@' + currentAuthor)}
    && ${textOf(handleSelector + ' mark')}?.replace(/^@/, '') === ${JSON.stringify(currentAuthor)}
    && ${hasCopy(nameSelector, pen)} && !${q(nameSelector + ' mark')}`),
    'S6 exact @ query highlights the Handle and keeps the pen name plain');

  await emptyPresentation(page, base, presentation);
  await initialOutage(page, base, check, presentation);

  await fill(page, INPUT, pen);
  await enter(page);
  await page.waitFor(`${HAS_ROWS} === 12 && ${q('#author-search-more')}`, 'fresh query resets cursor');
  const failAppend = await intercept(page, base + SEARCH + '?*', ({ requestId, request }) => {
    if (new URL(request.url).searchParams.has('cursor')) {
      unavailableAnswer(page, requestId).catch(() => {});
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

  const own = await openOwnRow(page, base);
  const ownLink = q(row(own.handle));
  const selfInRow = `${ownLink}?.closest('li').querySelector(${JSON.stringify(SELF)})`;
  const ssrIsPublic = await page.eval(`(async () => {
    const response = await fetch(${JSON.stringify(own.url)});
    if (!response.ok) return false;
    const document = new DOMParser().parseFromString(await response.text(), 'text/html');
    const row = ${ownLink}?.closest('li');
    return Boolean(row && !row.querySelector(${JSON.stringify(SELF + ', button')}));
  })()`);
  presentation(ssrIsPublic, 'S7 signed-in SSR shows the public row without viewer text or Follow');
  await waitForPresentation(page, `${selfInRow}?.textContent.trim() === ${JSON.stringify(COPY.self)}
    && !${ownLink}.contains(${selfInRow}) && !${ownLink}.closest('li').querySelector('button')
    && performance.getEntriesByType('resource').filter(entry =>
      new URL(entry.name).pathname === ${JSON.stringify(ENDPOINTS.ownProfile)}).length === 1`,
    'S7 owner sees Bạn outside the profile link, has no Follow button and uses one own-profile read', presentation);

  check((await fetchInPage(page, ENDPOINTS.session, { method: 'DELETE' })).status === 204, 'first session signs out');
  await page.goto(own.url);
  await page.waitFor(`${HYDRATED} && ${ownLink}`, 'guest opens the same exact Handle result');
  presentation(await page.eval(`!${selfInRow} && !${ownLink}.closest('li').querySelector('button')`),
    'S7 guest sees neither Bạn nor a Follow button');
  await provision(page, 'beta');
  await page.goto(own.url);
  await page.waitFor(`${ownLink}?.closest('li').querySelector('button')?.textContent.trim() === 'Theo dõi'`,
    'second session can follow the first viewer as an author');
  presentation(await page.eval(`!${selfInRow}`), 'S7 a different account does not inherit Bạn');
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
          const list = document.querySelector('#author-search-list');
          const rowEl = link.closest('li');
          const rowStyle = getComputedStyle(rowEl);
          const probe = document.createElement('span');
          probe.style.background = 'var(--vk-bg-content)';
          document.body.append(probe);
          const bgContent = getComputedStyle(probe).backgroundColor;
          probe.remove();
          const cardOk = rowStyle.borderTopLeftRadius === '20px' && rowStyle.backgroundColor === bgContent
            && rowStyle.borderBottomWidth === '0px' && getComputedStyle(list).rowGap === '8px';
          const targets = [link, ${q(INPUT)}, ${action}];
          return cardOk && root.scrollWidth <= root.clientWidth + 1 && targets.every(element => {
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
  presentation(await page.eval(`!${q(NOTICE)}`), 'S5 Back to a surviving author does not announce a missing profile');

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
  await assertNotice(page, presentation);
  const freshResults = await fetch(base + queryUrl(pen));
  presentation(freshResults.ok && !(await freshResults.text()).includes(COPY.notice),
    'S5 fresh SSR never contains the Back notice');
  authorFixtureSql('UPDATE author_handle_rollout SET ready = false');
  const unavailable = await fetch(base + MODE);
  const unavailableHtml = await unavailable.text();
  check(unavailable.status === 503 && !unavailableHtml.includes(COPY.emptySuggestions),
    'SSR unavailable and legitimate empty stay distinct');
  presentation(unavailableHtml.includes(COPY.unavailable) && unavailableHtml.includes(COPY.unavailableDetail),
    'S3 SSR unavailable uses the same heading and explanation as hydrated search');
  authorFixtureSql('UPDATE author_handle_rollout SET ready = true');

  await longHandleReflow(page, base, presentation);
  if (failures.length) {
    check(page.errors.length === 0, `no console errors or exceptions: ${page.errors.join(' | ')}`);
    throw new Error(`author search presentation: ${failures.length} failed assertions (see S1–S7 above)`);
  }
}
