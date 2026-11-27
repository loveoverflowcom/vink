// Runs the real routes and hydrated state owner. Only the API responses under test are
// controlled by CDP so an old count/receipt can be delivered in a reproducible order.
import assert from 'node:assert/strict';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const json = path => JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'));
const examples = json('../../../services/vink-app/contracts/http/v1/fixtures/notifications-examples.json');
const gateway = json('../../../services/vink-app/contracts/http/v1/fixtures/notifications-gateway.json');
const bell = '#notification-bell';
const mobile = '#notification-bell-mobile';
const popup = '#notification-popover';
const inbox = '/api/v1/me/notifications';
const read = `${inbox}/read`;
const markAll = `${inbox}/mark-all-read`;
const markButton = '#notification-read-all';
const markLabel = 'Đánh dấu tất cả là đã đọc';
const work = '33333333-3333-4333-8333-333333333333';
const privateTitle = 'Mùa gió chướng';
const denied = { error: { code: 'not_found', message: 'not found' } };
const review = json('../../../services/vink-app/contracts/http/v1/fixtures/publication.review.json');
const adverse = examples.decisions.find(decision => decision.kind === 'comment_moderation_decision');
const apiReview = `/api/v1/works/${work}/publication/review`;
const unavailable = { error: { code: 'dependency_unavailable', message: 'unavailable' } };
const sessionId = { A: '11111111-1111-4111-8111-111111111111', B: '22222222-2222-4222-8222-222222222222' };
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

async function fixture(page) {
  const calls = [];
  const trace = [];
  const started = performance.now();
  const state = { session: 'A', countMode: 'fresh', staleCount: 8, firstFailure: false, count: 3, snapshot: 45, seen: 0,
    pageSize: null, marked: false, moreFailure: true, holdCount: false, holdMark: false,
    markFailure: false, holdFirst: false, checkFailure: false, holdSession: false, standing: 'anonymous' };
  const held = new Map();
  const root = gateway.response.items[0];
  const [tham, rejected] = examples.inbox.items;
  const revision = review.state.revisions[0];
  const allRows = [
    { ...rejected, target: { ...rejected.target, work_id: work, revision_id: revision.revision_id,
      revision_created_at: revision.created_at } },
    tham, root,
    { ...root, group_id: '019a9b20-1111-7000-8000-000000000004',
      target: { ...root.target, kind: 'work_comment_reply', root_id: root.target.comment_id },
      summary: { kind: 'comment_reply_rollup', action_required: false, reply_to_viewer: true } },
    { ...root, group_id: '019a9b20-1111-7000-8000-000000000005',
      target: { ...root.target, kind: 'work_comment_endorsement' },
      summary: { kind: 'comment_ung_rollup', action_required: false }, excerpt: null },
    { ...root, group_id: '019a9b20-1111-7000-8000-000000000006', target: adverse,
      summary: { action_required: false, kind: 'comment_moderation_removed' }, excerpt: null },
  ];
  const unique = [...new Map(allRows.map(row => [row.group_id, row])).values()];
  const source = unique.map((row, index) => ({ ...row,
    updated_at: new Date(Date.now() - (index < 3 ? index * 300_000 : 86_400_000 * 2)).toISOString(),
  }));
  const snapshot = (scope, value = state.snapshot) => `s1.${value}.${scope}`;
  const requestBody = request => request.postData ? JSON.parse(request.postData) : null;
  const headers = (request, target) => Object.entries(request.headers).find(([name]) => name.toLowerCase() === target)?.[1];
  const reply = (requestId, status, body) => page.send('Fetch.fulfillRequest', {
    requestId, responseCode: status,
    responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
    body: Buffer.from(JSON.stringify(body)).toString('base64'),
  });
  const listener = (event) => {
    if (event.sessionId !== page.sessionId || event.method !== 'Fetch.requestPaused') return;
    const { request, requestId } = event.params;
    const { pathname: path, searchParams } = new URL(request.url);
    const scope = state.session;
    const expectScope = headers(request, 'x-vink-notification-scope');
    const body = requestBody(request);
    calls.push({ path, body, query: searchParams.toString(), scope: expectScope, method: request.method });
    let response;
    if (path === '/api/v1/auth/session') {
      response = state.checkFailure ? [503, unavailable]
        : [200, scope ? { state: 'active', principal_id: sessionId[scope] } : { state: state.standing }];
      if (state.holdSession) {
        state.holdSession = false;
        held.set('session', () => reply(requestId, ...response));
        return;
      }
    } else if (path === '/api/v1/auth/session/account') {
      response = scope ? [200, { principal_id: sessionId[scope], display_name: `Bút danh ${scope}`, notification_scope: scope }]
        : [401, { error: { code: 'unauthenticated' } }];
    } else if (path.startsWith(inbox) && (!scope || expectScope !== scope)) {
      response = [401, { error: { code: 'unauthenticated' } }];
    } else if (path === apiReview) {
      const fixture = structuredClone(review);
      Object.assign(fixture.state.revisions[0], { title: privateTitle, moderation: 'rejected',
        decision_reason: 'Lý do riêng của tác phẩm', can_appeal: true });
      Object.assign(fixture.decisions[0], { outcome: 'rejected', action_required: true, can_appeal: true, reason: 'Lý do riêng của tác phẩm' });
      response = scope === 'A' ? [200, fixture] : [404, denied];
    } else if (path === `${inbox}/count`) {
      const [fresh, stale] = examples.counts;
      const freshness = state.countMode === 'stale'
        ? { ...stale.freshness, last_known_count: state.staleCount, as_of_snapshot: snapshot(scope) }
        : { ...fresh.freshness, unseen_group_count: state.seen >= state.snapshot ? 0 : state.count,
          as_of_snapshot: snapshot(scope) };
      if (state.countMode === 'legacy') {
        // Replay the c277/c281 capture handler: HTTP 200, one required field missing, count fixed across Seen.
        delete freshness.count_is_lower_bound;
        freshness.unseen_group_count = state.count;
      }
      response = state.countMode === 'unavailable' ? [503, unavailable] : [200, { freshness }];
      if (state.holdCount) { state.holdCount = false; held.set('count', () => reply(requestId, ...response)); return; }
    } else if (path === inbox) {
      const more = searchParams.has('cursor');
      response = (more ? state.moreFailure : state.firstFailure) ? [503, unavailable] : [200, {
        inbox_snapshot: snapshot(scope), next_cursor: more || state.pageSize !== null ? null : 'next-fixture',
        items: (more ? source.slice(3) : source.slice(0, state.pageSize ?? 3)).map((row, index) => ({
          ...row, is_read: state.marked ? true : row.is_read,
          ...(state.pageSize !== null && { is_unseen: state.seen < state.snapshot && index < state.count }),
          target: scope === 'B' ? { ...row.target, work_title: 'Thông báo của tài khoản B' }
            : state.snapshot >= 46 && row.group_id === source[0].group_id
              ? { ...row.target, work_title: 'Tác phẩm có cập nhật' } : row.target,
        })),
      }];
      if (!more && state.holdFirst) {
        state.holdFirst = false;
        held.set('first', () => reply(requestId, ...response));
        return;
      }
    } else if (path === `${inbox}/seen`) {
      if (body.up_to_snapshot === snapshot(scope)) {
        state.seen = state.snapshot;
        response = [200, { seen_snapshot: snapshot(scope, state.seen) }];
      } else {
        response = [400, { error: { code: 'invalid_snapshot' } }];
      }
    } else if (path === read) {
      response = [200, { group_id: body.group_id, group_version: body.observed_group_version, is_read: true }];
      held.set('read', () => reply(requestId, ...response));
      return;
    } else if (path === markAll) {
      const answer = () => {
        if (state.markFailure) return reply(requestId, 503, unavailable);
        state.marked = true;
        return reply(requestId, 200, { up_to_snapshot: body.up_to_snapshot, groups_marked: 3 });
      };
      if (state.holdMark) { state.holdMark = false; held.set('mark', answer); return; }
      answer().catch(() => {});
      return;
    } else if (path.startsWith(`${inbox}/decisions/comments/`)) {
      response = [200, { ...examples.decisions.find(decision => decision.kind === 'comment_moderation_decision'),
        reason: { message: 'Lý do riêng của nhận xét' }, comment_body: 'Không được xuất hiện' }];
    } else {
      page.send('Fetch.continueRequest', { requestId }).catch(() => {});
      return;
    }
    if (state.pageSize !== null && [inbox, `${inbox}/count`, `${inbox}/seen`].includes(path)) {
      const [status, payload] = response;
      const freshness = payload.freshness;
      trace.push({ ms: Math.round(performance.now() - started), method: request.method, path, status,
        scopeMatches: expectScope === scope, fields: Object.keys(payload),
        ...(freshness && { freshness: freshness.status, count: freshness.unseen_group_count,
          countFields: Object.keys(freshness) }),
        ...(payload.items && { rows: payload.items.length, unseen: payload.items.filter(item => item.is_unseen).length }),
        ...(path.endsWith('/seen') && { snapshotMatches: body.up_to_snapshot === payload.seen_snapshot }),
      });
    }
    reply(requestId, ...response).catch(() => {});
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [
    { urlPattern: '*/api/v1/auth/session*' }, { urlPattern: `*${inbox}*` }, { urlPattern: `*${apiReview}` },
  ] });
  return { state, calls, trace, rows: source, held, async change(next) {
    state.session = next;
    await page.eval(`(() => { const channel = new BroadcastChannel('vink-session');
      channel.postMessage('changed'); setTimeout(() => channel.close(), 10); })()`);
  } };
}

