// LVE #73 browser checks: the reader's “Đánh giá cao” and each comment's “Hữu ích” against the
// real App and scratch PostgreSQL, acting through the loopback LocalFixture. This is NOT
// production auth (production writes answer 401 until D-05). Interception only holds or drops
// real responses (a lost ACK after the commit) and answers one own-state read with 401 to show
// the signed-out reason; no success is fabricated.
//
// Phase `main` (fixture = the work's author): SSR + hydration and the own-content reason.
// Phase `endorsement`: two servers share the scratch database — `--author-base` acts as the
// author (writes a root, then a flat reply once the root is public) and `--base` as a second
// principal holding a moderator grant, who approves both and then endorses in the browser.

const SAMPLE_WORK = '33333333-3333-4333-8333-333333333333';
const SAMPLE_REVISION = '44444444-4444-4444-8444-444444444444';
const LAST_LINE = 'đã theo mưa về rất lâu.';
const ROOT_BODY = 'LVE [Mẫu] Khổ cuối đọc chậm mới thấy mưa — nhận xét gốc để đánh dấu hữu ích.';
const REPLY_BODY = 'LVE [Mẫu] Phản hồi phẳng: mình cũng dừng lại ở câu thụt vào.';
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function api(args, path, init = {}) {
  const response = await fetch(`${args.base}${path}`, {
    ...init,
    headers: { 'Content-Type': 'application/json', Origin: args.base, ...(init.headers ?? {}) },
  });
  const text = await response.text();
  return { status: response.status, cache: response.headers.get('cache-control'), body: text ? JSON.parse(text) : null };
}

/** The visible count text of a server-rendered action, tags and hydration markers removed. */
function ssrCount(html, id) {
  const at = html.indexOf(`id="${id}-count"`);
  if (at < 0) return null;
  const tag = html.slice(html.lastIndexOf('<', at), html.indexOf('>', at) + 1);
  const inner = html.slice(html.indexOf('>', at) + 1, html.indexOf('lượt', at));
  return { known: tag.includes('data-count="known"'), text: inner.replace(/<[^>]*>/g, '').trim() };
}

const button = (id) => `document.querySelector(${JSON.stringify(`#${id}`)})`;
const note = (id) => `(${button(id)}?.getAttribute('aria-describedby') ?? '').split(' ').map(id=>document.getElementById(id)?.textContent ?? '').join(' ')`;
const count = (id) => `(document.querySelector(${JSON.stringify(`#${id}-count`)})?.textContent ?? '')`;

/** Main phase, fixture = the author of the sample work. */
export async function endorsementAsAuthor(page, { args, check }) {
  console.log('LVE #73 (author fixture): SSR + hydration of the work action, own-content reason:');
  const reader = `${args.base}/tac-pham/${SAMPLE_WORK}`;
  const html = await (await fetch(reader)).text();
  const at = html.indexOf('id="endorse-work"');
  check(at > html.indexOf(LAST_LINE) && at < html.indexOf('id="feedback-region"'), 'SSR renders “Đánh giá cao” after the published text and before the discussion');
  check(html.includes('data-icon="pen-flight"'), 'SSR contains the selected Pen Flight registry glyph');
  const workCount = ssrCount(html, 'endorse-work');
  check(workCount?.known && workCount.text === '0' && !html.includes('aria-pressed'), `SSR carries the public count of the current revision (${workCount?.text}), no aria-pressed`);
  check(html.includes('data-pressed="unknown"') && !html.includes('Cần đăng nhập để đánh giá'), 'SSR never claims an own state before the browser reads it, and a fixture session is not a guest');
  await page.goto(reader);
  await page.waitFor(`document.documentElement.dataset.hydrated==='true' && ${note('endorse-work')}.includes('chính mình')`, 'own-state read after hydration');
  check(await page.eval(`(() => { const b = ${button('endorse-work')}; return b.getAttribute('aria-disabled')==='true' && b.dataset.state==='disabled' && b.getAttribute('aria-label')==='Đánh giá cao' && b.getAttribute('aria-describedby').includes('endorse-work-note'); })()`), 'the author reads a text reason (CHOICE-08 A); the action stays focusable and inert');
  await page.click('#endorse-work');
  await sleep(300);
  check(await page.eval(`performance.getEntriesByType('resource').every(e => !e.name.includes('/endorsement/me'))`), 'pressing the blocked action sends no command');
  const self = await api(args, `/api/v1/works/${SAMPLE_WORK}/endorsement/me`, { method: 'PUT', body: JSON.stringify({ command_key: crypto.randomUUID(), expected_order: 1, endorsed: true, revision_id: SAMPLE_REVISION }) });
  check(self.status === 403 && self.body.error.code === 'self_endorsement' && self.cache === 'no-store', 'the server refuses self endorsement itself (403 self_endorsement, no-store)');
}

