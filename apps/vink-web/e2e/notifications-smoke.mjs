// Runs the real routes and hydrated state owner. Only the API responses under test are
// controlled by CDP so an old count/receipt can be delivered in a reproducible order.
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const json = path => JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'));
const examples = json('../../../packages/ui-system/leptos/preview/fixtures/notifications-examples.json');
const gateway = json('../../../services/vink-app/contracts/http/v1/fixtures/notifications-gateway.json');
const bell = '#notification-bell';
const mobile = '#notification-bell-mobile';
const popup = '#notification-popover';
const inbox = '/api/v1/me/notifications';
const read = `${inbox}/read`;
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
  const state = { session: 'A', countMode: 'fresh', count: 3, snapshot: 45, seen: 0,
    marked: false, moreFailure: true, holdCount: false, holdMark: false };
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
      response = [200, scope ? { state: 'active', principal_id: sessionId[scope] } : { state: 'anonymous' }];
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
      response = state.countMode === 'unavailable' ? [503, unavailable]
        : state.countMode === 'stale' ? [200, { freshness: { status: 'stale', last_known_count: 8, count_is_lower_bound: false } }]
          : [200, { freshness: { status: 'fresh', unseen_group_count: state.seen >= state.snapshot ? 0 : state.count,
            count_is_lower_bound: false, as_of_snapshot: snapshot(scope) } }];
      if (state.holdCount) { state.holdCount = false; held.set('count', () => reply(requestId, ...response)); return; }
    } else if (path === inbox) {
      const more = searchParams.has('cursor');
      response = more && state.moreFailure ? [503, unavailable] : [200, {
        inbox_snapshot: snapshot(scope), next_cursor: more ? null : 'next-fixture',
        items: (more ? source.slice(3) : source.slice(0, 3)).map(row => ({
          ...row, is_read: state.marked ? true : row.is_read,
          target: scope === 'B' ? { ...row.target, work_title: 'Thông báo của tài khoản B' } : row.target,
        })),
      }];
    } else if (path === `${inbox}/seen`) {
      state.seen = Math.max(state.seen, Number(body.up_to_snapshot.split('.')[1]));
      response = [200, { seen_snapshot: snapshot(scope, state.seen) }];
    } else if (path === read) {
      response = [200, { group_id: body.group_id, group_version: body.observed_group_version, is_read: true }];
      held.set('read', () => reply(requestId, ...response));
      return;
    } else if (path === `${inbox}/mark-all-read`) {
      const answer = () => {
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
    reply(requestId, ...response).catch(() => {});
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [
    { urlPattern: '*/api/v1/auth/session*' }, { urlPattern: `*${inbox}*` }, { urlPattern: `*${apiReview}` },
  ] });
  return { state, calls, rows: source, held, async change(next) {
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
async function preferences(page, width, theme, scale) {
  await page.viewport(width, 900);
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

const hydrated = `document.documentElement.dataset.hydrated === 'true'`;
const row = '.vk-notif-row';
const visible = selector => `document.querySelector(${JSON.stringify(selector)})?.getBoundingClientRect().width > 0`;
const focused = selector => `document.activeElement?.matches(${JSON.stringify(selector)})`;

export async function notifications(page, { args, check }) {
  const controlled = await fixture(page);
  const { state, calls, held } = controlled;
  const latest = path => calls.findLast(request => request.path === path);
  const report = [];
  await page.viewport(1440);
  await page.goto(`${args.base}/`);
  await page.waitFor(hydrated, 'real Web hydration');
  const screenshot = (name, width, scale, theme) => page.capture(name, {
    scenario: 'notifications-r4', requestedScene: { width, height: 900, scale, theme },
    boundary: 'Chrome-CDP-dom-override-geometry', skipSceneValidation: true,
  });

  if (args['notification-baseline'] === 'yes') {
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
  check(await page.eval(`document.querySelector('${popup}').contains(document.activeElement)`), 'AC-05 Tab stays in the dialog');
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
  await page.waitFor(`document.querySelector('${popup}').open`, 'Space opens popover');
  await page.viewport(390);
  await page.waitFor(focused('#noi-dung'), 'resize returns focus to main');
  check(await page.eval(`!document.querySelector('${popup}').open && ${visible(mobile)}
    && document.querySelectorAll('.vk-ui-bottom-nav a').length === 3`), 'AC-01 mobile has a header bell and exactly three bottom destinations');
  await page.click('#bottom-navigation a[href="/toi"]');
  await page.waitFor(`location.pathname === '/toi' && document.querySelector('#page-account .vk-account-status ${mobile}')`,
    '#520 mobile account card contains the enabled bell');
  const mobileAccount = '#page-account .vk-account-status';
  for (const [width, scale, theme] of [[390, 100, 'light'], [390, 150, 'dark'], [320, 200, 'dark']]) {
    await page.viewport(width);
    await page.eval(`document.documentElement.dataset.theme = '${theme}'; document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
    check(await page.eval(`(() => {
      const card = document.querySelector('${mobileAccount}');
      const button = card.querySelector('${mobile}');
      const title = card.querySelector('#me-title');
      const c = card.getBoundingClientRect(), b = button.getBoundingClientRect(), t = title.getBoundingClientRect();
      return title.matches('h1.vk-ui-type-title') && document.querySelectorAll('main h1').length === 1 &&
        title.textContent === 'Tài khoản' && b.width >= 48 && b.height >= 48 &&
        b.top >= c.top && b.bottom <= c.bottom && t.right <= b.left &&
        document.documentElement.scrollWidth <= innerWidth;
    })()`), `#520 mobile ${width}px/${scale}% ${theme}: heading and bell fit the card side by side`);
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
  state.holdMark = true;
  await page.click('main #notification-read-all');
  await eventually(() => held.get('mark'), 'held MarkAllRead');
  check(await page.eval(`document.querySelectorAll('main .vk-notif-row--unread').length > 0`), 'AC-03 bulk read waits for confirmation');
  await held.get('mark')();
  await page.waitFor(`!document.querySelector('main .vk-notif-row--unread')`, 'bulk read applied from authoritative reread');
  check(latest(`${inbox}/mark-all-read`).body.up_to_snapshot === 's1.45.A', 'AC-03 bulk read preserves the first-page snapshot');

  await page.goto(`${args.base}/`);
  await page.waitFor(`${hydrated} && ${visible(mobile)}`, 'mobile home');
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
  state.snapshot++;
  state.count = 120;
  await page.eval(`window.dispatchEvent(new Event('online'))`);
  await page.waitFor(`document.querySelector('#notification-updates') && document.querySelector('${bell} .vk-ui-badge').textContent === '99+'`, 'new count and update banner');
  check(await page.eval(`document.querySelectorAll('${popup} ${row}').length === 3`), 'AC-03 incoming count does not insert inbox rows');

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
  state.countMode = 'unavailable';
  await page.eval(`window.dispatchEvent(new Event('online'))`);
  await page.waitFor(`document.querySelector('${bell} .vk-ui-badge[data-state=unknown]')?.textContent === ''`, '503 ring without fake zero');
  await screenshot('notifications-badge-unavailable', 1440, 100, 'light');
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
  await controlled.change(null);
  await page.waitFor(`!document.querySelector('${bell}') && !document.body.textContent.includes('Lý do riêng của nhận xét')`, 'logout discards decision and bell');
  check(await page.eval(`!document.querySelector('${mobile}')`), 'AC-01 guest has neither bell');
}