async function eventually(predicate, detail) {
  const end = Date.now() + 10_000;
  while (Date.now() < end) {
    const value = predicate();
    if (value) return value;
    await sleep(25);
  }
  throw new Error(`timed out waiting for ${detail}`);
}

async function geometry(page, scope) {
  return page.eval(`(() => {
    const container = document.querySelector(${JSON.stringify(scope)});
    const targets = [...container.querySelectorAll('button,a[href]')]
      .filter(el => el.getClientRects().length && getComputedStyle(el).display !== 'none')
      .map(el => { const r = el.getBoundingClientRect(); return [r.width, r.height, el.textContent.trim()]; });
    return { overflow: document.documentElement.scrollWidth - innerWidth,
      innerOverflow: Math.max(0, ...[...container.querySelectorAll('.vk-notif-row, .vk-notif-popover__head, .vk-notif-popover__scroll')]
        .map(el => el.scrollWidth - el.clientWidth)), targets };
  })()`);
}

const viewport = [320, 390, 834, 1024, 1440];
async function preferences(page, width, theme, scale, height = 900) {
  await page.viewport(width, height);
  await page.send('Emulation.setEmulatedMedia', { features: [
    { name: 'prefers-color-scheme', value: theme }, { name: 'prefers-reduced-motion', value: 'reduce' },
  ] });
  await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 });
  await page.eval(`(async () => {
    document.documentElement.style.fontSize = '${scale}%';
    scrollTo(0, 0);
    await document.fonts.ready;
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  })()`);
  await page.waitFor(`innerWidth === ${width}
    && Math.round(parseFloat(getComputedStyle(document.documentElement).fontSize) / 16 * 100) === ${scale}
    && matchMedia('(prefers-color-scheme: ${theme})').matches`, 'painted capture preferences');
}

const mounted = `(document.documentElement.dataset.renderMode === 'csr'
  ? !!document.querySelector('#noi-dung') : document.documentElement.dataset.hydrated === 'true')`;
const row = '.vk-notif-row';
const visible = selector => `document.querySelector(${JSON.stringify(selector)})?.getBoundingClientRect().width > 0`;
const focused = selector => `document.activeElement?.matches(${JSON.stringify(selector)})`;