/** Holds or drops real PUT …/endorsement/me responses (Response stage) for this page. */
function writeGate(page, args) {
  const held = [];
  const listener = (message) => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    if (message.params.request.method === 'PUT') held.push(message.params);
    else page.send('Fetch.continueRequest', { requestId: message.params.requestId }).catch(() => {});
  };
  return {
    held,
    async enable() {
      page.cdp.listeners.push(listener);
      await page.send('Fetch.enable', { patterns: [{ urlPattern: `${args.base}/api/v1/works/*/endorsement/me`, requestStage: 'Response' }] });
    },
    async next(count) {
      const deadline = Date.now() + 10_000;
      while (held.length < count) {
        if (Date.now() > deadline) throw new Error(`timed out waiting for endorsement PUT ${count}`);
        await sleep(50);
      }
      return held[count - 1];
    },
    async body(params) {
      const { body, base64Encoded } = await page.send('Fetch.getResponseBody', { requestId: params.requestId });
      return JSON.parse(base64Encoded ? Buffer.from(body, 'base64').toString('utf8') : body);
    },
    release(params) { return page.send('Fetch.continueRequest', { requestId: params.requestId }); },
    drop(params) { return page.send('Fetch.failRequest', { requestId: params.requestId, errorReason: 'ConnectionClosed' }); },
    async disable() {
      await page.send('Fetch.disable');
      page.cdp.listeners = page.cdp.listeners.filter((item) => item !== listener);
    },
  };
}

