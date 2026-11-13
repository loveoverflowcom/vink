// AUTH-PASSWORD-WEB-01: real Chrome, App, native Identity, separate scratch PostgreSQL.
// The mail carrier is a local outbox. Faults delay/drop a real response or explicitly inject
// a refusal; no synthetic success is used. Don't log request bodies, credentials or cookies.

import { EDITOR_READY, STANDING, account, cookies, fabricate, fill, mailedCode, openAccount, q, signIn, signOut } from './auth-smoke.mjs';

const API = '/api/v1/auth/password';
const PANEL = '#account-session';
const LOGIN = '#password-login';
const REGISTER = '#password-register';
const ENROLL = '#password-enroll';
const sessionCookie = '__Host-vink_session';
const enrollCookie = '__Host-vink_enroll';
const SECRET_A = '  Mùa hạ bên hiên 𐐷  ';
const SECRET_B = 'Những con đường qua phố 𐐷';
const DRAFT_TEXT = 'Dòng đầu có dấu: ỹ ỷ ữ ợ Đ đ.\n\nDòng sau để đọc lại.';
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const contains = (text) => `${q(PANEL)}?.innerText.includes(${JSON.stringify(text)})`;
const bodyText = (draft) => draft.document.blocks.map((block) => block.lines.join('\n')).join('\n\n');

const read = (page, path) => page.eval(`fetch(${JSON.stringify(path)}).then(async r => ({
  status: r.status, cache: r.headers.get('cache-control'), data: r.ok ? await r.json() : null
}))`);

function captureRequest(page, method, path) {
  let requests = 0;
  const listener = (event) => {
    if (event.sessionId !== page.sessionId || event.method !== 'Network.requestWillBeSent') return;
    const { request } = event.params;
    if (request.method === method && new URL(request.url).pathname === path) requests++;
  };
  page.cdp.listeners.push(listener);
  return {
    count: () => requests,
    close: () => { page.cdp.listeners = page.cdp.listeners.filter((item) => item !== listener); },
  };
}

/** Hold the first matching real answer before delivery. Can release it or lose the ACK. */
async function holdAnswer(page, method, path) {
  let held;
  const listener = (event) => {
    if (event.sessionId !== page.sessionId || event.method !== 'Fetch.requestPaused') return;
    const { request, requestId, responseStatusCode } = event.params;
    if (!held && request.method === method && new URL(request.url).pathname === path) {
      held = { requestId, status: responseStatusCode };
    } else {
      page.send('Fetch.continueRequest', { requestId }).catch(() => {});
    }
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: `*${path}`, requestStage: 'Response' }] });
  return {
    async held() {
      for (let i = 0; !held && i < 160; i++) await sleep(50);
      if (!held) throw new Error(`timed out holding ${method} ${path}`);
      return held.status;
    },
    async finish(drop = false) {
      if (drop) await page.send('Fetch.failRequest', { requestId: held.requestId, errorReason: 'ConnectionClosed' });
      else await page.send('Fetch.continueRequest', { requestId: held.requestId });
      page.cdp.listeners = page.cdp.listeners.filter((item) => item !== listener);
      await page.send('Fetch.disable');
    },
  };
}

async function credentials(page, form, username, password) {
  await fill(page, `${form}-username`, username);
  await fill(page, `${form}-password`, password);
}

async function name(page, penName) {
  await page.waitFor(`${STANDING} === 'needs_principal' && ${q('#pen-name')}`, 'pen name');
  await fill(page, '#pen-name', penName);
  await page.click('#pen-name-save');
  await page.waitFor(`${STANDING} === 'active'`, 'active session');
  return (await account(page)).body;
}

