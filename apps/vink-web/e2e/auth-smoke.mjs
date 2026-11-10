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

  const discussionReader = await discussionSessionRaces(other, ctx, mailA, mailB, openPage, check);

  console.log('Auth Web: the real limiter answers 429 with Retry-After:');
  check((await signOut(other, base)).includes(204), 'B signs out (204)');
  await discussionReader.waitFor(`!document.querySelector('#own-comments')`, 'real revoked logout clears Reader private region');
  check(await discussionReader.eval(`document.querySelector('#root-composer').value === 'B đang viết sau đổi phiên' && !document.body.textContent.includes('Bình luận nêu danh tính đời thực') && !document.body.textContent.includes('Một góc nhìn khác về khổ hai.') && !document.querySelector('[data-testid=discussion-receipt]')?.textContent?.trim()`), 'Reader/Auth real 204 logout removes own fixture body/reason/receipt and keeps only B typed text');
  check(await discussionReader.eval(`fetch('/api/v1/works/33333333-3333-4333-8333-333333333333/discussion/me').then(r=>r.status)`) === 401, 'Reader/Auth expired authority answers 401; the existing Session owner handles refresh');
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

/** Reader privacy/races through real App + Identity. Cookies below are server-issued; CDP
 * switches between two still-valid test sessions to exercise direct A→B as well as logout.
 * No identity response is fabricated. A gate holds real responses, never their payloads. */