// A title or an empty/filled list can coexist with a failed count. Check both owners on /thong-bao.
function inboxObservation() {
  const body = document.querySelector('main .vk-notif-body');
  const rows = body?.querySelectorAll('.vk-notif-row');
  return {
    rows: rows?.length ?? null,
    unread: body?.querySelectorAll('.vk-notif-row--unread').length ?? null,
    empty: !!body?.querySelector('.vk-notif-empty'),
    listError: !!body?.querySelector('.vk-notif-error button'),
    countWarning: body?.querySelector('.vk-notif-note [role=status]')?.textContent ?? null,
    countRetry: !!body?.querySelector('#notification-count-retry'),
  };
}

async function captureInboxCases(page, args, { state, trace }) {
  const countPath = `${inbox}/count`, seenPath = `${inbox}/seen`;
  const unknown = 'Chưa tải được số thông báo mới.';
  await page.viewport(390, 844);
  await page.goto(`${args.base}/`);
  await page.waitFor(visible(mobile), 'signed-in home for notification cases');
  for (const [width, height, theme] of [[390, 844, 'light'], [1440, 900, 'dark']]) {
    const cases = ['empty', 'populated', 'count-error', 'list-error'];
    if (width === 390) cases.unshift('legacy-empty', 'legacy-populated');
    for (const name of cases) {
      const empty = name.endsWith('empty'), legacy = name.startsWith('legacy-');
      const countFailed = legacy || name === 'count-error', listFailed = name === 'list-error';
      const count = empty ? 0 : legacy ? 3 : 2;
      Object.assign(state, { pageSize: empty ? 0 : 3, count, seen: 0, marked: false,
        countMode: legacy ? 'legacy' : countFailed ? 'unavailable' : 'fresh', firstFailure: listFailed });
      trace.length = 0;
      await page.send('Emulation.setEmulatedMedia', { features: [
        { name: 'prefers-color-scheme', value: theme }, { name: 'prefers-reduced-motion', value: 'reduce' },
      ] });
      await page.eval("window.dispatchEvent(new Event('online'))");
      const badge = `document.querySelector('${mobile} .vk-ui-badge')`;
      const beforeBadge = countFailed ? 'unknown' : count ? 'count' : 'hidden';
      await eventually(() => trace.some(event => event.path === countPath), 'shell count response');
      await page.waitFor(`${badge}?.dataset.state === '${beforeBadge}' && ${badge}.textContent === '${countFailed || !count ? '' : count}'`, 'pre-inbox badge');
      await page.click(mobile);
      await page.waitFor("location.pathname === '/thong-bao'", 'bell opens inbox');
      await page.viewport(width, height);
      await eventually(() => trace.some(event => event.path === inbox), 'inbox response');
      if (!listFailed) {
        await eventually(() => {
          const seen = trace.findIndex(event => event.method === 'POST' && event.path === seenPath && event.snapshotMatches);
          return seen >= 0 && trace.slice(seen + 1).some(event => event.path === countPath);
        }, 'count refresh after scoped Seen receipt');
      }
      const rows = listFailed || empty ? 0 : 3;
      const expected = { rows, unread: rows, empty, listError: listFailed,
        countWarning: countFailed ? unknown : null, countRetry: countFailed };
      const readDom = `(${inboxObservation.toString()})()`;
      await page.waitFor(`JSON.stringify(${readDom}) === ${JSON.stringify(JSON.stringify(expected))}`, `${name} inbox and count`);
      const observed = await page.eval(readDom);
      const list = trace.find(event => event.path === inbox);
      const counts = trace.filter(event => event.path === countPath);
      assert.ok(trace.every(event => event.scopeMatches));
      assert.equal(list.status, listFailed ? 503 : 200);
      assert.equal(counts[0].status, name === 'count-error' ? 503 : 200);
      if (!listFailed) assert.equal(list.rows, rows);
      if (!countFailed) {
        assert.equal(counts[0].count, count);
        assert.equal(counts.at(-1).count, listFailed ? count : 0);
        assert.ok(counts.every(event => event.freshness === 'fresh' && event.countFields.includes('count_is_lower_bound')));
        if (!listFailed) assert.equal(list.unseen, count);
      }
      if (listFailed) assert.ok(trace.every(event => event.path !== seenPath));
      const normal = { ...expected, countWarning: null, countRetry: false };
      if (legacy) {
        assert.ok(counts.every(event => !event.countFields.includes('count_is_lower_bound')));
        assert.throws(() => assert.deepEqual(observed, normal), assert.AssertionError);
      } else if (!countFailed) {
        assert.deepEqual(observed, normal);
      }
      const observedTheme = await page.eval("document.documentElement.dataset.theme || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')");
      assert.equal(observedTheme, theme);
      await page.capture(`notifications-${name}-${width}x${height}-${theme}`, {
        scenario: `notifications-inbox-${name}`, boundary: 'Product Web / CDP simulated Gateway', dataMode: 'simulated',
        requestedScene: { width, height }, expectedTheme: theme, observedTheme,
        expected, observed, trace: [...trace],
      });
      if (name === 'count-error') {
        state.countMode = 'fresh';
        await page.click('#notification-count-retry');
        await page.waitFor(`JSON.stringify(${readDom}) === ${JSON.stringify(JSON.stringify(normal))}`, 'count retry keeps the unread rows');
        assert.equal(trace.filter(event => event.path === countPath).at(-1).count, 0);
      }
      await page.viewport(390, 844);
      await page.click('main .vk-notif-page button[aria-label="Quay lại"]');
      await page.waitFor(`location.pathname === '/' && ${visible(mobile)}`, 'inbox Back returns to the bell');
      if (!legacy && !listFailed) {
        await page.waitFor(`${badge}?.dataset.state === 'hidden' && ${badge}.hidden`, 'no badge after verified zero');
      }
    }
  }
}

