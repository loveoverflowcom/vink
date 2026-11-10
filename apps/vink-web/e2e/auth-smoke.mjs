// Auth Web browser checks: email → code → first pen name → Bàn viết → sign out, against the
// real App and the real Identity process (email-code authority) on scratch PostgreSQL. The only
// thing that is not production is the mail transport: codes are read from Identity's local
// outbox directory (`--outbox`). No LocalFixture principal exists in this phase.
//
// Real answers: 202 challenge, 422 code_rejected, 409 challenge_closed (attempts exhausted),
// 204 sign-out, 401 after the session ended, 404 for another account's draft and 429 with
// `Retry-After` from the App's own limiter. Fabricated answers, each named where it is used: the
// contract-shaped `501`/`503` of a code request and a `503` of one sign-out (the request never
// reaches the server). Interception otherwise only delays real responses.
//
// D-05 stays Provisional: this qualifies the Web flow in a test environment, not production
// authentication (no production mail transport is exercised here).

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const HYDRATED = `document.documentElement.dataset.hydrated === 'true'`;
const STANDING = `document.querySelector('#account-session')?.dataset.standing`;
const EDITOR_READY = `document.querySelector('[data-testid=editor-body] .cm-content[contenteditable=true]')`;
const PANEL_TEXT = `(document.querySelector('.vk-account')?.textContent ?? '')`;
const PEN_NAME_A = 'Nguyễn Thị Ánh';
const PEN_NAME_B = 'Bút danh thứ hai';
const TITLE_A = 'Auth Web — bản nháp của người thứ nhất';
const BODY_A = 'Dòng đầu viết sau khi đăng nhập,\n  dòng thụt vào còn nguyên dấu: ỹ ỷ ữ ợ Đ đ.';
const UNSAVED = '\n\nKhổ chưa lưu khi phiên kết thúc.';
const UNSAVED_LINE = 'Khổ chưa lưu khi phiên kết thúc.';

function q(selector) {
  return `document.querySelector(${JSON.stringify(selector)})`;
}

/** Replaces the value of a text field as committed input. */
async function fill(page, selector, text) {
  await page.eval(`(() => { const el = ${q(selector)}; el.focus(); el.select(); })()`);
  if (text) await page.send('Input.insertText', { text });
  else await page.key('Backspace');
}

/** Reads the code Identity wrote for `address` to its local outbox (a file per challenge). */
async function mailedCode(outbox, address, seen) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    for (const name of readdirSync(outbox)) {
      if (seen.has(name)) continue;
      const text = readFileSync(join(outbox, name), 'utf8');
      if (!text.includes(`To: ${address}\n`)) continue;
      seen.add(name);
      const code = /^X-Vink-Code: (\d{6})$/m.exec(text)?.[1];
      if (!code) throw new Error(`outbox message ${name} carries no code`);
      return code;
    }
    await sleep(100);
  }
  throw new Error(`no outbox message for ${address} within 15s`);
}

async function cookies(page, base) {
  const { cookies } = await page.send('Network.getCookies', { urls: [base] });
  return Object.fromEntries(cookies.map((c) => [c.name, c]));
}

async function openAccount(page, base, standing) {
  await page.goto(`${base}/toi`);
  await page.waitFor(`${HYDRATED} && ${STANDING} === ${JSON.stringify(standing)}`, `Tôi: ${standing}`);
}

/** Email → code through the form. Leaves the tab at the pen-name step or signed in. */
async function signIn(page, ctx, address) {
  const { args, seen } = ctx;
  await page.waitFor(`${STANDING} === 'anonymous' && ${q('#sign-in-address')}`, 'sign-in form');
  await fill(page, '#sign-in-address', address);
  await page.click('#sign-in-send');
  await page.waitFor(q('#sign-in-digits'), `code step for ${address}`);
  const code = await mailedCode(args.outbox, address, seen);
  await fill(page, '#sign-in-digits', code);
  await page.click('#sign-in-complete');
  await page.waitFor(`['needs_principal','active'].includes(${STANDING})`, `session for ${address}`);
  return code;
}