async function discussionSessionRaces(other, ctx, mailA, mailB, openPage, check) {
  const base = ctx.args.base;
  const work = '33333333-3333-4333-8333-333333333333';
  const endpoint = `/api/v1/works/${work}/discussion`;
  const cookieB = (await cookies(other, base))['__Host-vink_session'];
  const avatarB = await other.eval(`document.querySelector('.vk-nav-account-avatar [data-initials]')?.dataset.initials`);
  const announce = async () => other.eval(`(() => { const c = new BroadcastChannel('vink-session'); c.postMessage('changed'); c.close(); dispatchEvent(new Event('focus')); })()`);
  const select = async (cookie) => {
    if (cookie) await other.send('Network.setCookie', { name: cookie.name, value: cookie.value, url: base, path: '/', secure: true, httpOnly: true, sameSite: cookie.sameSite });
    else await other.send('Network.deleteCookies', { name: '__Host-vink_session', url: base });
    await announce();
  };
  const post = async (tab, body) => tab.eval(`fetch('${endpoint}/comments', { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({target_id:null, body:${JSON.stringify(body)}, command_key:crypto.randomUUID()}) }).then(async r=>({status:r.status, body:await r.json()}))`);
  const textB = 'Riêng B — nhận xét được đọc qua phiên thật';
  const createdB = await post(other, textB);
  check(createdB.status === 201, 'Reader/Auth: B creates a real pending comment');
  // Keep B's session valid at Identity, select guest in this browser and issue A another session.
  await select(null);
  await openAccount(other, base, 'anonymous');
  await signIn(other, ctx, mailA);
  await other.waitFor(`${STANDING} === 'active'`, 'Reader: A active');
  const cookieA = (await cookies(other, base))['__Host-vink_session'];
  const textA = 'Riêng A — nhận xét được đọc qua phiên thật';
  const createdA = await post(other, textA);
  check(createdA.status === 201, 'Reader/Auth: A creates a real pending comment');
  const reader = await openPage();
  await reader.goto(`${base}/tac-pham/${work}`);
  const ownText = `(document.querySelector('#own-comments')?.textContent ?? '')`;
  const settled = `document.querySelector('#own-comments-refresh')?.getAttribute('aria-busy') !== 'true' && document.querySelector('#discussion-refresh')?.getAttribute('aria-busy') !== 'true'`;
  await reader.waitFor(`${ownText}.includes(${JSON.stringify(textA)}) && ${settled}`, 'Reader: own A loaded');
  const html = await reader.eval(`fetch(location.href).then(r=>r.text())`);
  check(!html.includes(textA) && !html.includes(textB) && !html.includes('own_pending') && !html.includes('viewer_is_work_author'), 'Reader/Auth: SSR and hydration contain no own body or viewer facts');
  await reader.typeAtEnd('#root-composer', 'Nháp của A');
  // Hold one real own-list response, let later requests through even before it is released.
  const gate = new ListGate(reader);
  gate.listener = (message) => {
    if (message.sessionId !== reader.sessionId || message.method !== 'Fetch.requestPaused') return;
    const { requestId, request, responseStatusCode } = message.params;
    const u = new URL(request.url);
    if (u.pathname === `${endpoint}/me` && !u.searchParams.has('ids') && !gate.first) gate.first = { requestId, status: responseStatusCode };
    else if (u.pathname === `${endpoint}/public` && !gate.publicFirst) gate.publicFirst = { requestId, status: responseStatusCode };
    else { gate.passed++; reader.send('Fetch.continueRequest', {requestId}).catch(()=>{}); }
  };
  gate.enable = async () => {
    reader.cdp.listeners.push(gate.listener);
    await reader.send('Fetch.enable', {patterns:[{urlPattern:`*${endpoint}/me*`, requestStage:'Response'},{urlPattern:`*${endpoint}/public*`, requestStage:'Response'}]});
  };
  await gate.enable();
  await reader.click('#discussion-refresh');
  await gate.held();
  const publicDeadline = Date.now()+10000;
  while (!gate.publicFirst) { if (Date.now()>publicDeadline) throw new Error('held public response missing'); await sleep(30); }
  await select(cookieB); // Direct A → B; no anonymous observation in between.
  await reader.waitFor(`${ownText}.includes(${JSON.stringify(textB)}) && !${ownText}.includes(${JSON.stringify(textA)}) && ${settled}`, 'B loads before the obsolete A answer is released');
  check(gate.passed > 0 && await reader.eval(`document.querySelector('#root-composer').value === ''`), 'Reader/Auth direct A→B: private projection and draft clear immediately; B read progresses before old A response');
  await gate.release().catch(()=>{}); // The canceled fetch may already be detached by Chrome.
  await reader.send('Fetch.continueRequest',{requestId:gate.publicFirst.requestId}).catch(()=>{});
  await gate.disable();
  check(await reader.eval(`!${ownText}.includes(${JSON.stringify(textA)}) && ${settled}`), 'Reader/Auth: released A response neither leaks data nor strands loading');
  // A → guest → A keeps only typed text, reads private facts again, never revives receipts.
  await select(cookieA);
  await reader.waitFor(`${ownText}.includes(${JSON.stringify(textA)}) && ${settled}`, 'return A');
  await reader.typeAtEnd('#root-composer', 'Chữ mới của A');
  await select(null);
  await reader.waitFor(`!document.querySelector('#own-comments') && document.querySelector('#root-composer').value === 'Chữ mới của A'`, 'A to guest clears private state and keeps own text');
  await select(cookieA);
  await reader.waitFor(`${ownText}.includes(${JSON.stringify(textA)}) && ${settled}`, 'guest to same A reads fresh own data');
  const returnedFacts = await reader.eval(`({text:document.querySelector('#root-composer').value, receipt:document.querySelector('[data-testid=discussion-receipt]')?.textContent?.trim() ?? ''})`);
  check(returnedFacts.text === 'Chữ mới của A' && !returnedFacts.receipt, `Reader/Auth A→guest→A: text survives; no old receipt returns ${JSON.stringify(returnedFacts)}`);
  // A real write commits, but its ACK is held across another account generation.
  let ack;
  const listener = m => {
    if (m.sessionId !== reader.sessionId || m.method !== 'Fetch.requestPaused') return;
    ack = m.params;
  };
  reader.cdp.listeners.push(listener);
  await reader.send('Fetch.enable',{patterns:[{urlPattern:`*${endpoint}/comments`,requestStage:'Response'}]});
  await reader.click('#root-composer-send');
  const deadline = Date.now()+10000;
  while (!ack) { if (Date.now()>deadline) throw new Error('held real comment ACK missing'); await sleep(30); }
  check(ack.responseStatusCode === 201, 'Reader/Auth: held ACK belongs to a genuinely committed comment');
  await select(null);
  await reader.waitFor(`!document.querySelector('#own-comments')`, 'A write to guest');
  await select(cookieB);
  await reader.waitFor(`${ownText}.includes(${JSON.stringify(textB)}) && ${settled}`, 'A→guest→B while write answer held');
  await reader.typeAtEnd('#root-composer','B đang viết sau đổi phiên');
  await reader.send('Fetch.continueRequest',{requestId:ack.requestId});
  reader.cdp.listeners.splice(reader.cdp.listeners.indexOf(listener),1);
  await reader.send('Fetch.disable');
  await reader.waitFor(`document.querySelector('#root-composer-send')?.getAttribute('aria-busy') !== 'true'`, 'B send is not busy after A ACK');
  check(await reader.eval(`document.querySelector('#root-composer').value === 'B đang viết sau đổi phiên' && !${ownText}.includes(${JSON.stringify(textA)}) && !document.querySelector('[data-testid=discussion-receipt]')?.textContent?.trim()`), 'Reader/Auth A→guest→B: A ACK cannot clear B text, restore receipt, or own data');
  // Real remove ACKs in two generations: old completion must not clear the new busy owner.
  const removes = [];
  const removeListener = m => {
    if (m.sessionId !== reader.sessionId || m.method !== 'Fetch.requestPaused') return;
    removes.push(m.params);
  };
  reader.cdp.listeners.push(removeListener);
  await reader.send('Fetch.enable',{patterns:[{urlPattern:`*${endpoint}/comments/*`,requestStage:'Response'}]});
  const waitRemoves = async count => { const until = Date.now()+10000; while (removes.length<count) { if (Date.now()>until) throw new Error('held remove ACK missing'); await sleep(30); } };
  await reader.click(`#remove-comment-${createdB.body.comment_id}`);
  await waitRemoves(1);
  await select(cookieA);
  await reader.waitFor(`document.querySelector('#remove-comment-${createdA.body.comment_id}') && ${settled}`, 'A loads while obsolete B removal ACK is held');
  await reader.click(`#remove-comment-${createdA.body.comment_id}`);
  await waitRemoves(2);
  await reader.send('Fetch.continueRequest',{requestId:removes[0].requestId});
  check(await reader.eval(`document.querySelector('#discussion-refresh')?.getAttribute('aria-busy') === 'true'`), 'Reader/Auth: B removal ACK cannot clear the new A mutation busy owner');
  await reader.send('Fetch.continueRequest',{requestId:removes[1].requestId});
  reader.cdp.listeners.splice(reader.cdp.listeners.indexOf(removeListener),1);
  await reader.send('Fetch.disable');
  await reader.waitFor(settled, 'A mutation completes and releases busy');
  await select(cookieB);
  await reader.waitFor(`document.querySelector('.vk-nav-account-avatar [data-initials]')?.dataset.initials === ${JSON.stringify(avatarB)} && document.querySelector('#own-comments-refresh') && ${settled}`, 'return B confirmed by the Session-driven avatar after removals');
  await reader.waitFor(`${ownText}.includes('Chưa có nhận xét riêng') && ${settled}`, 'B own empty state after removals');
  check(await reader.eval(`!${ownText}.includes(${JSON.stringify(textA)})`), 'Reader/Auth: successful empty own list is distinct from failure and contains no A data');
  check((await post(other, textB)).status === 201, 'Reader/Auth: B creates a fresh pending comment after removal');
  await reader.click('#own-comments-refresh');
  await reader.waitFor(`${ownText}.includes(${JSON.stringify(textB)}) && ${settled}`, 'fresh B read after removals');
  await reader.typeAtEnd('#root-composer','B đang viết sau đổi phiên');
  // Fabricated failure of own/capability only; public GET remains real and usable.
  const publicBefore = await reader.eval(`document.querySelector('#root-list')?.textContent`);
  const failed = m => {
    if (m.sessionId !== reader.sessionId || m.method !== 'Fetch.requestPaused') return;
    reader.send('Fetch.fulfillRequest',{requestId:m.params.requestId,responseCode:503,responseHeaders:[{name:'Content-Type',value:'application/json'}],body:Buffer.from(JSON.stringify({error:{code:'dependency_unavailable',message:'fixture Identity outage'}})).toString('base64')}).catch(()=>{});
  };
  reader.cdp.listeners.push(failed);
  await reader.send('Fetch.enable',{patterns:[{urlPattern:`*${endpoint}/me*`,requestStage:'Request'},{urlPattern:`*${endpoint}?limit=1`,requestStage:'Request'}]});
  await reader.click('#own-comments-refresh');
  await reader.waitFor(`${ownText}.includes('Chưa làm mới được') && ${settled}`, 'private failure is local');
  const failureFacts = await reader.eval(`({publicText:document.querySelector('#root-list')?.textContent, draft:document.querySelector('#root-composer').value})`);
  check(failureFacts.publicText === publicBefore && failureFacts.draft === 'B đang viết sau đổi phiên', `Reader/Auth fabricated Identity read failure: public timeline and newer text remain ${JSON.stringify(failureFacts)}`);
  await reader.capture('discussion-own-private-error',{boundary:'fabricated-identity-outage-real-auth-session'});
  reader.cdp.listeners.splice(reader.cdp.listeners.indexOf(failed),1);
  await reader.send('Fetch.disable');
  await reader.click('#own-comments-refresh');
  await reader.waitFor(`${ownText}.includes(${JSON.stringify(textB)}) && !${ownText}.includes('Chưa làm mới được') && ${settled}`, 'private retry recovers with real B data');
  check(await reader.eval(`(() => { const stored=JSON.stringify([Object.entries(localStorage),Object.entries(sessionStorage)]); return !stored.includes(${JSON.stringify(textA)}) && !stored.includes(${JSON.stringify(textB)}) && !stored.includes('B đang viết sau đổi phiên') && !stored.includes('Chờ duyệt'); })()`), 'Reader/Auth: own bodies, draft and receipt never enter localStorage/sessionStorage');
  await discussionOwnFixtures(reader, endpoint, check);
  await openAccount(other, base, 'active'); // B remains selected for the existing limiter test.
  return reader;
}