async function inboxSessionStates(page, { args, check }, controlled) {
  const { state, calls, held } = controlled;
  const inboxPath = '/thong-bao';
  const auth = '/api/v1/auth/session';
  const notFound = 'main [data-panel="error:not-found"]';
  const explore = notFound + ' a[href="/"]';
  const loading = '!!document.querySelector("main .vk-notif-loading")';
  const privateControls = 'main .vk-notif-row, main .vk-notif-unread, #notification-read-all, #notification-load-more';
  const noPrivate = `!document.querySelector('${privateControls}')`;
  const isMissing = `!!document.querySelector('${notFound}')`;
  const retry = 'main .vk-notif-error button';
  const sessionError = `document.querySelector('main [role=alert]')?.textContent.includes('Chưa kiểm tra được phiên đăng nhập.')`;
  const historyPath = path => `location.pathname === '${path}'`;

  state.session = null;
  state.holdSession = true;
  await page.goto(args.base + inboxPath);
  await eventually(() => held.has('session'), 'initial session check is held');
  check(await page.eval(`${loading} && !(${isMissing}) && ${noPrivate}`), '#575 unknown session is pending');
  await held.get('session')();
  await page.waitFor(`${isMissing} || document.querySelector('main a[href="/toi"]')`, 'guest presentation settled');
  for (const [width, height, theme, scale] of [
    [1440, 900, 'light', 100], [1440, 900, 'dark', 150],
    [390, 844, 'light', 150], [390, 844, 'dark', 100], [320, 540, 'dark', 200],
  ]) {
    await preferences(page, width, theme, scale, height);
    await page.capture(`inbox-guest-${width}-${theme}-${scale}`, {
      scenario: 'inbox-confirmed-anonymous', requestedScene: { width, height, scale, theme },
      boundary: 'Chrome · session response fixture', skipSceneValidation: true,
    });
  }
  check(await page.eval(`${isMissing} && ${historyPath(inboxPath)} && ${noPrivate}
    && document.querySelectorAll('main h1').length === 1
    && document.querySelector('main h1').textContent === 'Không tìm thấy trang này.'
    && document.title === 'Không tìm thấy — Vink'
    && document.querySelector('meta[name=robots][content=noindex]')
    && !document.querySelector('#notification-page-title, main a[href="/toi"]')
    && !document.querySelector('main').textContent.includes('Đăng nhập để xem thông báo')`),
  '#575 confirmed Anonymous uses shared NotFound at the Inbox URL');
  check(await page.eval('document.documentElement.scrollWidth <= innerWidth + 1'), '#575 guest NotFound at 320px and 200% fits the viewport');

  await preferences(page, 390, 'light', 100, 844);
  await page.eval(`document.querySelector('${notFound} h1').focus()`);
  await page.key('Tab', 0, { settle: false });
  await page.waitFor(`document.activeElement?.matches('${explore}')`, 'Tab reaches the general recovery link');
  check(await page.eval('getComputedStyle(document.activeElement).outlineStyle !== "none"'),
    '#575 guest recovery link has a visible keyboard outline');
  await page.key('Enter', 0, { settle: false });
  await page.waitFor(historyPath('/'), 'Enter opens Khám phá');
  await page.eval('history.back()');
  await page.waitFor(`${historyPath(inboxPath)} && ${isMissing}`, 'Back restores guest Inbox at its own URL');
  await page.eval('history.forward()');
  await page.waitFor(historyPath('/'), 'Forward returns to Khám phá');

  state.session = 'A';
  state.checkFailure = true;
  await page.goto(args.base + inboxPath);
  await page.waitFor(`${sessionError} && !(${loading}) && !(${isMissing})`, 'first session 503 offers retry');
  state.checkFailure = false;
  state.pageSize = 0;
  await page.click(retry);
  await page.waitFor('document.querySelector("main").textContent.includes("Chưa có thông báo.")', 'session retry reaches signed-in empty inbox');
  check(await page.eval(noPrivate), '#602 a signed-in empty inbox has no bulk action');
  check(!calls.some(request => request.path === inbox && request.scope == null), '#575 no unauthorised inbox read during session verification');

  state.pageSize = null;
  await page.reload();
  await page.waitFor(`document.querySelector('main ${row}') && document.querySelector('#notification-read-all')`, 'Inbox loaded before the session fence');
  held.delete('session');
  state.holdSession = true;
  state.checkFailure = true;
  await controlled.change('B');
  await eventually(() => held.has('session'), 'fenced session recheck is held');
  await page.waitFor(`${loading} && ${noPrivate} && !(${isMissing})`, 'session fence removes rows, unread and actions while pending');
  await held.get('session')();
  await page.waitFor(`${sessionError} && ${noPrivate} && !(${loading}) && !(${isMissing})`, 'fenced 503 offers session retry without private DOM');
  const sessionChecks = calls.filter(request => request.path === auth).length;
  state.checkFailure = false;
  await page.click(retry);
  await page.waitFor(`document.querySelector('main ${row}')?.textContent.includes('Thông báo của tài khoản B')`, 'session retry loads B Inbox');
  check(calls.filter(request => request.path === auth).length > sessionChecks
    && await page.eval(`!document.querySelector('main').textContent.includes('${privateTitle}')`),
  '#575 retry checks session before restoring scoped rows');
  await controlled.change(null);
  await page.waitFor(`${isMissing} && ${noPrivate}`, 'logout replaces the loaded Inbox with guest NotFound');

  state.standing = 'suspended';
  await controlled.change(null);
  await page.waitFor(`document.querySelector('main a[href="/toi"]') && ${noPrivate} && !(${isMissing})`,
    'confirmed suspended account keeps its account recovery without NotFound');

  state.standing = 'anonymous';
  state.session = 'A';
  state.seen = 0;
  await preferences(page, 1440, 'light', 100);
  await page.goto(args.base + '/');
}