async function signOut(page, base) {
  const answers = [];
  const listener = (message) => {
    if (message.sessionId !== page.sessionId || message.method !== 'Network.responseReceived') return;
    const { response } = message.params;
    if (response.url === `${base}/api/v1/auth/session` && message.params.type !== 'Preflight') answers.push(response.status);
  };
  page.cdp.listeners.push(listener);
  await page.click('#sign-out');
  await page.waitFor(`${STANDING} === 'anonymous'`, 'signed out');
  page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener), 1);
  return answers;
}

/** Answers the next request matching `method path` itself; the server never sees it. */
async function fabricate(page, method, path, status, body, headers = []) {
  let done;
  const answered = new Promise((resolve) => (done = resolve));
  const listener = (message) => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    const { requestId, request } = message.params;
    if (request.method === method && new URL(request.url).pathname === path) {
      page.send('Fetch.fulfillRequest', {
        requestId,
        responseCode: status,
        responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }, ...headers],
        body: Buffer.from(JSON.stringify(body)).toString('base64'),
      }).then(done);
    } else page.send('Fetch.continueRequest', { requestId }).catch(() => {});
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: `*${path}`, requestStage: 'Request' }] });
  return async () => {
    await answered;
    page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener), 1);
    await page.send('Fetch.disable');
  };
}

/** Holds the first real draft-list response; later ones pass. */
class ListGate {
  constructor(page) {
    this.page = page;
    this.first = null;
    this.passed = 0;
    this.listener = (message) => {
      if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
      const { requestId, request, responseStatusCode } = message.params;
      const list = request.method === 'GET' && new URL(request.url).pathname === '/api/v1/drafts';
      if (list && !this.first) this.first = { requestId, status: responseStatusCode };
      else {
        if (list) this.passed += 1;
        page.send('Fetch.continueRequest', { requestId }).catch(() => {});
      }
    };
  }

  async enable() {
    this.page.cdp.listeners.push(this.listener);
    await this.page.send('Fetch.enable', { patterns: [{ urlPattern: '*/api/v1/drafts*', requestStage: 'Response' }] });
  }

  async held() {
    const deadline = Date.now() + 15000;
    while (!this.first) {
      if (Date.now() > deadline) throw new Error('timed out waiting for the draft list response');
      await sleep(50);
    }
    return this.first;
  }

  async release() {
    await this.page.send('Fetch.continueRequest', { requestId: this.first.requestId });
  }

  async disable() {
    this.page.cdp.listeners.splice(this.page.cdp.listeners.indexOf(this.listener), 1);
    await this.page.send('Fetch.disable');
  }
}

const storedText = (draft) => draft.document.blocks.map((block) => block.lines.join('\n')).join('\n\n');
const account = (page) => page.eval(`fetch('/api/v1/auth/session/account').then(async (r) => ({ status: r.status, body: r.ok ? await r.json() : null }))`);
const fieldError = (id) => `(document.getElementById(${JSON.stringify(id)})?.getAttribute('aria-describedby') ?? '').split(' ').map((i) => document.getElementById(i)?.textContent ?? '').join(' ')`;