/** Labelled provider fixtures for off-page/unlisted/rejected and reconciliation, separate
 * from the real-session race evidence above. No fixture is claimed as database mutation. */
async function discussionOwnFixtures(page, endpoint, check) {
  const fixture = JSON.parse(readFileSync(new URL('../../../services/vink-app/contracts/http/v1/fixtures/discussion.me.get.json', import.meta.url),'utf8'));
  let own = structuredClone(fixture);
  const ownCursors = [], ownBatches = [];
  const listener = m => {
    if (m.sessionId !== page.sessionId || m.method !== 'Fetch.requestPaused') return;
    const u = new URL(m.params.request.url), ids = u.searchParams.get('ids')?.split(',');
    const cursor = u.searchParams.get('cursor');
    if (ids) ownBatches.push(ids); else ownCursors.push(cursor);
    const body = {...own, comments: ids ? own.comments.filter(c=>ids.includes(c.comment_id)) : cursor ? own.comments.slice(2) : own.comments.slice(0,2), next_cursor: ids || cursor ? null : 'fixture-own-next'};
    page.send('Fetch.fulfillRequest',{requestId:m.params.requestId,responseCode:200,responseHeaders:[{name:'Content-Type',value:'application/json'},{name:'Cache-Control',value:'no-store'}],body:Buffer.from(JSON.stringify(body)).toString('base64')}).catch(()=>{});
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable',{patterns:[{urlPattern:`*${endpoint}/me*`,requestStage:'Request'}]});
  await page.click('#own-comments-refresh');
  const rejected=fixture.comments[2];
  await page.waitFor(`document.querySelector('#own-comments-more') && document.querySelector('#own-comments-refresh')?.getAttribute('aria-busy') !== 'true'`, 'independent own cursor');
  await page.click('#own-comments-more');
  await page.waitFor(`document.querySelector('#own-comments')?.textContent.includes(${JSON.stringify(rejected.rejection_reason)}) && document.querySelector('#own-comments-refresh')?.getAttribute('aria-busy') !== 'true'`, 'own rejected fixture');
  check(ownCursors.includes('fixture-own-next') && ownBatches.every(b=>b.length<=20 && b.length===new Set(b).size) && ownBatches.some(b=>b.length>1), 'labelled own fixtures: separate own cursor and deduplicated multi-ID lookup batches <=20');
  check(await page.eval(`document.querySelector('#own-comments').textContent.includes('Nhánh chưa có trong danh sách công khai') && !document.querySelector('#root-list').textContent.includes(${JSON.stringify(rejected.body)})`), 'labelled own fixtures: off-page pending, unlisted-root reply and own rejection remain outside the public timeline');
  const beforeInput = await page.eval(`document.querySelector('#root-composer').value`);
  await page.eval(`window.__discussionInput = document.querySelector('#root-composer'); window.__discussionInput.focus(); window.__discussionInput.setSelectionRange(window.__discussionInput.value.length,window.__discussionInput.value.length)`);
  await page.send('Input.imeSetComposition',{text:'d',selectionStart:1,selectionEnd:1});
  await page.send('Input.imeSetComposition',{text:'đ',selectionStart:1,selectionEnd:1});
  // Refresh without moving native focus away from the active composition.
  await page.eval(`document.querySelector('#own-comments-refresh').click()`);
  await page.waitFor(`document.querySelector('#own-comments-refresh')?.getAttribute('aria-busy') !== 'true'`, 'own refresh during simulated composition');
  await page.send('Input.insertText',{text:'đ'});
  check(await page.eval(`window.__discussionInput === document.querySelector('#root-composer') && window.__discussionInput.value === ${JSON.stringify(beforeInput+'đ')}`), 'own fixture refresh preserves native composition/input identity (CDP simulation)');
  for (const [command, modifiers, expected] of [['undo',page.mod,beforeInput],['redo',page.mod|8,beforeInput+'đ'],['undo',page.mod,beforeInput]]) {
    await page.send('Input.dispatchKeyEvent',{type:'keyDown',key:'z',code:'KeyZ',modifiers,commands:[command]});
    await page.send('Input.dispatchKeyEvent',{type:'keyUp',key:'z',code:'KeyZ',modifiers});
    check(await page.eval(`window.__discussionInput.value === ${JSON.stringify(expected)}`), `own refresh keeps native ${command} history (CDP editing command)`);
  }
  await page.eval(`window.__discussionInput.focus(); window.__discussionInput.setSelectionRange(2,5)`);
  for (const width of [320,390,834,1440]) for (const theme of ['light','dark']) for (const scale of [100,200]) {
    await page.viewport(width);
    await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%'; document.querySelector('#own-comments').scrollIntoView({block:'start'})`);
    check(await page.eval(`document.documentElement.scrollWidth <= innerWidth+1 && window.__discussionInput === document.querySelector('#root-composer') && document.activeElement === window.__discussionInput && window.__discussionInput.selectionStart === 2 && window.__discussionInput.selectionEnd === 5`), `own fixture ${width}/${theme}/${scale}: reflows and preserves input identity/selection`);
    await page.capture(`discussion-own-${width}-${theme}-${scale}`,{boundary:'intercepted-provider-own-fixture-real-auth-session',textScale:scale});
  }
  own.comments[0].status='approved';
  await page.click('#own-comments-refresh');
  await page.waitFor(`document.querySelector('[data-own-comment="${own.comments[0].comment_id}"]')?.textContent.includes('Đã được duyệt')`, 'off-page approval confirmed by lookup');
  check(await page.eval(`document.querySelectorAll('[data-own-comment="${own.comments[0].comment_id}"]').length === 1`),'labelled fixture pending→approved off-page has one representation');
  own.comments[0].status='removed'; own.comments[0].body=null; own.comments[0].can_remove=false;
  await page.click('#own-comments-refresh');
  await page.waitFor(`document.querySelector('[data-own-comment="${own.comments[0].comment_id}"]')?.textContent.includes('không còn hiển thị')`, 'removed fixture');
  check(await page.eval(`!document.querySelector('[data-own-comment="${own.comments[0].comment_id}"]').textContent.includes(${JSON.stringify(fixture.comments[0].body)})`),'labelled fixture removed strips body and capability');
  await page.capture('discussion-own-removed',{boundary:'intercepted-provider-own-fixture-real-auth-session',textScale:200});
  page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener),1);
  await page.send('Fetch.disable');
  await page.eval(`document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme; delete window.__discussionInput`);
  await page.viewport(1280);
}