/** Endorsement phase, fixture = a second principal with a moderator grant. */
export async function endorsementAsReader(page, { args, check, captureMatrix }) {
  console.log('LVE #73 (reader fixture): the author comments, the moderator fixture approves:');
  const author = { base: args['author-base'] };
  if (!author.base) throw new Error('missing --author-base');
  const approve = async (id, what) => {
    const queue = await api(args, '/api/v1/moderation/comments');
    const item = queue.body.items.find((entry) => entry.comment_id === id);
    const decided = await api(args, `/api/v1/moderation/comments/${id}/decisions`, { method: 'POST', body: JSON.stringify({ expected_version: item.version, action: 'approve' }) });
    check(decided.status === 200 && decided.body.review === 'approved', `moderator fixture approves ${what}`);
  };
  const submit = async (target, body, what) => {
    const sent = await api(author, `/api/v1/works/${SAMPLE_WORK}/discussion/comments`, { method: 'POST', body: JSON.stringify({ target_id: target, body, command_key: crypto.randomUUID() }) });
    check(sent.status === 201 && sent.body.status === 'pending', `the author submits ${what} for review`);
    return sent.body.comment_id;
  };
  const root = await submit(null, ROOT_BODY, 'a root comment');
  await approve(root, 'the root');
  const reply = await submit(root, REPLY_BODY, 'a flat reply to it');
  await approve(reply, 'the reply');
  const reader = `${args.base}/tac-pham/${SAMPLE_WORK}`;
  const html = await (await fetch(reader)).text();
  check([root, reply].every((id) => ssrCount(html, `endorse-comment-${id}`)?.text === '0'), 'SSR renders “Hữu ích” with a public count on the root and on the flat reply');
  await page.goto(reader);
  const ready = `document.documentElement.dataset.hydrated==='true' && ${button('endorse-work')}?.dataset.state==='enabled' && ${button(`endorse-comment-${reply}`)}?.dataset.state==='enabled'`;
  await page.waitFor(ready, 'own states read after hydration');
  check(await page.eval(`${button('endorse-work')}.dataset.pressed==='false' && ${count('endorse-work')}.startsWith('0')`), 'a reader who has not endorsed sees Off and the public count');
  const glyphs = async (phase) => check(await page.eval(`(() => {
    const controls = [...document.querySelectorAll('.vk-endorsement .vk-ui-toggle__control')];
    return controls.length >= 3 && controls.every(b => {
      const glyph = b.querySelector('.vk-ui-toggle__glyph svg');
      return glyph?.dataset.icon === 'pen-flight' && glyph.getAttribute('aria-hidden') === 'true'
        && glyph.getBoundingClientRect().width === 20 && b.getBoundingClientRect().height >= 48;
    });
  })()`), `Pen Flight: work, root and flat reply keep decorative 20px glyph / 48px targets (${phase})`);
  await glyphs('ready');
  const order = `[...document.querySelectorAll('.vk-discussion-comment')].map(c => c.id).join()`;
  const before = await page.eval(order);

  const gate = writeGate(page, args);
  await gate.enable();
  try {
    console.log('  double tap, pending, confirmation only after the ACK:');
    await page.click('#endorse-work');
    await page.waitFor(`${button('endorse-work')}.dataset.state==='pending' && ${button('endorse-work')}.getAttribute('aria-busy')==='true'`, 'pending state');
    await page.click('#endorse-work');
    const first = await gate.next(1);
    await sleep(200);
    check(gate.held.length === 1, 'a double tap sends one command');
    check(await page.eval(`${button('endorse-work')}.dataset.pressed==='false' && ${note('endorse-work')}.includes('Đang gửi')`), 'CHOICE-10 A: no optimistic selection while the command is pending');
    await glyphs('pending');
    const firstBody = JSON.parse(first.request.postData);
    check(first.responseStatusCode === 200 && firstBody.expected_order === 1 && firstBody.endorsed === true && firstBody.revision_id === SAMPLE_REVISION && /^[0-9a-f]{8}-[0-9a-f]{4}-4/.test(firstBody.command_key), 'the command names the shown revision, the shown order and a UUID key');
    await gate.release(first);
    await page.waitFor(`${button('endorse-work')}.dataset.pressed==='true' && ${count('endorse-work')}.startsWith('1')`, 'confirmed selection');
    check(await page.eval(`${button('endorse-work')}.getAttribute('aria-label')==='Bỏ đánh giá cao' && !!document.querySelector('#endorse-work .vk-ui-toggle__mark svg, #endorse-work .vk-ui-toggle__mark .vk-ui-icon')`), 'after the ACK: accessible name “Bỏ đánh giá cao” and the check mark');

    await glyphs('confirmed');
    check(await page.eval(`document.querySelector('#endorse-work .vk-ui-toggle__mark svg')?.dataset.icon === 'check'`), 'Pen Flight remains leading; confirmed check uses the trailing slot');
    console.log('  lost ACK after the commit, exact keyed replay:');
    await page.click('#endorse-work');
    const lost = await gate.next(2);
    check(lost.responseStatusCode === 200, 'the server committed the removal before its answer is lost');
    await gate.drop(lost);
    await page.waitFor(`${note('endorse-work')}.includes('Chưa xác nhận') && [...document.querySelectorAll('#work-endorsement button')].some(b => b.textContent==='Kiểm tra lại')`, 'uncertain state');
    check(await page.eval(`${button('endorse-work')}.dataset.pressed==='true' && ${button('endorse-work')}.getAttribute('aria-disabled')==='true'`), 'a lost answer proves nothing: the confirmed On stays and new presses wait');
    await page.click('#endorse-work');
    await sleep(200);
    check(gate.held.length === 2, 'no blind command while uncertain');
    await page.eval(`[...document.querySelectorAll('#work-endorsement button')].find(b => b.textContent==='Kiểm tra lại').click()`);
    const replay = await gate.next(3);
    check(replay.request.postData === lost.request.postData, 'reconcile resends exactly the same key, order, desired state and revision');
    const replayed = await gate.body(replay);
    check(replayed.outcome === 'replayed' && replayed.endorsement.selected === false, 'the server answers `replayed` with the current state');
    await gate.release(replay);
    await page.waitFor(`${button('endorse-work')}.dataset.pressed==='false' && ${count('endorse-work')}.startsWith('0') && ${button('endorse-work')}.dataset.state==='enabled'`, 'reconciled removal');

    console.log('  another tab moves the order: 409 shows the observed state, never resends:');
    const own = await api(args, `/api/v1/works/${SAMPLE_WORK}/endorsements/me`);
    check(own.cache === 'no-store' && own.body.work.state === 'not_selected', 'own state read is no-store');
    let order3 = own.body.work.order;
    for (const endorsed of [true, false]) {
      const other = await api(args, `/api/v1/works/${SAMPLE_WORK}/endorsement/me`, { method: 'PUT', body: JSON.stringify({ command_key: crypto.randomUUID(), expected_order: order3, endorsed, revision_id: SAMPLE_REVISION }) });
      check(other.status === 200 && other.body.outcome === 'accepted', `another tab: ${endorsed ? 'select' : 'remove'} accepted`);
      order3 = other.body.endorsement.order;
    }
    await page.click('#endorse-work');
    const stale = await gate.next(4);
    check(stale.responseStatusCode === 409 && JSON.parse(stale.request.postData).expected_order === order3 - 2, 'the page sends the order it showed and the server answers 409');
    await gate.release(stale);
    await page.waitFor(`${button('endorse-work')}.dataset.pressed==='false' && ${note('endorse-work')}.includes('Không thể đánh giá') && ${button('endorse-work')}.dataset.state==='enabled'`, 'conflict reconciled to the observed state');
    await sleep(300);
    check(gate.held.length === 4, 'the old intent is not resent with the new order');
    await page.click('#endorse-work');
    const fresh = await gate.next(5);
    check(JSON.parse(fresh.request.postData).expected_order === order3 && fresh.responseStatusCode === 200, 'a new press is a new intent at the observed order');
    await gate.release(fresh);
    await page.waitFor(`${button('endorse-work')}.dataset.pressed==='true'`, 'new intent confirmed');

    console.log('  “Hữu ích” on a flat reply by keyboard; the root and the work are untouched:');
    await page.eval(`${button(`endorse-comment-${reply}`)}.focus()`);
    await page.key('Enter');
    const useful = await gate.next(6);
    check(new URL(useful.request.url).pathname.endsWith(`/discussion/comments/${reply}/endorsement/me`) && JSON.parse(useful.request.postData).endorsed === true, 'Enter on the focused reply action sends its own command');
    await gate.release(useful);
    await page.waitFor(`${button(`endorse-comment-${reply}`)}.dataset.pressed==='true' && ${count(`endorse-comment-${reply}`)}.startsWith('1')`, 'reply endorsement confirmed');
    check(await page.eval(`document.activeElement?.id===${JSON.stringify(`endorse-comment-${reply}`)}`), 'focus stays on the same action after confirmation');
    check(await page.eval(`${count(`endorse-comment-${root}`)}.startsWith('0') && ${count('endorse-work')}.startsWith('1')`), 'the root and the work keep their own counts (no cascade)');
    await page.key('Space');
    const off = await gate.next(7);
    check(JSON.parse(off.request.postData).endorsed === false, 'Space toggles the desired state off');
    await gate.release(off);
    await page.waitFor(`${button(`endorse-comment-${reply}`)}.dataset.pressed==='false'`, 'reply endorsement removed');
    check((await page.eval(order)) === before, 'endorsements never reorder roots or replies');
  } finally {
    await gate.disable();
  }
  const summary = await api(args, `/api/v1/works/${SAMPLE_WORK}/endorsements?comment_ids=${root},${reply}`);
  check(summary.cache === 'no-store' && summary.body.work.count === 1 && summary.body.comments.every((c) => c.count === 0), 'PostgreSQL holds exactly the confirmed states (work 1, comments 0)');
  const ssr = await (await fetch(reader)).text();
  check(ssrCount(ssr, 'endorse-work')?.text === '1', 'a fresh SSR shows the committed count');

  await page.goto(reader);
  await page.waitFor(ready, 'hydrated before the responsive matrix');
  await captureMatrix(page, 'lve-local-postgres', '#work-endorsement', 'real-local-postgresql-with-fixture-identity');

  console.log('  401 on the own-state read: every action shows the signed-out reason:');
  const unauthorized = (message) => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    const url = new URL(message.params.request.url);
    if (url.pathname.endsWith('/endorsements/me')) {
      page.send('Fetch.fulfillRequest', { requestId: message.params.requestId, responseCode: 401,
        responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
        body: Buffer.from(JSON.stringify({ error: { code: 'unauthenticated', message: 'fixture: session not recognized' } })).toString('base64'),
      }).catch(() => {});
    } else page.send('Fetch.continueRequest', { requestId: message.params.requestId }).catch(() => {});
  };
  page.cdp.listeners.push(unauthorized);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: `${args.base}/api/v1/works/*/endorsements/me*`, requestStage: 'Request' }] });
  try {
    await page.reload();
    await page.waitFor(`${note('endorse-work')}.includes('Cần đăng nhập để đánh giá.') && ${note(`endorse-comment-${reply}`)}.includes('Cần đăng nhập để đánh dấu hữu ích.')`, 'signed-out reasons');
    check(await page.eval(`${count('endorse-work')}.startsWith('1') && ${button('endorse-work')}.dataset.pressed==='unknown' && !document.querySelector('#work-endorsement a, #feedback-region a[href*="dang-nhap"]')`), 'public counts stay; no own state and no fake login');
    await page.capture('lve-signed-out-reason', { scenario: 'LVE-UC-01', boundary: 'real-local-postgresql-own-read-answered-401-by-interception' });
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter((item) => item !== unauthorized);
  }
}