async function createDraft(page, title) {
  await page.click('#account-session a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && ${q('#create-draft')}`, 'own writing desk');
  await page.click('#create-draft');
  await page.waitFor(`location.pathname.startsWith('/ban-viet/') && ${EDITOR_READY}`, 'new draft');
  await fill(page, '#draft-title', title);
  await page.typeInBody(DRAFT_TEXT);
  await page.click('#save');
  await page.waitStatus('saved');
  const id = await page.eval("location.pathname.split('/').pop()");
  await page.reload();
  await page.waitFor(`${EDITOR_READY} && ${q('#draft-title')}.value === ${JSON.stringify(title)}`, 'saved title after reload');
  if (await page.body() !== DRAFT_TEXT) throw new Error('saved draft body changed on reload');
  return id;
}

async function matrix(page, scenario, anchor, check) {
  const path = JSON.stringify(anchor);
  for (const width of [320, 390, 1280]) for (const theme of ['light', 'dark']) for (const scale of [100, 200]) {
    await page.viewport(width);
    const measured = await page.eval(`(async () => {
      document.documentElement.dataset.theme = '${theme}';
      document.documentElement.style.fontSize = '${scale}%';
      const panel = ${q(PANEL)}, section = document.querySelector(${path});
      section.scrollIntoView({block: 'start', behavior: 'instant'});
      await document.fonts.ready;
      await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
      const rect = section.getBoundingClientRect();
      const clipped = [...panel.querySelectorAll('input,button,.vk-ui-field__help,.vk-ui-field__error,[role=alert]')]
        .filter(el => !el.classList.contains('vk-ui-visually-hidden'))
        .some(el => {
          const r = el.getBoundingClientRect();
          return r.width > 1 && (r.left < -1 || r.right > document.documentElement.clientWidth + 1 ||
            el.scrollWidth > el.clientWidth + 1 && el.tagName !== 'INPUT');
        });
      const font = [...document.fonts].some(f => f.family.includes('Be Vietnam Pro') && f.status === 'loaded');
      const toRgb = (value) => value.match(/[0-9.]+/g)?.slice(0, 3).map(Number);
      const luminance = (rgb) => rgb.map(c => (c /= 255) <= .04045 ? c/12.92 : ((c+.055)/1.055)**2.4)
        .reduce((sum, c, i) => sum + c * [.2126, .7152, .0722][i], 0);
      const contrasts = [...section.querySelectorAll('label,.vk-account-error,.vk-ui-field__help,.vk-ui-field__error')]
        .filter(el => !el.classList.contains('vk-ui-visually-hidden')).map(el => {
          const text = toRgb(getComputedStyle(el).color);
          let background;
          for (let p = el; p && !background; p = p.parentElement) {
            const value = getComputedStyle(p).backgroundColor;
            if (value !== 'rgba(0, 0, 0, 0)' && value !== 'transparent') background = toRgb(value);
          }
          if (!background) return null;
          const [a,b] = [luminance(text), luminance(background)].sort((x,y)=>y-x);
          return (a+.05)/(b+.05);
        }).filter(r => r !== null);
      return { minContrast: contrasts.length ? Math.min(...contrasts) : null, ok: !!rect.width && !clipped && document.documentElement.scrollWidth <= innerWidth + 1 && font,
        width: rect.width, left: rect.left, font, device: devicePixelRatio,
        focus: document.activeElement?.id,
        partial: rect.bottom > (document.querySelector('.vk-ui-bottom-nav')?.getBoundingClientRect().top || innerHeight) };
    })()`);
    check(measured.ok, `${scenario} at ${width}px / ${theme} / text ${scale}%: no horizontal clipping, Be Vietnam Pro loaded (${JSON.stringify(measured)})`);
    if (measured.minContrast !== null) check(measured.minContrast >= 4.5, `${scenario}: measured label/help/error contrast ≥ 4.5:1`);
    if ((width === 390 && scale === 100) || (width === 320 && scale === 200 && theme === 'dark') ||
        (width === 1280 && scale === 100 && theme === 'light')) {
      await page.capture(`auth-password-${scenario}-${width}-${theme}-${scale}`, {
        boundary: 'native Identity + App + scratch PostgreSQL',
        scenario, textScale: scale, font: 'Be Vietnam Pro', ...measured,
      });
      if (measured.partial) {
        await page.eval(`(() => {
          const section = document.querySelector(${path}), bar = document.querySelector('.vk-ui-bottom-nav');
          section.scrollIntoView({block:'end',behavior:'instant'});
          const bottom = bar?.getBoundingClientRect().top || innerHeight;
          scrollBy({top: Math.max(0, section.getBoundingClientRect().bottom - bottom + 12), behavior:'instant'});
        })()`);
        await page.capture(`auth-password-${scenario}-${width}-${theme}-${scale}-end`, {
          boundary: 'native Identity + App + scratch PostgreSQL', scenario, textScale: scale, font: 'Be Vietnam Pro',
        });
      }
    }
  }
  await page.eval("document.documentElement.style.fontSize = ''; delete document.documentElement.dataset.theme");
  await page.viewport(1280);
}

export async function passwordAuth(page, { args, check, openPage }) {
  const { base } = args;
  const ctx = { args, seen: new Set() };
  const userA = 'banviet.a';
  const userB = 'banviet.b';
  const penA = 'An Nhiên';
  const penB = 'Người giữ mùa thu';
  const email = 'taikhoan-email@vi-du.vn';

  console.log('Auth password: registration, browser input, server cookie and own draft:');
  await openAccount(page, base, 'anonymous');
  check(await page.eval(`${q(LOGIN)} && !${q(REGISTER)} && !${q('#sign-in-email')}`), 'password is the default guest form');
  check(await page.eval(`(() => { const p=${q(LOGIN + '-password')}; return p.type === 'password' && p.name === 'password' && p.autocomplete === 'current-password' &&
    !p.hasAttribute('maxlength') && p.getAttribute('aria-required') === 'true' && ${q(LOGIN + '-username')}.autocomplete === 'username'; })()`), 'native login field names and autocomplete, with no raw maxlength');

  await page.eval(`${q('#use-registration')}.focus()`);
  await page.key('Enter');
  await page.waitFor(`document.activeElement?.id === 'password-register-username'`, 'explicit mode switch focuses first field');
  check(await page.eval(`!${q(LOGIN)} && ${q(REGISTER + '-password')}.autocomplete === 'new-password'`), 'registration asks for username and password without email');
  await matrix(page, 'registration', REGISTER, check);
  // DOM autofill simulation: set the actual values, without input events.
  await page.eval(`(() => {
    ${q(REGISTER + '-username')}.value = ${JSON.stringify(userA.toUpperCase())};
    const p = window.savedInput = ${q(REGISTER + '-password')};
    p.value = ${JSON.stringify(SECRET_A.normalize('NFD'))};
    p.focus(); p.setSelectionRange(2, 7, 'backward');
  })()`);
  const registration = captureRequest(page, 'POST', `${API}/register`);
  await page.click(REGISTER + '-password-reveal');
  check(await page.eval(`(() => {
    const p = ${q(REGISTER + '-password')};
    return p === window.savedInput && p.type === 'text' && p.selectionStart === 2 &&
      p.selectionEnd === 7 && p.selectionDirection === 'backward' && document.activeElement === p &&
      ${q(REGISTER + '-password-reveal')}.getAttribute('aria-pressed') === 'true';
  })()`), 'pointer reveal keeps the same input, focus and selection');
  await page.key('Tab');
  await page.key('Space');
  check(registration.count() === 0 && await page.eval(`
    ${q(REGISTER + '-password')}.type === 'password' && document.activeElement?.id === 'password-register-password-reveal'
  `), 'keyboard conceal keeps focus on the toggle and sends no mutation');
  await page.eval(`delete window.savedInput;
    ${q(REGISTER + '-username')}.dispatchEvent(new CompositionEvent('compositionstart', {bubbles: true}));
    ${q(REGISTER + '-username')}.focus()`);
  await page.key('Enter');
  check(registration.count() === 0, 'Enter during simulated composition sends nothing');
  await page.eval(`${q(REGISTER + '-username')}.dispatchEvent(new CompositionEvent('compositionend', {bubbles: true}))`);

  await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
  const heldRegistration = await holdAnswer(page, 'POST', `${API}/register`);
  await page.key('Enter');
  check(await heldRegistration.held() === 200, 'registration commits through the App with DOM autofill and Unicode NFD');
  await page.waitFor(`${q(REGISTER + '-submit')}.getAttribute('aria-busy') === 'true'`, 'registration pending');
  const motion = await page.eval(`(() => {
    const arc = ${q(REGISTER + '-submit .vk-ui-arc svg')};
    if (!arc) return { found: false };
    const style = getComputedStyle(arc);
    return { found: true, iterations: style.animationIterationCount, duration: style.animationDuration,
      animation: style.animationName, preference: matchMedia('(prefers-reduced-motion: reduce)').matches };
  })()`);
  check(motion.preference && motion.found &&
    (Number(motion.iterations) === 0 || Number(motion.iterations) * parseFloat(motion.duration) < .001 || motion.animation === 'none'),
    `pending control keeps an accessible busy state and a static arc in reduced motion (${JSON.stringify(motion)})`);
  await page.click(REGISTER + '-submit');
  check(registration.count() === 1, 'Enter then click while the real response is held sends one registration');
  await heldRegistration.finish();
  registration.close();
  await page.send('Emulation.setEmulatedMedia', { features: [] });
  await page.waitFor(`${STANDING} === 'needs_principal'`, 'registration needs pen name');
  check(await page.eval(`!${q(REGISTER + '-password')} && !${q('#password-credential')}`), 'registration disposes the secret; unbound is not active');
  await matrix(page, 'pen-name', '#pen-name-form', check);
  const A = await name(page, penA);
  check(A.display_name === penA && A.display_name !== userA, 'the server confirms a separate pen name');
  const draftA = await createDraft(page, '[Mẫu] Nháp tạo sau đăng ký');
  await openAccount(page, base, 'active');
  await page.click('#password-credential');
  await page.waitFor(contains('đã có thông tin đăng nhập'), 'credential already present');
  check(await page.eval(`!${q(ENROLL + '-send')}`), 'already-enrolled account has no second enrollment form');

  console.log('Auth password: generic invalid credentials, exact logout and late cookie readback:');
  const refusal = await fabricate(page, 'DELETE', '/api/v1/auth/session', 503, { error: { code: 'dependency_unavailable', message: 'fixture' } });
  await page.click('#sign-out');
  await refusal();
  await page.waitFor(q('#sign-out-unconfirmed'), 'unconfirmed logout');
  check((await account(page)).body.principal_id === A.principal_id, 'fault 503 logout: account still valid and displayed');
  check((await signOut(page, base)).includes(204), 'real 204 confirms logout');
  await credentials(page, LOGIN, userA, SECRET_A.trim());
  const readbacks = captureRequest(page, 'GET', '/api/v1/auth/session');
  await page.click(LOGIN + '-submit');
  await page.waitFor(contains('Tên đăng nhập hoặc mật khẩu chưa đúng'), 'invalid credentials');
  check(readbacks.count() === 0 && await page.eval(`!${contains('Phiên đăng nhập đã kết thúc')}`), 'wrong password has generic form error, no session refresh, spaces were significant');
  readbacks.close();
  await page.eval(`${q(LOGIN + '-username')}.value = ''; ${q(LOGIN + '-password')}.value = ''`);
  await matrix(page, 'invalid-credentials', LOGIN, check);
  const throttled = await fabricate(page, 'POST', `${API}/login`, 429,
    { error: { code: 'rate_limited', message: 'fixture' } }, [{ name: 'Retry-After', value: '8' }]);
  await page.click(LOGIN + '-submit');
  await throttled();
  await page.waitFor(`${q(LOGIN + '-submit-note')}?.innerText.includes('Có thể thử lại sau')`, 'password retry-after feedback');
  check(await page.eval(`${q(LOGIN + '-submit')}.getAttribute('aria-disabled') === 'true'`), '429 disables the password action during Retry-After');
  await matrix(page, 'cooldown', LOGIN, check);
  await page.waitFor(`${q(LOGIN + '-submit')}.getAttribute('aria-disabled') !== 'true'`, 'password cooldown elapsed');
  // A stale form may not update the replacement form; the late Set-Cookie still has to be read.
  await credentials(page, LOGIN, userA, SECRET_A);
  const late = await holdAnswer(page, 'POST', `${API}/login`);
  await page.click(LOGIN + '-submit');
  check(await late.held() === 200, 'real NFC login matches NFD registration');
  await page.click('#use-email-code');
  await page.waitFor(q('#sign-in-address'), 'switch to email while password login is in flight');
  check(await page.eval(`!${q(LOGIN + '-password')} && ${q('#sign-in-send')}.getAttribute('aria-disabled') === 'true'`),
    'switch removes the password and keeps the single auth lane occupied');
  await late.finish();
  await page.waitFor(`${STANDING} === 'active' && !${q('#sign-in-email')}`, 'late real cookie observed');
  const returnedA = (await account(page)).body;
  check(returnedA.principal_id === A.principal_id && returnedA.display_name === penA &&
    bodyText(await page.draft(draftA)) === DRAFT_TEXT, 'password returns to the same principal, pen name and stored draft');

  console.log('Auth password: email account adds a credential without changing its session:');
  const { browserContextId } = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
  const other = await openPage(browserContextId);
  await openAccount(other, base, 'anonymous');
  await signIn(other, ctx, email);
  const B = await name(other, penB);
  const draftB = await createDraft(other, '[Mẫu] Nháp có từ trước khi thêm mật khẩu');
  await openAccount(other, base, 'active');
  await other.click('#password-credential');
  await other.waitFor(q(ENROLL + '-send'), 'unenrolled status read from server');
  check(await other.eval(`!${q(ENROLL + '-code')} && !${q('input[name=email]')}`), 'proof is explicitly requested for the verified address on this account');
  const cookieBefore = (await cookies(other, base))[sessionCookie];
  await other.click(ENROLL + '-send');
  await other.waitFor(q(ENROLL + '-code'), 'empty-body request accepted, enrollment code step');
  const proof = (await cookies(other, base))[enrollCookie];
  check(proof?.httpOnly && proof.secure && proof.sameSite === 'Strict', 'server owns the secure HttpOnly proof cookie');
  const otp = await mailedCode(args.outbox, email, ctx.seen);
  await matrix(other, 'enrollment', ENROLL, check);
  await other.viewport(320);
  await other.send('Page.bringToFront');
  await other.eval("document.documentElement.style.fontSize = '200%'");
  await other.waitFor(`(() => {
    const nav = document.querySelector('.vk-ui-bottom-nav');
    return nav.getBoundingClientRect().height > 0 &&
      Math.abs(parseFloat(getComputedStyle(nav.closest('.vk-ui-scaffold')).getPropertyValue('--vk-ui-bar-height')) - nav.getBoundingClientRect().height) < 1;
  })()`, 'mobile navigation clearance measured');
  await other.eval(`${q(ENROLL + '-password-reveal')}.focus()`);
  await other.key('Tab');
  const keyboard = await other.eval(`(() => {
    const submit = ${q(ENROLL + '-submit')}, r = submit.getBoundingClientRect();
    const bar = document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect();
    const hit = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
    return { focused: document.activeElement?.id, bottom: r.bottom, barTop: bar.top, hit: submit.contains(hit) };
  })()`);
  check(keyboard.focused === 'password-enroll-submit' && keyboard.bottom <= keyboard.barTop + 1 && keyboard.hit,
    `keyboard reaches the enrollment action above bottom navigation at 320px / text 200% (${JSON.stringify(keyboard)})`);
  await other.eval("document.documentElement.style.fontSize = ''");
  await other.viewport(1280);
  const wrong = String((Number(otp) + 1) % 1000000).padStart(6, '0');
  await fill(other, ENROLL + '-code', wrong);
  await credentials(other, ENROLL, userA, SECRET_B);
  await other.click(ENROLL + '-submit');
  await other.waitFor(contains('Mã chưa đúng'), 'wrong enrollment OTP');
  await fill(other, ENROLL + '-code', otp);
  await other.click(ENROLL + '-submit');
  await other.waitFor(contains('Tên đăng nhập này đã được dùng'), 'real username collision');
  check((await cookies(other, base))[enrollCookie]?.value === proof.value &&
    await other.eval(`${q(ENROLL + '-code')}.value.length === 6`), 'username collision preserves the proof and typed code for another username');
  await fill(other, ENROLL + '-username', userB);
  const enrollment = captureRequest(other, 'POST', `${API}/enroll`);
  const lostEnroll = await holdAnswer(other, 'POST', `${API}/enroll`);
  await other.click(ENROLL + '-submit');
  check(await lostEnroll.held() === 200, 'real server committed the enrollment before the ACK is dropped');
  await lostEnroll.finish(true);
  await other.waitFor(contains('đã có thông tin đăng nhập'), 'enrolled status read after lost ACK');
  check(enrollment.count() === 1 && (await cookies(other, base))[sessionCookie]?.value === cookieBefore.value &&
    (await account(other)).body.principal_id === B.principal_id &&
    await other.eval(`!${q(ENROLL + '-password')} && !${contains(userB)}`),
    'readback reports only the credential presence; no replay, no second session and no remaining secrets');
  enrollment.close();
  const status = await read(other, `${API}/enroll`);
  check(status.data.enrolled && status.cache?.includes('no-store'), 'credential status is private and no-store');
  check((await signOut(other, base)).includes(204), 'email account signs out before password login');
  await credentials(other, LOGIN, userB, SECRET_B);
  await other.click(LOGIN + '-submit');
  await other.waitFor(`${STANDING} === 'active'`, 'password sign-in for enrolled account');
  const returnedB = (await account(other)).body;
  check(returnedB.principal_id === B.principal_id && returnedB.display_name === penB &&
    bodyText(await other.draft(draftB)) === DRAFT_TEXT &&
    (await read(other, `/api/v1/drafts/${draftA}`)).status === 404,
    'password reads the prior draft of the email account; another principal’s draft remains inaccessible');

  console.log('Auth password: tab switch invalidates a pending credential status and prior form:');
  const tab = await openPage();
  await openAccount(tab, base, 'active');
  const staleStatus = await holdAnswer(tab, 'GET', `${API}/enroll`);
  await tab.click('#password-credential');
  check(await staleStatus.held() === 200, 'A’s actual credential-status answer held in another tab');
  await signOut(page, base);
  await tab.waitFor(`${STANDING} === 'anonymous' && ${q(LOGIN)}`, 'tab sees confirmed logout');
  await credentials(tab, LOGIN, 'old-mode', 'temporary browser fixture');
  await credentials(page, LOGIN, userA, SECRET_A);
  await page.click(LOGIN + '-submit');
  await tab.waitFor(`${STANDING} === 'active' && !${q(LOGIN + '-password')}`, 'same principal, fresh cookie, prior login form gone');
  await page.waitFor(`${STANDING} === 'active' && ${q('#sign-out')}`, 'signing-in tab has confirmed the account');
  await staleStatus.finish();
  check(await tab.eval(`!${q('#password-credential-status')} && ${q('#password-credential')}.getAttribute('aria-expanded') === 'false'`),
    'late status cannot reopen an earlier enrollment, even when the principal returns');
  check((await account(tab)).body.principal_id === A.principal_id, 'server confirms the same principal after the cross-tab round trip');

  console.log('Auth password: a lost registration ACK does not imply the account was not created:');
  await signOut(page, base);
  const lostLogin = await holdAnswer(page, 'POST', `${API}/login`);
  const loginLoss = captureRequest(page, 'POST', `${API}/login`);
  await credentials(page, LOGIN, userA, SECRET_A);
  await page.click(LOGIN + '-submit');
  check(await lostLogin.held() === 200, 'the server issued a session before the login ACK is lost');
  await lostLogin.finish(true);
  await page.waitFor(`${STANDING} === 'active' || (${q('#password-login-retry')} && ${q('#password-login-retry')}.getAttribute('aria-disabled') !== 'true')`, 'login readback finished');
  const actual = (await read(page, '/api/v1/auth/session')).data;
  check(loginLoss.count() === 1 && (actual.state === 'active' && actual.principal_id === A.principal_id ||
    actual.state === 'anonymous' && await page.eval(`${q('#password-login-retry')}.getAttribute('aria-disabled') !== 'true'`)),
    'lost login ACK follows the actual cookie and never replays the mutation');
  loginLoss.close();
  if (actual.state === 'active') await signOut(page, base);
  await page.click('#use-registration');
  await credentials(page, REGISTER, userA.toUpperCase(), SECRET_A);
  await page.click(REGISTER + '-submit');
  await page.waitFor(`${q(REGISTER + '-username')}.getAttribute('aria-invalid') === 'true' && ${contains('Tên đăng nhập này đã được dùng')}`, 'registration collision is a field error');
  check(await page.eval(`${q(REGISTER + '-password')}.value.length > 0`), 'a correctable registration collision keeps the typed password');
  await credentials(page, REGISTER, 'banviet.lost', '𐐷'.repeat(1025));
  await page.click(REGISTER + '-submit');
  await page.waitFor(`${q(REGISTER + '-password')}.getAttribute('aria-invalid') === 'true'`, 'server raw limit on registration');
  check(await page.eval(`[...${q(REGISTER + '-password')}.value].length === 1025`),
    'browser preserves the oversized raw value for the server to refuse, with no truncation');
  await credentials(page, REGISTER, 'banviet.lost', SECRET_A);
  const loss = captureRequest(page, 'POST', `${API}/register`);
  const lostRegister = await holdAnswer(page, 'POST', `${API}/register`);
  await page.click(REGISTER + '-submit');
  check(await lostRegister.held() === 200, 'registration reached a real successful commit');
  // On some Chrome versions Set-Cookie is already installed at the CDP response stage.
  // Explicitly remove just the test cookie to model "commit succeeded, browser got no cookie".
  await lostRegister.finish(true);
  await page.send('Network.deleteCookies', { name: sessionCookie, url: base });
  await page.waitFor(contains('Tài khoản có thể đã được tạo'), 'uncertain registration guidance');
  await page.waitFor(`${q('#password-check-session')}.getAttribute('aria-busy') !== 'true'`, 'session readback settled');
  const readback = await read(page, '/api/v1/auth/session');
  check(readback.data.state === 'anonymous', 'lost ACK without cookie: readback still says guest');
  await page.click('#password-check-session');
  await page.waitFor(`${q('#password-check-session')}.getAttribute('aria-busy') !== 'true'`, 'explicit readback settled');
  check(loss.count() === 1 && await page.eval(`${q(REGISTER + '-submit')}.getAttribute('aria-disabled') === 'true'`),
    'no automatic or accidental registration replay after the uncertain commit');
  loss.close();
  await page.click('#use-password');
  await page.waitFor(q(LOGIN), 'back to login');
  check(await page.eval(`${q(LOGIN + '-password')}.value === '' && ${q(LOGIN + '-password')}.type === 'password'`),
    'switching flow forgets the password and starts hidden');
  await credentials(page, LOGIN, 'banviet.lost', SECRET_A);
  await page.click(LOGIN + '-submit');
  await page.waitFor(`${STANDING} === 'needs_principal'`, 'explicit login recovers committed registration');
  check((await read(page, '/api/v1/auth/session')).data.principal_id === undefined, 'unbound session never becomes an active principal');
  check(await page.eval(`(() => {
    const store = JSON.stringify([Object.entries(localStorage), Object.entries(sessionStorage)]);
    return !store.includes(${JSON.stringify(SECRET_A)}) && !store.includes('banviet.lost') &&
      !document.cookie.includes('vink_s') && !location.search && !location.hash;
  })()`), 'no test credentials in script-visible cookies, storage or the URL');
  check(other.errors.length === 0 && tab.errors.length === 0, 'no console exceptions in the extra password tabs');
}