export async function notifications(page, { args, check }) {
  const controlled = await fixture(page);
  const { state, calls, held } = controlled;
  const latest = path => calls.findLast(request => request.path === path);
  const report = [];
  await page.viewport(1440);
  const screenshot = (name, width, scale, theme) => page.capture(name, {
    scenario: 'notifications-r4', requestedScene: { width, height: 900, scale, theme },
    boundary: 'Chrome-CDP-dom-override-geometry', skipSceneValidation: true,
  });

  if (args['notification-baseline'] === 'yes') {
    await page.goto(`${args.base}/`);
    await page.waitFor(mounted, 'notification shell mounted');
    for (const theme of ['light', 'dark']) {
      for (const width of viewport) {
        for (const scale of [100, 150, 200]) {
          await preferences(page, width, theme, scale);
          await screenshot(`notifications-before-${width}-${theme}-${scale}`, width, scale, theme);
        }
      }
    }
    return;
  }

  await inboxSessionStates(page, { args, check }, controlled);
  // Keep the original popover assertions scoped to the fresh shell.
  calls.length = 0;
  await page.waitFor(`${visible(bell)} && document.querySelector('${bell} .vk-ui-badge').textContent === '3'`, 'fresh rail badge');
  await screenshot('notifications-badge-fresh', 1440, 100, 'light');
  check(await page.eval(`document.querySelector('${bell}').previousElementSibling.dataset.destination === 'desk'
    && document.querySelector('#navigation-account').children.length === 1`), 'AC-01 bell directly below Desk, Me alone in account');
  check(calls.filter(x => x.path.startsWith(inbox)).every(x => x.path === `${inbox}/count`),
    'AC-02 the closed shell only reads count');
  const beforePause = calls.filter(({path}) => path === `${inbox}/count`).length;
  await page.eval(`Object.defineProperty(document, 'hidden', { configurable: true, value: true });
    window.dispatchEvent(new Event('visibilitychange'));`);
  await sleep(2100);
  check(calls.filter(({path}) => path === `${inbox}/count`).length === beforePause, 'AC-02 hidden tab pauses count');
  await page.eval(`delete document.hidden; window.dispatchEvent(new Event('visibilitychange'));`);
  await eventually(() => calls.filter(({path}) => path === `${inbox}/count`).length > beforePause, 'visible tab resumes count');
  const beforeOffline = calls.filter(({path}) => path === `${inbox}/count`).length;
  await page.eval(`Object.defineProperty(navigator, 'onLine', { configurable: true, value: false });
    window.dispatchEvent(new Event('offline'));`);
  await page.waitFor(`document.querySelector('${bell} .vk-ui-badge[data-state=stale]')?.textContent === '3'`, 'offline keeps last count as stale');
  await sleep(2100);
  check(calls.filter(({path}) => path === `${inbox}/count`).length === beforeOffline, 'AC-02 offline pauses count');
  await page.eval(`delete navigator.onLine; window.dispatchEvent(new Event('online'));`);
  await eventually(() => calls.filter(({path}) => path === `${inbox}/count`).length > beforeOffline, 'reconnected tab resumes count');


  const ssr = await fetch(`${args.base}/thong-bao`);
  const html = await ssr.text();
  check(ssr.headers.get('cache-control')?.includes('no-store') && !html.includes(privateTitle)
    && !html.includes('x-vink-notification-scope'), 'AC-05 inbox HTML is no-store and private-data-free');
  await page.eval(`document.querySelector('${bell}').focus()`);
  await page.key('Enter');
  await page.waitFor(`document.querySelector('${popup}').open && document.querySelectorAll('${popup} ${row}').length >= 3`, 'popover and its rows');
  await eventually(() => latest(`${inbox}/seen`), 'Seen command');
  check(latest(`${inbox}/seen`).body.up_to_snapshot === 's1.45.A', 'AC-03 Seen uses the first-page snapshot');
  check(await page.eval(`document.querySelectorAll('${popup} .vk-notif-row--unread').length >= 1`), 'AC-03 Seen keeps rows unread');
  check(await page.eval(`Math.abs(document.querySelector('${popup}').getBoundingClientRect().top -
      document.querySelector('${bell}').getBoundingClientRect().top) <= 2`), 'AC-01 popover starts at the bell');
  check(await page.eval(focused('#notification-popover-title')), 'AC-01 opening moves focus to the title');
  check(await page.eval(`[...document.querySelectorAll('${popup} .vk-ui-avatar-group .vk-ui-avatar__initials')]
    .some(initials => initials.textContent.trim() === 'LP' && getComputedStyle(initials).visibility === 'visible')`),
    'AC-05 the shared avatar group shows fallback initials');
  await page.key('Tab');
  check(await page.eval(focused(markButton)), '#602 Tab reaches the icon from the popover title');
  await page.waitFor(`(() => {
    const tip = document.querySelector('${markButton}-tooltip'), bounds = tip.getBoundingClientRect();
    return !tip.hidden && tip.textContent === '${markLabel}' &&
      document.elementFromPoint(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)?.closest('[role=tooltip]') === tip;
  })()`, '#602 the keyboard tooltip is visible over the popover');
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  check(nodes.some(node => node.role?.value === 'button' && node.name?.value === markLabel),
    '#602 the icon has the action name in the Chrome accessibility tree');
  await page.key('Escape');
  check(await page.eval(`document.querySelector('${popup}').open && document.querySelector('${markButton}-tooltip').hidden && ${focused(markButton)}`),
    '#602 Escape dismisses the tooltip without closing the popover or moving focus');
  await page.key('Escape');
  check(await page.eval(focused(bell)), 'AC-01 Escape restores rail bell');
  await page.click(bell);
  await page.waitFor(`document.querySelector('${popup}').open`, 'popover reopens for outside click');
  for (const kind of ['mousePressed', 'mouseReleased']) {
    await page.send('Input.dispatchMouseEvent', { type: kind, x: 1100, y: 100, button: 'left', clickCount: 1 });
  }
  await page.waitFor(focused(bell), 'outside click restores the bell');
  await page.key('Space');
  await page.waitFor(`document.querySelector('${popup}').open`, 'popover reopens for explicit close');
  await page.click(`${popup} button[aria-label="Đóng thông báo"]`);
  await page.waitFor(focused(bell), 'close button restores the bell');
  await page.key('Space');
  await page.waitFor(`document.querySelector('${popup}').open && document.querySelector('${popup} ${markButton}')`, 'Space opens popover');
  await page.key('Tab');
  await page.key('Space');
  await page.waitFor(`!document.querySelector('${popup} ${markButton}') && ${focused('#notification-popover-title')}`,
    '#602 Space marks the popover read and returns focus to its title');
  state.marked = false;
  await page.viewport(390);
  await page.waitFor(focused('#noi-dung'), 'resize returns focus to main');
  check(await page.eval(`!document.querySelector('${popup}').open && ${visible(mobile)}
    && document.querySelectorAll('.vk-ui-bottom-nav a').length === 3`), 'AC-01 mobile has a header bell and exactly three bottom destinations');
  await page.click('#bottom-navigation a[href="/toi"]');
  await page.waitFor(`location.pathname === '/toi' && document.querySelector('#page-account .vk-account-status ${mobile}')`,
    '#520 mobile account card contains the enabled bell');
  const mobileAccount = '#page-account .vk-account-status';
  for (const [width, scale, theme] of [[700, 100, 'light'], [390, 100, 'light'], [390, 150, 'dark'], [320, 200, 'dark']]) {
    await page.viewport(width);
    await page.eval(`document.documentElement.dataset.theme = '${theme}'; document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
    check(await page.eval(`(() => {
      const card = document.querySelector('${mobileAccount}');
      const button = card.querySelector('${mobile}');
      const title = card.querySelector('#me-title');
      const illustration = card.querySelector('.vk-account-illustration');
      const c = card.getBoundingClientRect(), b = button.getBoundingClientRect(), t = title.getBoundingClientRect();
      // R5 gives the name its own row below the bell and avatar in the narrow card.
      const fits = ${width === 700} ? t.right <= b.left : t.top >= b.bottom && Math.abs(t.width - c.width) <= 1;
      return !illustration && title.matches('h1.vk-ui-type-title') && document.querySelectorAll('main h1').length === 1 &&
        title.textContent.trim().length > 0 && title.textContent !== 'Tài khoản' && b.width >= 48 && b.height >= 48 &&
        b.top >= c.top && b.bottom <= c.bottom && fits &&
        document.documentElement.scrollWidth <= innerWidth;
    })()`), `#520/#591 mobile ${width}px/${scale}% ${theme}: name and bell fit the expected card rows`);
    await screenshot(`account-mobile-bell-${width}-${scale}-${theme}`, width, scale, theme);
  }
  await page.eval(`document.documentElement.style.fontSize = ''; document.querySelector('${mobile}').focus()`);
  await page.key('Space');
  await page.waitFor(`location.pathname === '/thong-bao' && document.querySelectorAll('main ${row}').length >= 3`, 'mobile account bell opens the inbox');
  await page.eval(`[...document.querySelectorAll('main button')].find(el => el.textContent.includes('Xem thêm')).click()`);
  await page.waitFor(`document.querySelector('main .vk-notif-more [role=alert]')`, 'pagination error below list');
  check(await page.eval(`document.querySelectorAll('main ${row}').length === 3`), 'AC-03 failed continuation keeps the previous page');
  state.moreFailure = false;
  await page.eval(`document.querySelector('main .vk-notif-more [role=alert] button').focus()`);
  await page.key('Enter');
  await page.waitFor(`document.querySelectorAll('main ${row}').length > 3`, 'same continuation succeeds');
  await page.waitFor(focused('main .vk-notif-row__link'), 'focus moves to the newly loaded page after the continuation ends');
  await page.eval("document.querySelector('main button[aria-label=\"Quay lại\"]').focus()");
  await page.key('Tab');
  check(await page.eval(focused(markButton)), '#602 Tab reaches the page icon after Back');
  state.markFailure = true;
  state.holdMark = true;
  held.delete('mark');
  const previousMarks = calls.filter(request => request.path === markAll).length;
  await page.key('Enter');
  await eventually(() => held.get('mark'), 'held MarkAllRead');
  check(await page.eval(`document.querySelector('${markButton}[aria-busy=true][aria-disabled=true]') &&
    document.querySelector('main .vk-notif-row--unread') && ${focused(markButton)}`),
  '#602 the focused icon is busy and the rows stay unread before ACK');
  await page.key('Space');
  await page.eval(`document.querySelector('${markButton}').click()`);
  await held.get('mark')();
  const markRetry = 'main .vk-notif-error button';
  await page.waitFor(`document.querySelector('${markRetry}') && document.querySelector('main [role=alert]').textContent.includes('Chưa đánh dấu')`,
    '#602 a failed mark-all exposes the retry');
  check(calls.filter(request => request.path === markAll).length === previousMarks + 1
    && await page.eval(`document.querySelector('main .vk-notif-row--unread') && !document.querySelector('${markButton}').hasAttribute('aria-busy')`),
    '#602 failure preserves unread rows and pending activation dispatches only once');

  state.markFailure = false;
  state.holdFirst = true;
  await page.eval(`document.querySelector('${markRetry}').focus()`);
  await page.key('Enter');
  await eventually(() => held.get('first'), 'held inbox reread after MarkAllRead ACK');
  check(await page.eval(`document.querySelector('main .vk-notif-row--unread') && ${focused(markButton)}`),
    '#602 the receipt alone leaves rows unread until the authoritative reread');
  await held.get('first')();
  await page.waitFor(`!document.querySelector('main .vk-notif-row--unread, main ${markButton}') && ${focused('#notification-page-title')}`,
    '#602 the reread removes the icon and returns focus to the page title');
  check(calls.filter(request => request.path === markAll).length === previousMarks + 2
    && latest(markAll).body.up_to_snapshot === 's1.45.A', '#602 retry sends the same snapshot exactly once');

  await page.goto(`${args.base}/`);
  await page.waitFor(`${mounted} && ${visible(mobile)}`, 'mobile home');
  state.marked = false;
  for (const theme of ['light', 'dark']) {
    for (const width of viewport) {
      for (const scale of [100, 150, 200]) {
        await preferences(page, width, theme, scale);
        const trigger = width < 760 ? mobile : bell;
        const modal = width >= 760 && scale === 100;
        await page.waitFor(`document.querySelector('${trigger}')?.tagName === '${modal ? 'BUTTON' : 'A'}'`, 'viewport-specific trigger');
        await screenshot(`notifications-after-${width}-${theme}-${scale}`, width, scale, theme);
        await page.click(trigger);
        const surface = modal ? popup : 'main .vk-notif-page';
        await page.waitFor(`document.querySelectorAll('${surface} ${row}').length >= 3`, 'matrix rows');
        const measure = await geometry(page, surface);
        const name = `notifications-${modal ? 'popover' : 'inbox'}-${width}-${theme}-${scale}`;
        report.push({ name, ...measure });
        check(measure.overflow <= 1 && measure.innerOverflow <= 1 && measure.targets.every(([w, h]) => w >= 44 && h >= 44),
          `AC-05 ${name}: no horizontal overflow, targets ≥44px`);
        const header = await page.eval(`(() => {
          const title = document.querySelector('${surface} .vk-notif-title');
          const button = document.querySelector('${surface} ${markButton}');
          const glyph = button.querySelector('[data-icon=done-all]');
          const a = title.getBoundingClientRect(), b = button.getBoundingClientRect();
          const c = glyph?.getBoundingClientRect(), style = getComputedStyle(button);
          const close = button.closest('header')?.querySelector('[aria-label="Đóng thông báo"]');
          const text = title.firstChild;
          const wholeWords = [...text.textContent.matchAll(/\\S+/g)].every(word => {
            const range = document.createRange();
            range.setStart(text, word.index);
            range.setEnd(text, word.index + word[0].length);
            return range.getClientRects().length === 1;
          });
          return { fit: a.right <= b.left && Math.abs(a.y + a.height / 2 - b.y - b.height / 2) <= 1 &&
              (!close || b.right <= close.getBoundingClientRect().left), wholeWords, target: [b.width, b.height],
            icon: c && [c.width, c.height], standard: style.backgroundColor === 'rgba(0, 0, 0, 0)' &&
              style.borderTopColor === 'rgba(0, 0, 0, 0)' && !button.textContent.trim() };
        })()`);
        report.at(-1).header = header;
        check(header.fit && header.wholeWords && header.standard && header.icon?.every(size => size === 24) && header.target.every(size => size >= 48),
          `#602 ${name}: whole title words, aligned 24px icon and transparent 48px target`);
        await screenshot(name, width, scale, theme);
        if (modal) {
          check(await page.eval(`getComputedStyle(document.querySelector('${popup}')).animationName === 'none'`),
            'AC-05 reduced motion opens without animation');
          await page.key('Escape');
        } else {
          await page.eval('history.back()');
          await page.waitFor(`location.pathname === '/' && ${focused(trigger)}`, 'Back returns to the bell');
        }
      }
    }
  }
  mkdirSync(dirname(args.state), { recursive: true });
  writeFileSync(join(dirname(args.state), 'notifications-geometry.json'), JSON.stringify(report, null, 2));

  await preferences(page, 1440, 'light', 100);
  await page.waitFor(`document.querySelector('${bell}')?.tagName === 'BUTTON'`, 'desktop normal text');
  await page.click(bell);
  await page.waitFor(`document.querySelectorAll('${popup} ${row}').length >= 3`, 'reopen inbox');
  const seenBefore = calls.filter(({path}) => path === `${inbox}/seen`).length;
  const fetchesBefore = calls.filter(({path}) => path === inbox).length;
  const oldRow = await page.eval(`window.__shownNotification = document.querySelector('${popup} ${row}'); !!window.__shownNotification`);
  state.snapshot++;
  state.count = 120;
  await page.eval(`window.dispatchEvent(new Event('online'))`);
  await page.waitFor(`document.querySelector('${bell} .vk-ui-badge').textContent === '99+'`, 'new count on the existing poll');
  check(oldRow && await page.eval(`!document.querySelector('#notification-updates, #notification-count-retry')
    && !document.querySelector('${popup}').textContent.includes('Tác phẩm có cập nhật')
    && document.querySelector('${popup} ${row}') === window.__shownNotification
    && document.querySelectorAll('${popup} ${row}').length === 3`), 'fresh count leaves the open rows mounted without a reload control');
  check(calls.filter(({path}) => path === inbox).length === fetchesBefore
    && calls.filter(({path}) => path === `${inbox}/seen`).length === seenBefore,
    'fresh count does not reset paging or advance Seen');
  await page.key('Escape');
  await page.click(bell);
  await eventually(() => latest(`${inbox}/seen`).body.up_to_snapshot === 's1.46.A', 'reopen sees the new snapshot');
  await page.waitFor(`document.querySelectorAll('${popup} ${row}').length === 3
    && document.querySelector('${popup}').textContent.includes('Tác phẩm có cập nhật')`, 'new snapshot and rows load on reopen');

  const clicked = controlled.rows.find(item => item.target.kind === 'work_comment_thread');
  const selector = `${popup} .vk-notif-row__link[href*="nhan-xet=${clicked.target.comment_id}"]`;
  await page.eval(`(() => {
    const fetch = window.fetch;
    window.fetch = function (resource, ...options) {
      if (new URL(resource.url ?? resource, location.href).pathname === '${read}') {
        window.fetch = fetch;
        window.readStartedAt = location.pathname;
      }
      return fetch.call(this, resource, ...options);
    };
  })()`);
  await page.click(selector);
  await eventually(() => held.get('read'), 'Read command');
  check(latest(read).body.group_id === clicked.group_id && latest(read).body.observed_group_version === clicked.group_version,
    'AC-03 Read submits the displayed group/version');
  await page.waitFor(`location.pathname === '/tac-pham/${clicked.target.work_id}' && !document.body.textContent.includes('${privateTitle}')`, 'typed Reader destination before Read receipt');
  check(await page.eval("window.readStartedAt === '/'"), 'AC-03 Read starts before navigation even when the destination is unavailable');
  await held.get('read')();

  await page.goto(`${args.base}/`);
  await page.waitFor(visible(bell), 'rail again');
  state.countMode = 'stale';
  await page.eval(`window.dispatchEvent(new Event('online'))`);
  await page.waitFor(`document.querySelector('${bell} .vk-ui-badge[data-state=stale]')?.textContent === '8'`, 'stale badge');
  await screenshot('notifications-badge-stale', 1440, 100, 'light');
  state.staleCount = 0;
  await page.eval(`window.dispatchEvent(new Event('online'))`);
  await page.waitFor(`document.querySelector('${bell} .vk-ui-badge[data-state=stale]')?.hidden`, 'stale zero has no badge');
  state.countMode = 'unavailable';
  await page.eval(`window.dispatchEvent(new Event('online'))`);
  await page.waitFor(`document.querySelector('${bell} .vk-ui-badge[data-state=unknown]')?.hidden`, '503 hides the badge');
  await screenshot('notifications-badge-unavailable', 1440, 100, 'light');
  state.firstFailure = true;
  for (const [width, scale, theme] of [[1440, 100, 'light'], [1440, 100, 'dark'], [390, 100, 'light'], [320, 200, 'dark']]) {
    await preferences(page, width, theme, scale);
    const desktop = width >= 760;
    const surface = desktop ? popup : 'main .vk-notif-page';
    await page.click(desktop ? bell : mobile);
    const failure = `${surface} .vk-notif-error`;
    await page.waitFor(`document.querySelector('${failure}')?.textContent.includes('Chưa tải được thông báo.')`, 'first page failed');
    check(await page.eval(`!document.querySelector('${surface} .vk-notif-empty') &&
      getComputedStyle(document.querySelector('${failure}')).backgroundColor === 'rgba(0, 0, 0, 0)'`), 'inline failure on the shared surface');
    const measure = await geometry(page, surface);
    check(measure.overflow <= 1 && measure.innerOverflow <= 1 && measure.targets.every(([w, h]) => w >= 44 && h >= 44),
      `error controls fit at ${width}px/${scale}% ${theme}`);
    await screenshot(`notifications-first-error-${width}-${scale}-${theme}`, width, scale, theme);
    state.firstFailure = false;
    await page.eval(`document.querySelector('${failure} button').focus()`);
    await page.key('Enter');
    await page.waitFor(`document.querySelector('${surface} ${row}') && document.querySelector('${surface} .vk-notif-note')?.textContent.includes('Chưa tải được số')`, 'retry restores rows while the count is unavailable');
    state.firstFailure = true;
    if (desktop) await page.key('Escape');
    else {
      await page.eval('history.back()');
      await page.waitFor(`location.pathname === '/'`, 'leave inbox');
    }
  }
  state.firstFailure = false;
  await preferences(page, 1440, 'light', 100);
  state.countMode = 'fresh';
  state.holdCount = true;
  await page.eval(`window.dispatchEvent(new Event('online'))`);
  await eventually(() => held.get('count'), 'old A count in flight');
  await controlled.change('B');
  await eventually(() => latest(`${inbox}/count`)?.scope === 'B', 'B count');
  await page.click(bell);
  await page.waitFor(`document.querySelector('${popup}').textContent.includes('Thông báo của tài khoản B')`, 'B inbox');
  state.seen = state.snapshot;
  await controlled.change('A');
  await page.waitFor(`!document.querySelector('${popup}').open && !document.body.textContent.includes('Thông báo của tài khoản B')`, 'B inbox cleared synchronously');
  await eventually(() => latest(`${inbox}/count`)?.scope === 'A', 'new A count');
  await held.get('count')().catch(() => {}); // An old A result may already have been cancelled.
  await sleep(150);
  check(await page.eval(`!document.body.textContent.includes('Thông báo của tài khoản B')
    && document.querySelector('${bell} .vk-ui-badge')?.hidden`), 'AC-02 A→B→A fences old private DOM and replies');

  const selectedWork = controlled.rows[0].target;
  await page.goto(`${args.base}/ban-viet?review=${selectedWork.work_id}&revision=${selectedWork.revision_id}&decision=3`);
  await page.waitFor(`document.body.textContent.includes('Lý do riêng của tác phẩm')`, 'exact work decision');
  check(await page.eval(`document.querySelector('h1').textContent.includes('${privateTitle}')
      && document.body.textContent.includes('Cần xử lý')
      && document.body.textContent.includes('Gửi phản đối')
      && document.querySelector('main a[href="/ban-viet/${work}"]').textContent.includes('Sửa trong Bàn viết')`),
    'AC-04 work decision uses the exact revision and eligible appeal action');
  await screenshot('notifications-work-decision', 1440, 100, 'light');

  const comment = examples.decisions.find(decision => decision.kind === 'comment_moderation_decision');
  await page.goto(`${args.base}/thong-bao/nhan-xet/${comment.comment_id}/${comment.moderation_version}`);
  await page.waitFor(`document.body.textContent.includes('Lý do riêng của nhận xét')`, 'private comment decision');
  check(await page.eval(`!document.body.textContent.includes('Không được xuất hiện')`), 'AC-04 removed comment body never renders');
  check(await page.eval(`parseFloat(getComputedStyle(document.querySelector('.vk-notif-decision__panel')).paddingInlineStart) >= 20`),
    'AC-05 decision panel preserves R4 inset');
  await screenshot('notifications-comment-decision', 1440, 100, 'light');
  state.checkFailure = true;
  await controlled.change('A');
  await page.waitFor(`document.querySelector('main [role=alert]')?.textContent.includes('Chưa kiểm tra được phiên đăng nhập.')
    && !document.querySelector('main .vk-notif-loading') && !document.querySelector('.vk-notif-decision__panel')`,
    '#575 decision removes private details and offers session retry after fenced 503');
  state.checkFailure = false;
  await page.click('main .vk-notif-error button');
  await page.waitFor(`document.body.textContent.includes('Lý do riêng của nhận xét')`, 'session retry restores decision under the confirmed scope');
  await controlled.change(null);
  await page.waitFor(`!document.querySelector('${bell}') && !document.body.textContent.includes('Lý do riêng của nhận xét')
    && document.querySelector('main h1')?.textContent === 'Nội dung không còn khả dụng'`, 'logout keeps decision masking and discards decision and bell');
  check(await page.eval(`!document.querySelector('${mobile}')`), 'AC-01 guest has neither bell');
  await controlled.change('A');
  await captureInboxCases(page, args, controlled);
}