export async function authWeb(page, { args, check, openPage }) {
  if (!args.outbox) throw new Error('phase auth needs --outbox');
  const base = args.base;
  const ctx = { args, seen: new Set() };
  const stamp = Date.now().toString(36);
  const mailA = `nguoi-viet-a-${stamp}@vi-du.vn`;
  const mailB = `nguoi-viet-b-${stamp}@vi-du.vn`;
  const mailC = `nguoi-quen-ma-${stamp}@vi-du.vn`;

  console.log('Auth Web: a guest, and nothing about a session in server HTML:');
  const html = await (await fetch(`${base}/toi`)).text();
  check(html.includes('name="vink-sign-in"') && html.includes('data-standing="unknown"') && !html.includes('id="sign-in-email"'), 'SSR says only whether sign-in is open; the standing is unknown until the browser asks');
  await openAccount(page, base, 'anonymous');
  check(await page.eval(`${q('#sign-in-email')} && !${q('#sign-out')} && ${PANEL_TEXT}.includes('Bạn chưa đăng nhập')`), 'a guest sees the email step and no sign-out');
  await page.goto(`${base}/ban-viet`);
  await page.waitFor(`${HYDRATED} && ${q('#desk-sign-in')}`, 'guest desk');
  check(await page.eval(`${q('#desk-sign-in')}.getAttribute('href') === '/toi' && !${q('#create-draft')} && !${q('[data-testid=draft-empty]')}`), 'Bàn viết points a guest to Tôi and offers neither create nor an empty list');

  console.log('Auth Web: code request failures keep the typed address:');
  await openAccount(page, base, 'anonymous');
  await fill(page, '#sign-in-address', 'khong-phai-email');
  await page.click('#sign-in-send');
  await page.waitFor(`${fieldError('sign-in-address')}.includes('Địa chỉ email chưa đúng')`, 'local email refusal');
  check(await page.eval(`performance.getEntriesByType('resource').every((e) => !e.name.includes('/auth/sign-in/'))`), 'an address without @ is refused before any request');
  for (const [status, code, text] of [
    [503, 'dependency_unavailable', 'tạm thời chưa phản hồi'],
    [501, 'authority_not_configured', 'Vink chưa mở đăng nhập'],
  ]) {
    await fill(page, '#sign-in-address', mailC);
    // Fabricated: contract-shaped answer; the request does not reach the App.
    const done = await fabricate(page, 'POST', '/api/v1/auth/sign-in/challenges', status, { error: { code, message: 'test fixture' } });
    await page.click('#sign-in-send');
    await done();
    await page.waitFor(`${fieldError('sign-in-address')}.includes(${JSON.stringify(text)})`, `${status} recovery copy`);
    check(await page.eval(`${q('#sign-in-address')}.value === ${JSON.stringify(mailC)} && ${q('#sign-in-email')} && !${PANEL_TEXT}.includes('test fixture')`), `${status} ${code} (fabricated): own recovery copy, the address stays, server text is not echoed`);
  }

  console.log('Auth Web: wrong code, then a challenge closed by the real Identity:');
  await page.click('#sign-in-send');
  await page.waitFor(q('#sign-in-digits'), 'code step');
  const realC = await mailedCode(args.outbox, mailC, ctx.seen);
  const challenge = (await cookies(page, base))['__Host-vink_sign_in'];
  check(challenge?.httpOnly && challenge.secure && !(await page.eval(`document.cookie`)).includes('vink'), 'the pending challenge is an HttpOnly Secure cookie scripts cannot read');
  check(await page.eval(`${q('#sign-in-sent')}.textContent.includes(${JSON.stringify(mailC)}) && ${q('#sign-in-resend')}.getAttribute('aria-disabled') === 'true' && ${fieldError('sign-in-resend')}.includes('Có thể gửi lại sau')`), 'the code step names the address and makes a resend wait, with the reason in text');
  await fill(page, '#sign-in-digits', '12');
  await page.click('#sign-in-complete');
  await page.waitFor(`${fieldError('sign-in-digits')}.includes('đúng 6 chữ số')`, 'local code refusal');
  const wrong = String((Number(realC) + 1) % 1000000).padStart(6, '0');
  await fill(page, '#sign-in-digits', wrong);
  await page.click('#sign-in-complete');
  await page.waitFor(`${fieldError('sign-in-digits')}.includes('Mã chưa đúng')`, 'wrong code');
  check(await page.eval(`${q('#sign-in-digits')}.value === ${JSON.stringify(wrong)} && ${STANDING} === 'anonymous'`), '422 code_rejected: the same step accepts another attempt and keeps what was typed');
  let attempts = 1;
  while (await page.eval(`Boolean(${q('#sign-in-digits')})`)) {
    if (attempts > 7) throw new Error('the challenge never closed');
    await page.waitFor(`!${q('#sign-in-digits')} || ${q('#sign-in-complete')}.getAttribute('aria-disabled') !== 'true'`, 'attempt settled');
    if (!(await page.eval(`Boolean(${q('#sign-in-digits')})`))) break;
    await page.click('#sign-in-complete');
    attempts += 1;
    await sleep(250);
    await page.waitFor(`!${q('#sign-in-digits')} || ${q('#sign-in-complete')}.getAttribute('aria-busy') !== 'true'`, 'attempt answered');
  }
  await page.waitFor(`${fieldError('sign-in-address')}.includes('Hãy xin mã mới')`, 'closed challenge');
  check(attempts >= 5 && attempts <= 7 && await page.eval(`${q('#sign-in-address')}.value === ${JSON.stringify(mailC)}`), `409 challenge_closed after ${attempts} wrong codes: back to the email step with the address kept`);

  console.log('Auth Web: a new person signs in and chooses a pen name:');
  await openAccount(page, base, 'anonymous');
  await signIn(page, ctx, mailA);
  await page.waitFor(`${STANDING} === 'needs_principal' && ${q('#pen-name-form')}`, 'pen-name step');
  const session = (await cookies(page, base))['__Host-vink_session'];
  check(session?.httpOnly && session.secure && session.path === '/' && !(await page.eval(`document.cookie`)).includes('vink'), 'the session is an HttpOnly Secure __Host- cookie');
  check(await page.eval(`(() => { const all = JSON.stringify([Object.entries(localStorage), Object.entries(sessionStorage)]); return !all.includes('vi-du.vn') && !all.includes('vink_s'); })()`), 'no address or credential reaches browser storage');
  await page.reload();
  await page.waitFor(`${STANDING} === 'needs_principal' && ${q('#pen-name')}`, 'pen-name step after reload');
  check(true, 'a reload in the middle resumes at the pen-name step');
  await page.click('#pen-name-save');
  await page.waitFor(`${fieldError('pen-name')}.includes('1 đến 80 ký tự')`, 'empty pen name');
  await fill(page, '#pen-name', PEN_NAME_A);
  await page.click('#pen-name-save');
  await page.waitFor(`${STANDING} === 'active'`, 'active account');
  const first = await account(page);
  check(first.status === 200 && first.body.display_name === PEN_NAME_A && await page.eval(`${PANEL_TEXT}.includes(${JSON.stringify(PEN_NAME_A)}) && Boolean(${q('#sign-out')})`), 'the account region shows the Vietnamese pen name and offers sign-out');
  const principalA = first.body.principal_id;
  const again = await page.eval(`fetch('/api/v1/auth/principal', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ pen_name: 'Tên gửi lại' }) }).then(async (r) => ({ status: r.status, body: await r.json().catch(() => null) }))`);
  const afterRetry = await account(page);
  check(again.status < 500 && afterRetry.body.principal_id === principalA && afterRetry.body.display_name === PEN_NAME_A && (again.body?.principal_id ?? principalA) === principalA, `a repeated principal request (${again.status}) creates no second account and changes nothing`);

  console.log('Auth Web: Bàn viết through the real session:');
  await page.click('#navigation-primary a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && ${q('#create-draft')}`, 'own desk');
  await page.click('#create-draft');
  await page.waitFor(`location.pathname.startsWith('/ban-viet/') && ${EDITOR_READY}`, 'new draft editor');
  const workA = await page.eval(`location.pathname.split('/').pop()`);
  await fill(page, '#draft-title', TITLE_A);
  await page.typeInBody(BODY_A);
  await page.click('#save');
  await page.waitStatus('saved', 'first save');
  await page.reload();
  await page.waitFor(`${EDITOR_READY} && ${q('#draft-title')}.value === ${JSON.stringify(TITLE_A)}`, 'draft read again');
  check((await page.body()) === BODY_A, 'the draft is created, saved and read again with lines and marks intact');
  const stored = await page.draft(workA);
  check(storedText(stored) === BODY_A && stored.title === TITLE_A, `the App stored it for this principal${storedText(stored) === BODY_A ? '' : `: ${JSON.stringify(stored)}`}`);

  console.log('Auth Web: sign-out, and typed text when the session ends:');
  await page.typeInBody(UNSAVED);
  await page.waitStatus('unsaved');
  const other = await openPage();
  await openAccount(other, base, 'active');
  // Fabricated: Identity did not confirm the revocation. The request does not reach the App.
  const refused = await fabricate(other, 'DELETE', '/api/v1/auth/session', 503, { error: { code: 'dependency_unavailable', message: 'test fixture' } });
  await other.click('#sign-out');
  await refused();
  await other.waitFor(q('#sign-out-unconfirmed'), 'unconfirmed sign-out');
  check(await other.eval(`${STANDING} === 'active' && ${q('#sign-out-unconfirmed')}.textContent.includes('Chưa đăng xuất xong') && ${q('#sign-out-unconfirmed')}.textContent.includes('vẫn còn hiệu lực')`) && (await account(other)).status === 200, '503 on sign-out (fabricated): reported as not completed; the session is still shown and still valid');
  const answers = await signOut(other, base);
  check(answers.includes(204) && !(await cookies(other, base))['__Host-vink_session'] && (await account(other)).status === 401, `sign-out answers 204 without a body (${answers.join(',')}); the cookie is gone and the account read is 401`);
  check(other.errors.length === 0, `the 204 is not parsed as JSON${other.errors.length ? `: ${other.errors.join(' | ')}` : ''}`);
  await page.click('#save');
  await page.waitStatus('failed', 'save after the session ended');
  check(await page.eval(`${q('[data-testid=save-failed]')}.textContent.includes('Phiên đăng nhập đã hết')`) && (await page.body()).endsWith(UNSAVED_LINE), 'the editor in the other tab keeps the unsaved stanza and says the session ended');

  console.log('Auth Web: the same person returns:');
  await signIn(other, ctx, mailA);
  await other.waitFor(`${STANDING} === 'active'`, 'returning account');
  const returned = await account(other);
  check(returned.body.principal_id === principalA && returned.body.display_name === PEN_NAME_A && !(await other.eval(`Boolean(${q('#pen-name-form')})`)), 'a returning address gets the same principal and no second pen-name step');
  await sleep(500);
  check((await page.body()).endsWith(UNSAVED_LINE), 'the text typed before the session ended is still in the editor');
  await page.click('#retry-save');
  await page.waitStatus('saved', 'save after signing in again');
  check(storedText(await page.draft(workA)) === BODY_A + UNSAVED, 'and is saved once the same account is back');

  console.log('Auth Web: another account takes over the browser (two tabs):');
  await page.typeInBody(' (A)');
  await page.waitStatus('unsaved');
  const desk = await openPage();
  const gate = new ListGate(desk);
  await desk.goto(`${base}/ban-viet`);
  await desk.waitFor(`${q('[data-testid=draft-row]')} && document.body.textContent.includes(${JSON.stringify(TITLE_A)})`, "A's desk in a third tab");
  await gate.enable();
  await desk.click('#refresh-drafts');
  const held = await gate.held();
  check(held.status === 200, "A's desk lists the draft; a second list answer (200) is held before the page receives it");
  const out = await signOut(other, base);
  check(out.includes(204), 'A signs out (204)');
  await signIn(other, ctx, mailB);
  await other.waitFor(`${STANDING} === 'needs_principal' && ${q('#pen-name')}`, "B's pen-name step");
  await fill(other, '#pen-name', PEN_NAME_B);
  await other.click('#pen-name-save');
  await other.waitFor(`${STANDING} === 'active'`, 'B active');
  const second = await account(other);
  check(second.body.principal_id !== principalA && second.body.display_name === PEN_NAME_B, 'B is another principal');
  // Reads are serial: B's list waits behind A's held answer. Nothing of A may show meanwhile.
  await desk.waitFor(`${q('[data-testid=draft-loading]')} && !${q('[data-testid=draft-row]')}`, 'the waiting tab shows loading for the new session');
  check(await desk.eval(`!document.body.textContent.includes(${JSON.stringify(TITLE_A)})`) && gate.passed === 0, "A's list leaves the screen as soon as the session changes, before A's late answer arrives");
  await gate.release();
  await desk.waitFor(`${q('[data-testid=draft-empty]')} && ${q('#create-draft')}`, "B's own empty desk in the tab that was waiting");
  check(gate.passed >= 1 && await desk.eval(`!document.body.textContent.includes(${JSON.stringify(TITLE_A)}) && !${q('[data-testid=draft-row]')}`), "A's late list reply is dropped; the tab asks again and shows B's empty desk");
  await gate.disable();
  await page.waitFor(`!document.body.textContent.includes(${JSON.stringify(UNSAVED_LINE)}) && !${q('#draft-title')}?.value`, "A's editor let go of A's text");
  check(await page.eval(`!document.body.textContent.includes('Dòng đầu viết sau khi đăng nhập') && !document.body.textContent.includes(${JSON.stringify(TITLE_A)})`), "the editor tab holds none of A's title or text once B is confirmed");
  const foreign = await page.eval(`fetch('/api/v1/drafts/${workA}').then((r) => r.status)`);
  check(foreign === 404, "the App answers 404 for A's draft to B");
  await desk.reload();
  await desk.waitFor(`${q('[data-testid=draft-empty]')}`, "B's desk after reload");
  check(await desk.eval(`!document.body.textContent.includes(${JSON.stringify(TITLE_A)})`), "a reload shows B's desk only");

  console.log('Auth Web: the real limiter answers 429 with Retry-After:');
  check((await signOut(other, base)).includes(204), 'B signs out (204)');
  let burned = 0;
  for (; burned < 14; burned++) {
    const status = await other.eval(`fetch('/api/v1/auth/sign-in/challenges', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ email: 'gioi-han-${stamp}-${burned}@vi-du.vn' }) }).then((r) => r.status)`);
    if (status === 429) break;
    if (status !== 202) throw new Error(`unexpected ${status} while reaching the limit`);
  }
  check(burned < 14, `the App's sign-in limit is reached after ${burned} more request(s)`);
  await openAccount(other, base, 'anonymous');
  await fill(other, '#sign-in-address', mailB);
  await other.click('#sign-in-send');
  await other.waitFor(`${fieldError('sign-in-address')}.includes('Có quá nhiều yêu cầu')`, '429 recovery copy');
  check(await other.eval(`/thử lại sau \\d+ (giây|phút)/.test(${fieldError('sign-in-address')}) && ${q('#sign-in-address')}.value === ${JSON.stringify(mailB)} && ${q('#sign-in-send')}.getAttribute('aria-disabled') === 'true' && /Có thể xin mã sau \\d+ (giây|phút)/.test(${fieldError('sign-in-send')})`), '429 (real): the wait from Retry-After is shown, the address stays and the request waits');

  for (const [name, tab] of [['account', other], ['desk', desk]]) {
    check(tab.errors.length === 0, `no console errors in the ${name} tab${tab.errors.length ? `: ${tab.errors.join(' | ')}` : ''}`);
  }
}
