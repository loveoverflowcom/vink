// AUTH-PASSWORD-WEB-01: real Chrome, App, native Identity, separate scratch PostgreSQL.
// The mail carrier is a local outbox. Faults delay/drop a real response or explicitly inject
// a refusal; no synthetic success is used. Don't log request bodies, credentials or cookies.

import { checkLoginR3 } from './auth-r3-smoke.mjs';
import { accountDeletionFixtureSql } from './author-fixtures.mjs';
import { randomUUID } from 'node:crypto';
import { wakeDesk } from './desk-lifecycle.mjs';
import { followingFeed } from './following-feed-smoke.mjs';
import { authorFollows } from './author-follow-smoke.mjs';
import { authorHandles } from './author-handle-smoke.mjs';
import { authorProfiles } from './author-profile-smoke.mjs';
import { EDITOR_READY, HYDRATED, STANDING, account, cookies, fabricate, fill, mailedCode, openAccount, q, signIn, signOut } from './auth-smoke.mjs';
import { deskGuest } from './desk-empty-smoke.mjs';
import { captureRequest, holdAnswer } from './auth-browser-network.mjs';
import { APPEARANCE_SETTLED } from './visual-evidence.mjs';
import { standaloneWorkViewCheck } from './work-views-smoke.mjs';
import { readerSave } from './saved-works-smoke.mjs';

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
const bodyText = (draft) => draft.document.source;

const read = (page, path) => page.eval(`fetch(${JSON.stringify(path)}).then(async r => ({
  status: r.status, cache: r.headers.get('cache-control'), data: r.ok ? await r.json() : null
}))`);

async function credentials(page, form, username, password) {
  await fill(page, `${form}-username`, username);
  await fill(page, `${form}-password`, password);
}

async function setPenName(page, penName, { returnToDesk = false } = {}) {
  await page.waitFor(`${STANDING} === 'needs_principal' && ${q('#pen-name')}`, 'pen name');
  await fill(page, '#pen-name', penName);
  await page.click('#pen-name-save');
  if (returnToDesk) {
    await page.waitFor(`location.pathname === '/ban-viet' && ${q('#create-draft')}`, 'active session returns to Bàn viết');
  } else {
    await page.waitFor(`${STANDING} === 'active'`, 'active session on Tôi');
  }
  return (await account(page)).body;
}

async function createDraft(page, title) {
  if (await page.eval("location.pathname !== '/ban-viet'")) {
    await page.click('#navigation-primary a[href="/ban-viet"]');
  }
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

async function attachVerifiedEmail(page, { args, check, accountBefore, draft, seen }) {
  const path = '/api/v1/auth/email/attachment';
  const email = 'nhap-da-luu@vi-du.vn';
  const field = (suffix) => `#email-attach-${suffix}`;
  const cookie = (await cookies(page, args.base))[sessionCookie];
  await page.waitFor(q(field('missing')), '#329 password account has no verified email');
  const initial = await read(page, path);
  check(initial.status === 200 && initial.data.password_account && initial.data.verified_email === null &&
    initial.data.principal_id === accountBefore.principal_id && initial.cache?.includes('no-store'),
    '#329 private initial binding read is missing and identifies the current principal');

  await page.click(field('add'));
  await page.waitFor(`document.activeElement?.id === 'email-attach-address'`, '#329 explicit open focuses email');
  await page.eval(`${q(field('cancel'))}.focus()`);
  await page.key('Enter');
  await page.waitFor(`document.activeElement?.id === 'email-attach-add'`, '#329 keyboard cancel returns focus to Add');
  await page.click(field('add'));
  await page.waitFor(q(field('address')), '#329 attachment form reopens');
  await page.viewport(320);
  await page.eval("document.documentElement.style.fontSize = '200%'");
  check(await page.eval(`(async () => {
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const viewport = document.documentElement.clientWidth;
    const section = ${q('#recovery-email-settings')};
    return document.documentElement.scrollWidth <= viewport + 1 &&
      Array.from(section.querySelectorAll('input, button')).every(el => {
        const r = el.getBoundingClientRect();
        return r.width > 0 && r.left >= -1 && r.right <= viewport + 1;
      });
  })()`), '#329 email form has no horizontal overflow at 320px / 200% root text');
  await page.eval("document.documentElement.style.fontSize = ''");
  await page.viewport(1280);
  await fill(page, field('address'), email.toUpperCase());
  await fill(page, field('password'), 'wrong current password');
  await page.click(field('send'));
  await page.waitFor(contains('Hãy nhập đúng mật khẩu hiện tại'), '#329 current password verified on server');
  check(await page.eval(`!${q(field('code'))} && ${q(field('password'))}.value === ''`),
    '#329 bad current password does not advance and the password is cleared');
  await fill(page, field('password'), SECRET_A);
  const pending = await holdAnswer(page, 'POST', path + '/challenges');
  await page.click(field('send'));
  check(await pending.held() === 202, '#329 holds a real issued challenge before delivery');
  await page.eval(`${q('#password-credential')}.focus()`);
  await pending.finish(false, true);
  await page.waitFor(q(field('code')), '#329 code is available after the delayed response');
  check(await page.eval(`document.activeElement === ${q('#password-credential')}`),
    '#329 receipt does not steal focus back from another account setting');
  await mailedCode(args.outbox, email, seen);
  await page.click(field('new-code'));
  await page.waitFor(q(field('add')), '#329 start again reads the still-missing binding');
  await page.click(field('add'));
  await fill(page, field('address'), email);
  await fill(page, field('password'), SECRET_A);
  await page.click(field('send'));
  await page.waitFor(`document.activeElement?.id === 'email-attach-code'`, '#329 accepted request focuses code when the form still owns focus');
  const handle = (await cookies(page, args.base))['__Host-vink_attach'];
  check(handle?.httpOnly && handle.secure && handle.sameSite === 'Strict' &&
    await page.eval(`!${q(field('password'))} && ${q(field('code'))}.autocomplete === 'one-time-code'`),
    '#329 secure strict Handle is browser-owned, password is removed, and code supports autofill');
  const otp = await mailedCode(args.outbox, email, seen);
  const wrong = String((Number(otp) + 1) % 1000000).padStart(6, '0');
  await fill(page, field('code'), wrong);
  await page.click(field('confirm'));
  await page.waitFor(contains('Mã chưa đúng'), '#329 wrong code rejected without advancing');
  check((await read(page, path)).data.verified_email === null, '#329 wrong code did not write the binding');

  await fill(page, field('code'), otp);
  const writes = captureRequest(page, 'POST', `${path}/complete`);
  const reads = captureRequest(page, 'GET', path);
  const lost = await holdAnswer(page, 'POST', `${path}/complete`);
  await page.eval(`${q(field('confirm'))}.focus()`);
  await page.key('Enter');
  check(await lost.held() === 204, '#329 real server committed before the ACK was dropped');
  check(await page.eval(`!${q(field('verified'))}`), '#329 no optimistic verified result');
  await lost.finish(true);
  await page.waitFor(`${q(field('verified'))}?.textContent.includes(${JSON.stringify(email)})`, '#329 lost ACK reconciled from real binding');
  await page.waitFor(`document.activeElement?.id === 'email-attach-verified'`, '#329 keyboard focus follows confirmed readback');
  check(writes.count() === 1 && reads.count() === 1, '#329 uncertain completion reads once and never replays');
  writes.close();
  reads.close();
  const bound = await read(page, path);
  check(bound.data.verified_email === email && bound.data.session_id === initial.data.session_id &&
    (await cookies(page, args.base))[sessionCookie]?.value === cookie.value &&
    (await account(page)).body.principal_id === accountBefore.principal_id &&
    (await account(page)).body.display_name === accountBefore.display_name &&
    bodyText(await page.draft(draft)) === DRAFT_TEXT &&
    await page.eval(`!${q(field('code'))} && !${q(field('add'))}`),
    '#329 canonical email binds to the same session, principal, pen name and existing draft; no second attach control');
  return email;
}

async function expiredDesk(page, {base, check, openPage}) {
  const cookie = (await cookies(page, base))[sessionCookie];
  const { browserContextId } = await page.cdp.send('Target.createBrowserContext');
  try {
    const desk = await openPage(browserContextId);
    await desk.send('Network.setCookie', {name:cookie.name, value:cookie.value, url:base,
      path:'/', secure:true, httpOnly:true, sameSite:cookie.sameSite});
    await openAccount(desk, base, 'active');
    await chooseAppearance(desk, 'theme', 'dark');
    await desk.waitFor("document.documentElement.dataset.theme === 'dark'", 'active Desk fixture saves a member theme');
    const reads = captureRequest(desk, 'GET', '/api/v1/author/works');
    await desk.goto(base+'/ban-viet');
    await desk.waitFor(q('[data-testid=draft-row]'), '#308 active scope loads its own draft');
    await desk.waitFor("document.documentElement.dataset.theme === 'dark'", 'the scoped member theme reaches Desk');
    check(reads.count()===1, '#308 active scope starts exactly one initial read');
    const late = await holdAnswer(desk, 'GET', '/api/v1/author/works');
    await desk.send('Network.deleteCookies', {name:sessionCookie, url:base});
    await wakeDesk(desk);
    check(await late.held()===401, '#308 holding the real 401 from a private refresh after session loss');
    await late.finish();
    await desk.waitFor(q('#desk-sign-in'), '#308 late 401 reconciles through the session owner');
    await expectDefaultAppearance(desk, check, 'private 401 after the browser loses its session cookie');
    check(reads.count()===2 && await desk.eval("!document.querySelector('#draft-search, #retry-drafts, [data-testid=draft-row]')"),
      '#308 previous rows and controls are gone with no further list read');
    reads.close();
  } finally {
    await page.cdp.send('Target.disposeBrowserContext', {browserContextId});
  }
}

async function chooseAppearance(page, group, value) {
  const selected = `document.querySelector('#page-${group} input:checked')?.value === '${value}'`;
  if (!await page.eval(selected)) await page.click(`#page-${group} label:has(input[value="${value}"])`);
  await page.waitFor(selected, `Appearance ${group} applies ${value}`);
}

async function matrix(page, scenario, anchor, check) {
  const path = JSON.stringify(anchor);
  const actions = ({
    login: ['#password-login-submit', '#use-email-code', '#use-registration'],
    registration: ['#password-register-submit', '#use-password', '#use-email-code'],
    email: ['#sign-in-send', '#use-password'],
    'pen-name': ['#pen-name-save'],
    closed: [],
  })[scenario];
  const guest = actions !== undefined;
  const boundary = scenario === 'closed' ? 'App (sign-in disabled) + scratch PostgreSQL' :
    'native Identity + App + scratch PostgreSQL';
  const widths = guest ? [320, 390, 834, 1440] : [320, 390, 1280];
  for (const width of widths) for (const theme of ['light', 'dark']) for (const scale of [100, 200]) {
    await page.viewport(width, width === 834 ? 600 : 900);
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: theme }] });
    await page.eval("document.documentElement.style.fontSize = ''");
    if (guest) await expectDefaultAppearance(page, check, scenario);
    await page.waitFor(APPEARANCE_SETTLED, 'Auth appearance colours settle');
    const endTarget = actions?.at(-1) ?? anchor;
    const measured = await page.eval(`(async () => {
      if (${scale} === 200) document.documentElement.style.fontSize = '200%';
      const panel = ${q(PANEL)}, section = document.querySelector(${path});
      if (${guest}) scrollTo({top: 0, behavior: 'instant'});
      else section.scrollIntoView({block: 'start', behavior: 'instant'});
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
      const contrast = el => {
        const text = toRgb(getComputedStyle(el).color);
        let background;
        for (let p = el; p && !background; p = p.parentElement) {
          const value = getComputedStyle(p).backgroundColor;
          if (value !== 'rgba(0, 0, 0, 0)' && value !== 'transparent') background = toRgb(value);
        }
        if (!background) return null;
        const [a,b] = [luminance(text), luminance(background)].sort((x,y)=>y-x);
        return (a+.05)/(b+.05);
      };
      const contrasts = [...section.querySelectorAll('label,.vk-account-error,.vk-ui-field__help,.vk-ui-field__error')]
        .filter(el => !el.classList.contains('vk-ui-visually-hidden')).map(contrast).filter(r => r !== null);
      const entry = document.querySelector('.vk-auth-entry');
      const logo = entry?.querySelector('.vk-brand__lockup');
      const logoAsset = logo ? getComputedStyle(logo).backgroundImage : '';
      const target = document.querySelector('${endTarget}');
      const bottomNav = document.querySelector('#bottom-navigation');
      const barTop = bottomNav?.getClientRects().length ? bottomNav.getBoundingClientRect().top : innerHeight;
      const fields = [...section.querySelectorAll('input:not([type=hidden])')].map(el => el.getBoundingClientRect());
      const singleColumn = fields.every((r, i) => i === 0 ||
        Math.abs(r.left - fields[0].left) <= 1 && r.top >= fields[i-1].bottom);
      const ordered = ${JSON.stringify(actions ?? [])}.map(sel => document.querySelector(sel)?.getBoundingClientRect())
        .every((r, i, rects) => r && r.width > 0 && r.height > 0 && (!i || r.top >= rects[i-1].bottom - 1));
      return { logoAsset, logoVisible: Boolean(logo?.checkVisibility()), singleColumn, ordered,
        minContrast: contrasts.length ? Math.min(...contrasts) : null, ok: !!rect.width && !clipped && document.documentElement.scrollWidth <= innerWidth + 1 && font,
        width: rect.width, left: rect.left, font, device: devicePixelRatio,
        rootSize: getComputedStyle(document.documentElement).fontSize,
        focus: document.activeElement?.id,
        partial: (target?.getBoundingClientRect().bottom ?? rect.bottom) > barTop };
    })()`);
    check(measured.ok, `${scenario} at ${width}px / ${theme} / text ${scale}%: no horizontal clipping, Be Vietnam Pro loaded (${JSON.stringify(measured)})`);
    if (measured.minContrast !== null) check(measured.minContrast >= 4.5, `${scenario}: measured label/help/error contrast ≥ 4.5:1`);
    if (guest) check(measured.logoVisible && measured.logoAsset.includes(`/brand/vpen-r1-lockup-${theme}.svg`) &&
      measured.singleColumn && measured.ordered, `#327 ${scenario} ${width}/${theme}/${scale}: theme asset and one-column reading order`);
    check(measured.rootSize === `${16 * scale / 100}px`, 'Auth uses the expected root text size');
    const loginR3 = scenario === 'login' ? await checkLoginR3(page, check, `/toi ${width}/${theme}/${scale}`) : undefined;
    const textScaleLane = scale === 200 ? 'dom-override-geometry-only' : 'default-100';
    if (scenario === 'login' || (width === 390 && scale === 100) || (width === 320 && scale === 200 && theme === 'dark') ||
        (scenario === 'email' && width === 1440 && scale === 100) || (width === 1280 && scale === 100 && theme === 'light')) {
      await page.capture(`auth-password-${scenario}-${width}-${theme}-${scale}`, {
        boundary,
        requiredTargets: measured.partial ? [] : [anchor],
        scenario, textScale: scale, textScaleLane, font: 'Be Vietnam Pro', ...measured, loginR3,
      });
      if (measured.partial) {
        await page.eval(`(() => {
          const end = document.querySelector('${endTarget}');
          end.scrollIntoView({block:'center',behavior:'instant'});
        })()`);
        await page.capture(`auth-password-${scenario}-${width}-${theme}-${scale}-end`, {
          boundary, scenario, textScale: scale, textScaleLane, font: 'Be Vietnam Pro',
        });
      }
    }
    if (guest && width <= 390 && scale === 200) {
      check(await page.eval(`(() => {
        const nav = document.querySelector('#bottom-navigation'), barTop = nav?.getClientRects().length ? nav.getBoundingClientRect().top : innerHeight;
        const targets = ${JSON.stringify(actions)};
        return targets.every(sel => {
          const action = document.querySelector(sel);
          action.scrollIntoView({block:'center',behavior:'instant'});
          const r = action.getBoundingClientRect();
          return r.top >= 0 && r.bottom <= barTop + 1 &&
            action.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2));
        });
      })()`), `#576 ${scenario} actions remain reachable above bottom navigation: ${width}/${theme}/${scale}`);
    }
  }
  await page.eval("document.documentElement.style.fontSize = ''");
  await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
  await page.viewport(1280);
  await page.eval("scrollTo({top:0,behavior:'instant'})");
}

const MISSING_APPEARANCE = `!${q('#page-appearance, #auth-appearance, a[href="#page-appearance"]')}`;
const ROOT_DEFAULTS = `['theme', 'text', 'rail'].every(name => !document.documentElement.dataset[name])
  && getComputedStyle(document.documentElement).fontSize === '16px'`;
const DARK_LARGE_COMPACT = `document.documentElement.dataset.theme === 'dark' &&
  document.documentElement.dataset.text === '150' && document.documentElement.dataset.rail === 'compact'`;

async function expectDefaultAppearance(page, check, state) {
  await page.waitFor(`${MISSING_APPEARANCE} && ${ROOT_DEFAULTS}`, `${state}: system defaults and no appearance controls`);
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  check(!nodes.some(node => !node.ignored && ['Giao diện', 'Chế độ màu', 'Cỡ chữ', 'Thanh điều hướng'].includes(node.name?.value)),
    `${state}: the accessibility tree has no appearance actions or settings`);
}

async function seedLegacyAppearance(page) {
  await page.eval(`localStorage.setItem('vink.ui.theme', 'dark');
    localStorage.setItem('vink.ui.text', '200'); localStorage.setItem('vink.ui.rail', 'compact')`);
}

async function firstPaintAppearance(page, base, check, mode, standing = 'guest') {
  const response = await fetch(base + '/toi', {headers: {accept: 'text/html'}});
  const html = await response.text();
  check(response.ok && !/id="(?:auth-appearance|page-appearance)"|<html[^>]+data-(?:theme|text|rail)=/.test(html),
    `${mode}: initial HTML contains neither appearance controls nor personal root overrides`);

  const errorsBefore = page.errors.length;
  await page.send('Network.setBlockedURLs', { urls: ['*/pkg/vink.js*', '*/pkg/vink_bg.wasm*'] });
  try {
    await page.goto(base + '/toi');
    await page.waitFor(`${ROOT_DEFAULTS} && ${MISSING_APPEARANCE} &&
      document.documentElement.dataset.hydrated !== 'true'`, `${mode}: first paint while the app bundle is blocked`);
    for (const theme of ['dark', 'light']) {
      await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: theme }] });
      await page.waitFor(`document.querySelector('#vink-favicon')?.href.endsWith('-${theme}.svg')
        && document.querySelector('#vink-manifest')?.href.endsWith('-${theme}.webmanifest') && ${ROOT_DEFAULTS}`,
      `${mode}: system ${theme} changes head assets without app hydration`);
    }
    await page.capture(`appearance-${mode}-${standing}-before-session`, { textScale: 100, textScaleLane: 'default-100' });
  } finally {
    await page.send('Network.setBlockedURLs', { urls: [] });
    const blockedImport = `TypeError: Failed to fetch dynamically imported module: ${base}/pkg/vink.js`;
    if (mode === 'ssr') {
      for (const deadline = Date.now() + 5000; page.errors.length === errorsBefore && Date.now() < deadline;) await sleep(50);
    }
    page.errors = [...page.errors.slice(0, errorsBefore), ...page.errors.slice(errorsBefore).filter(error => error !== blockedImport)];
  }
}

export async function closedAccountAppearance(page, { args, check }) {
  await page.goto(args.base + '/toi');
  await page.waitFor(`${HYDRATED} && ${q('#sign-in-closed')}`, 'closed sign-in on Tôi');
  check(await page.eval(`!${q('#password-login, #password-register, #sign-in-email, #pen-name-form')} &&
    ${q('#sign-in-closed')}.textContent.includes('Vink chưa mở đăng nhập')`), '#327 closed gate has no Auth form');
  await matrix(page, 'closed', '#sign-in-closed', check);
}

async function nativeAccountZoom(page, openPage, check, member) {
  const settings = await openPage();
  const { targetInfo } = await settings.send('Target.getTargetInfo');
  const defaultZoom = 'new Promise(resolve => chrome.settingsPrivate.getDefaultZoom(resolve))';
  try {
    await settings.send('Page.navigate', {url: 'chrome://settings/appearance'});
    await settings.waitFor("typeof chrome.settingsPrivate?.setDefaultZoom === 'function'", 'Chrome native zoom setting');
    check(await settings.eval(defaultZoom) === 1, 'Chrome starts at native zoom 100%');
    await page.viewport(1440);
    await settings.eval('chrome.settingsPrivate.setDefaultZoom(2)');
    check(await settings.eval(defaultZoom) === 2, 'Chrome native zoom is 200%');
    await page.send('Page.bringToFront');
    const expectedSize = member ? '24px' : '16px';
    await page.waitFor(`innerWidth === 720 && devicePixelRatio === 2 &&
      getComputedStyle(document.documentElement).fontSize === '${expectedSize}'`, 'real zoom halves CSS width and doubles pixel ratio');
    check(await page.eval(`${member ? q('#page-appearance') + " && document.documentElement.dataset.text === '150'" : MISSING_APPEARANCE} &&
      document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1`),
    member ? 'member app text remains 150% at browser zoom 200%' : 'guest email form keeps 100% app text at browser zoom 200%');
  } finally {
    await settings.eval('chrome.settingsPrivate?.setDefaultZoom(1)').catch(() => {});
    await page.cdp.send('Target.closeTarget', {targetId: targetInfo.targetId});
    await page.send('Page.bringToFront');
    await page.viewport(1280);
  }
}

async function authR3Login(page, check, openPage) {
  check(await page.eval(`(() => {
    const entry = document.querySelector('.vk-auth-entry'), form = ${q(LOGIN)};
    return entry?.contains(form) && entry.querySelectorAll('form').length === 1
      && !document.querySelector('#page-account .vk-account-status, #pen-name-form')
      && !form.querySelector('a[href*="quen"], [role=tablist]')
      && document.activeElement !== form.querySelector('input')
      && ${q(LOGIN + '-submit')}.textContent.trim() === 'Đăng nhập'
      && ${q('#use-registration')}.textContent.trim() === 'Tạo tài khoản'
      && ${q('#use-email-code')}.textContent.trim() === 'Dùng mã email';
  })()`), '#559 login renders one R3 form without the guest status card, premature pen-name step, duplicate actions or autofocus');
  await matrix(page, 'login', LOGIN, check);
  await page.eval(`${q(LOGIN + '-username')}.focus()`);
  const utilities = await page.eval(`${q('#open-account-deletion')}?.checkVisibility()`) ? ['#open-account-deletion'] : [];
  for (const id of [LOGIN + '-password', LOGIN + '-password-reveal', LOGIN + '-submit', '#use-email-code', '#use-registration', ...utilities]) {
    await page.key('Tab');
    check(await page.eval(`document.activeElement === ${q(id)} && document.activeElement.matches(':focus-visible')`), `#327 keyboard order and focus-visible: ${id}`);
  }
  await page.click('#use-email-code');
  await page.waitFor(`document.activeElement?.id === 'sign-in-address'`, 'email mode focuses its input');
  await matrix(page, 'email', '#sign-in-email', check);
  await nativeAccountZoom(page, openPage, check, false);
  await page.click('#use-password');
  await page.waitFor(q(LOGIN), 'password mode after email');
}

async function memberAppearance(page, { base, check, openPage, userA, userB, principalA, principalB }) {
  const sessionPath = '/api/v1/auth/session';
  await openAccount(page, base, 'active');
  await page.waitFor(`${DARK_LARGE_COMPACT} && ${q('#page-appearance')}`, 'member A restores all three choices after reload');

  const outage = await fabricate(page, 'GET', sessionPath, 503, {error: {code: 'dependency_unavailable'}});
  await page.eval("window.dispatchEvent(new Event('focus'))");
  await outage();
  await expectDefaultAppearance(page, check, 'failed read after a confirmed active session');
  check(await page.eval(`${STANDING} === 'active'`), 'a failed read retains the existing account UI state without exposing preferences');
  const recovery = await holdAnswer(page, 'GET', sessionPath);
  await page.eval("window.dispatchEvent(new Event('focus'))");
  check(await recovery.held() === 200, 'real A session read answers during recovery');
  await expectDefaultAppearance(page, check, 'reconfirmation pending');
  await recovery.finish();
  await page.waitFor(DARK_LARGE_COMPACT, 'reconfirmation restores only A’s choices');

  const tab = await openPage();
  const { targetInfo } = await tab.send('Target.getTargetInfo');
  try {
    await openAccount(tab, base, 'active');
    await tab.waitFor(DARK_LARGE_COMPACT, 'same-profile second tab sees A’s choices');
    const old = await holdAnswer(tab, 'GET', sessionPath);
    await tab.eval("window.dispatchEvent(new Event('focus'))");
    check(await old.held() === 200, 'the other tab holds a real session answer for A');
    await signOut(page, base);
    await expectDefaultAppearance(page, check, 'confirmed logout');
    await tab.waitFor(`${STANDING} === 'anonymous'`, 'cross-tab logout accepted');
    await expectDefaultAppearance(tab, check, 'cross-tab logout');
    await old.finish(false, true);
    await expectDefaultAppearance(tab, check, 'old Active answer delivered after logout');

    await credentials(page, LOGIN, userB, SECRET_B);
    await page.click(LOGIN + '-submit');
    await tab.waitFor(`${q('#page-appearance')} && ${ROOT_DEFAULTS}`, 'cross-tab B receives its defaults');
    await page.waitFor(`${q('#page-appearance')} && ${ROOT_DEFAULTS}`, 'B does not inherit A choices');
    check((await account(page)).body.principal_id === principalB, 'real account B receives defaults while A preferences are saved');
    await chooseAppearance(page, 'theme', 'light');
    await page.waitFor(`document.documentElement.dataset.theme === 'light'`, 'B saves its own theme');
    await chooseAppearance(page, 'theme', 'system');
    await signOut(page, base);
    await expectDefaultAppearance(page, check, 'B signs out');
    await credentials(page, LOGIN, userA, SECRET_A);
    await page.click(LOGIN + '-submit');
    await tab.waitFor(DARK_LARGE_COMPACT, 'cross-tab return to A restores A choices');
    await page.waitFor(DARK_LARGE_COMPACT, 'A reloads its choices without B theme');
    check((await account(page)).body.principal_id === principalA, 'real account A restores its theme, text and rail after B');
    await chooseAppearance(page, 'theme', 'system');
    await chooseAppearance(page, 'text', 100);
    await chooseAppearance(page, 'rail', 'auto');
    await page.waitFor(ROOT_DEFAULTS, 'restore A defaults for the subsequent profile audits');
  } finally {
    await page.cdp.send('Target.closeTarget', { targetId: targetInfo.targetId });
    await page.send('Page.bringToFront');
  }
}

export async function passwordAuth(page, { args, check, openPage, textContrast }) {
  const { base } = args;
  const ctx = { args, seen: new Set() };
  const userA = 'banviet.a';
  const userB = 'banviet.b';
  const penA = 'An Nhiên';
  const penB = 'Người giữ mùa thu';
  const email = 'taikhoan-email@vi-du.vn';
  const workQueuePath = '/api/v1/moderation/works/queue/page';

  await deskGuest(page, {args,check}, true);
  const unavailable = await fabricate(page, 'GET', '/api/v1/auth/session', 503, {error:{code:'dependency_unavailable'}});
  const gated = captureRequest(page, 'GET', '/api/v1/author/works');
  await page.goto(base+'/ban-viet');
  await unavailable();
  await page.waitFor(q('#desk-session-retry'), '#308 initial session failure offers a session retry');
  await page.click('#desk-session-retry');
  await page.waitFor(q('#desk-sign-in'), '#308 session retry confirms guest');
  check(gated.count()===0, '#308 unavailable and confirmed guest never query the private list');
  await page.click('#desk-sign-in');
  await page.waitFor(`location.pathname==='/toi' && new URLSearchParams(location.search).get('next') === '/ban-viet' && ${q(LOGIN)}`,
    'F-06 Desk sign-in links to the existing form with a return target');
  const suspended = await fabricate(page, 'GET', '/api/v1/auth/session', 200, {state:'suspended'});
  await page.goto(base+'/ban-viet');
  await suspended();
  await page.waitFor("document.querySelector('.vk-desk-access h2')?.textContent==='Tài khoản đang bị tạm khóa'", '#308 suspended session is distinct');
  check(gated.count()===0 && await page.eval("!document.querySelector('#draft-search, #retry-drafts')"), '#308 suspended state (intercepted) has no private controls or reads');
  gated.close();

  console.log('Auth password: registration, browser input, server cookie and own draft:');
  const guestReview = captureRequest(page, 'HEAD', workQueuePath);
  await openAccount(page, base, 'anonymous');
  check(await page.eval(`${q(LOGIN)} && !${q(REGISTER)} && !${q('#sign-in-email')}`), 'password is the default guest form');
  check(await page.eval(`(() => { const p=${q(LOGIN + '-password')}; return p.type === 'password' && p.name === 'password' && p.autocomplete === 'current-password' &&
    !p.hasAttribute('maxlength') && p.getAttribute('aria-required') === 'true' && ${q(LOGIN + '-username')}.autocomplete === 'username'; })()`), 'native login field names and autocomplete, with no raw maxlength');

  await seedLegacyAppearance(page);
  await firstPaintAppearance(page, base, check, 'ssr');
  await openAccount(page, base, 'anonymous');
  await authR3Login(page, check, openPage);
  const loginField = (name) => q(LOGIN + '-' + name);
  const invalid = (name, message) =>
    `${loginField(name)}.getAttribute('aria-invalid') === 'true' &&
    ${loginField(name)}.getAttribute('aria-describedby').split(' ').some(id =>
      document.getElementById(id)?.textContent.includes(${JSON.stringify(message)}))`;
  const blankLogin = captureRequest(page, 'POST', `${API}/login`);
  await page.click(LOGIN + '-submit');
  await page.waitFor(`${invalid('username', 'Hãy nhập tên đăng nhập.')} &&
    ${invalid('password', 'Hãy nhập mật khẩu.')} && document.activeElement === ${loginField('username')}`,
    'F-11 empty login marks both fields and focuses the first');
  const invalidPaint = await page.eval(`(() => {
    const fields = [...document.querySelectorAll('#password-login .vk-ui-field__control')];
    return fields.map(field => {
      const s = getComputedStyle(field), input = getComputedStyle(field.querySelector('input'));
      return [s.outlineWidth, s.outlineColor, s.borderTopWidth, input.outlineWidth];
    });
  })()`);
  check(invalidPaint[0][0] === '2px' && invalidPaint[1][0] === '1px' &&
    invalidPaint[0][1] === invalidPaint[1][1] &&
    invalidPaint.every(([, , border, inputOutline]) => border === '0px' && inputOutline === '0px'),
    `#559 error and error+focus use one shared perimeter (${JSON.stringify(invalidPaint)})`);
  await fill(page, LOGIN + '-username', userA);
  await page.key('Enter');
  await page.waitFor(`${invalid('password', 'Hãy nhập mật khẩu.')} && document.activeElement === ${loginField('password')} &&
    ${loginField('username')}.getAttribute('aria-invalid') !== 'true' &&
    ${loginField('username')}.value === ${JSON.stringify(userA)}`, 'F-11 Enter keeps the username and focuses the missing password');
  check(blankLogin.count() === 0, 'F-11 blank login sends zero requests');
  check(guestReview.count() === 0 && await page.eval(`!${q('#me-moderation-link')}`),
    'F-18 the guest has neither a moderation check nor a moderation link');
  blankLogin.close();
  guestReview.close();
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
  // Arm interception before any activation in this scenario, including the IME negative control.
  const heldRegistration = await holdAnswer(page, 'POST', `${API}/register`);
  await page.click(REGISTER + '-password-reveal');
  check(await page.eval(`(() => {
    const p = ${q(REGISTER + '-password')};
    return p === window.savedInput && p.type === 'text' && p.selectionStart === 2 &&
      p.selectionEnd === 7 && p.selectionDirection === 'backward' && document.activeElement === p &&
      ${q(REGISTER + '-password-reveal')}.getAttribute('aria-pressed') === 'true' &&
      ${q(REGISTER + '-password-reveal [data-icon="visibility-off"]')} &&
      ${q(REGISTER + '-password-reveal')}.getAttribute('aria-label') === 'Hiển thị mật khẩu';
  })()`), 'pointer reveal keeps the same input, focus and selection');
  await page.key('Tab');
  await page.key('Space');
  check(registration.count() === 0 && await page.eval(`
    ${q(REGISTER + '-password')}.type === 'password' && document.activeElement?.id === 'password-register-password-reveal' &&
    ${q(REGISTER + '-password-reveal [data-icon="visibility"]')} &&
    ${q(REGISTER + '-password-reveal')}.getAttribute('aria-pressed') === 'false'
  `), 'keyboard conceal keeps focus on the toggle and sends no mutation');
  await page.eval(`(() => {
    delete window.savedInput;
    const input = ${q(REGISTER + '-username')};
    input.focus();
    const composition = window.authComposition = { enter: null, submits: 0 };
    input.addEventListener('keydown', event => { composition.enter = event; }, { once: true });
    input.form.addEventListener('submit', () => { composition.submits++; }, { capture: true });
    input.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
  })()`);
  await page.key('Enter');
  check(registration.count() === 0 && await page.eval(`(() => {
    const { enter, submits } = window.authComposition;
    return enter?.key === 'Enter' && enter.target === ${q(REGISTER + '-username')} &&
      enter.defaultPrevented && submits === 0;
  })()`), 'Enter during simulated composition is canceled at the input and sends no form or network request');
  await page.eval(`${q(REGISTER + '-username')}.dispatchEvent(new CompositionEvent('compositionend', {bubbles: true}))`);

  await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
  await page.key('Enter');
  check(await page.eval('window.authComposition.submits === 1'), 'Enter after composition submits the registration form once');
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
  await page.eval('delete window.authComposition');
  await heldRegistration.finish();
  registration.close();
  await page.send('Emulation.setEmulatedMedia', { features: [] });
  await page.waitFor(`${STANDING} === 'needs_principal'`, 'registration needs pen name');
  check(await page.eval(`!${q(REGISTER + '-password')} && !${q('#password-credential')} &&
    document.querySelectorAll('.vk-auth-entry form').length === 1 && ${q('#pen-name-save')}.textContent.trim() === 'Lưu bút danh'`),
    '#327 the server-confirmed needs_principal replaces the credentials with one pen-name form; no credential remains');
  await matrix(page, 'pen-name', '#pen-name-form', check);
  const unbound = captureRequest(page, 'GET', '/api/v1/author/works');
  await page.goto(base+'/ban-viet');
  await page.waitFor("document.querySelector('.vk-desk-access h2')?.textContent==='Đặt bút danh để bắt đầu'", '#308 unbound session is distinct');
  check(unbound.count()===0 && await page.eval("!document.querySelector('#draft-search, #retry-drafts')"), '#308 the real unbound session never queries the private list');
  unbound.close();
  await page.click('#desk-pen-name');
  await page.waitFor(`new URLSearchParams(location.search).get('next') === '/ban-viet' && ${q('#pen-name-form')}`,
    'F-06 the Desk pen-name action preserves its return target');
  const A = await setPenName(page, penA, { returnToDesk: true });
  check(A.display_name === penA && A.display_name !== userA, 'the server confirms a separate pen name');
  const draftA = await createDraft(page, '[Mẫu] Nháp tạo sau đăng ký');
  await expiredDesk(page, {base, check, openPage});
  const writerReview = captureRequest(page, 'HEAD', workQueuePath);
  await openAccount(page, base, 'active');
  check(writerReview.count() === 0, 'Tôi makes no moderation probe for an ordinary writer');
  writerReview.close();
  check((await read(page, workQueuePath)).status === 403, 'the server refuses the ordinary writer’s queue read');
  await page.waitFor(`document.querySelectorAll('#page-appearance').length === 1 && ${ROOT_DEFAULTS}`,
    'new native member gets one settings region and ignores unowned legacy keys');
  await chooseAppearance(page, 'theme', 'dark');
  await chooseAppearance(page, 'text', 150);
  await chooseAppearance(page, 'rail', 'compact');
  await page.waitFor(DARK_LARGE_COMPACT, 'real A account applies theme, text and rail from Tôi');
  for (const width of [320, 390, 1440]) {
    await page.viewport(width);
    for (const theme of ['light', 'dark']) {
      await chooseAppearance(page, 'theme', theme);
      await page.waitFor(`document.documentElement.dataset.theme === '${theme}' &&
        document.documentElement.dataset.text === '150'`, 'member appearance matrix');
      await page.eval(`document.querySelector('#page-appearance').scrollIntoView({block:'start', behavior:'instant'})`);
      await page.capture(`appearance-ssr-native-active-${width}-${theme}`, {
        textScale: 150, textScaleLane: 'app-appearance-control', requiredTargets: ['#page-appearance-title'],
      });
    }
  }
  await nativeAccountZoom(page, openPage, check, true);
  await firstPaintAppearance(page, base, check, 'ssr', 'active');
  await openAccount(page, base, 'active');
  await page.waitFor(DARK_LARGE_COMPACT, 'SSR active reload applies choices only after confirmation');
  check(await page.eval(`!${q('#me-moderation-link')}`), 'F-18 Tôi offers no moderation link to an ordinary writer');
  await page.click('#password-credential');
  await page.waitFor(contains('đã có thông tin đăng nhập'), 'credential already present');
  check(await page.eval(`!${q(ENROLL + '-send')}`), 'already-enrolled account has no second enrollment form');

  await page.eval("window.credentialStatus = document.getElementById('password-credential-status')");
  await page.click('#password-credential');
  check(await page.eval("window.credentialStatus.closest('[hidden]') && document.activeElement?.id === 'password-credential'"),
    '#521 closing credentials hides its mounted settings and keeps focus on the row');
  await page.key('Enter');
  check(await page.eval("window.credentialStatus === document.getElementById('password-credential-status') && !window.credentialStatus.closest('[hidden]')"),
    '#521 keyboard reopening retains the same credential settings');
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
  await credentials(page, LOGIN, userA, SECRET_A);
  const throttled = await fabricate(page, 'POST', `${API}/login`, 429,
    { error: { code: 'rate_limited', message: 'fixture' } }, [{ name: 'Retry-After', value: '8' }]);
  await page.click(LOGIN + '-submit');
  await throttled();
  await page.waitFor(`${q(LOGIN + '-submit-note')}?.innerText.includes('Có thể thử lại sau')`, 'password retry-after feedback');
  check(await page.eval(`${q(LOGIN + '-submit')}.getAttribute('aria-disabled') === 'true'`), '429 disables the password action during Retry-After');
  await matrix(page, 'cooldown', LOGIN, check);
  await page.waitFor(`${q(LOGIN + '-submit')}.getAttribute('aria-disabled') !== 'true'`, 'password cooldown elapsed');
  // A stale form may not update the replacement form; the late Set-Cookie still has to be read.
  await page.goto(base + '/ban-viet');
  await page.waitFor(q('#desk-sign-in'), 'returning guest at the Desk');
  await page.click('#desk-sign-in');
  await page.waitFor(`location.pathname === '/toi' && ${q(LOGIN)}`, 'F-06 returning guest opens contextual sign-in');
  await credentials(page, LOGIN, userA, SECRET_A);
  const late = await holdAnswer(page, 'POST', `${API}/login`);
  await page.click(LOGIN + '-submit');
  check(await late.held() === 200, 'real NFC login matches NFD registration');
  await page.click('#use-email-code');
  await page.waitFor(q('#sign-in-address'), 'switch to email while password login is in flight');
  check(await page.eval(`!${q(LOGIN + '-password')} && ${q('#sign-in-send')}.getAttribute('aria-disabled') === 'true'`),
    'switch removes the password and keeps the single auth lane occupied');
  await late.finish();
  await page.waitFor(`location.pathname === '/ban-viet' && ${q('#create-draft')} && !${q('#sign-in-email')}`,
    'F-06 late real cookie makes the Desk available and returns to it');
  const returnedA = (await account(page)).body;
  check(returnedA.principal_id === A.principal_id && returnedA.display_name === penA &&
    bodyText(await page.draft(draftA)) === DRAFT_TEXT, 'password returns to the same principal, pen name and stored draft');

  console.log('Auth password: first verified email, wrong code and lost completion ACK:');
  await openAccount(page, base, 'active');
  const attachedEmail = await attachVerifiedEmail(page, { args, check, accountBefore: A, draft: draftA, seen: ctx.seen });
  await signOut(page, base);
  await signIn(page, ctx, attachedEmail);
  check((await account(page)).body.principal_id === A.principal_id && bodyText(await page.draft(draftA)) === DRAFT_TEXT &&
    await page.eval(`!${q('#pen-name-form')}`), '#327/#329 sign-in with the newly attached mailbox returns to the same writer and saved draft, without onboarding');

  console.log('Auth password: email account adds a credential without changing its session:');
  const { browserContextId } = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
  const other = await openPage(browserContextId);
  await openAccount(other, base, 'anonymous');
  await signIn(other, ctx, email);
  const B = await setPenName(other, penB);
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
  await other.waitFor(`${q('#email-attach-verified')}?.textContent.includes(${JSON.stringify(email)})`,
    '#329 eligibility changes after confirmed enrollment: email already bound is shown without reloading Tôi');
  check(await other.eval(`!${q('#email-attach-add')}`), '#329 the newly enrolled account cannot attach its verified email again');
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

  const switchActor = async (actor, control = page, standing = 'active') => {
    await openAccount(control, base, standing);
    if (standing === 'active') await signOut(control, base);
    if (actor === 'guest') return;
    await credentials(control, LOGIN, actor === 'a' ? userA : userB, actor === 'a' ? SECRET_A : SECRET_B);
    await control.click(LOGIN + '-submit');
    await control.waitFor(`${STANDING} === 'active'`, 'replacement cookie confirmed');
  };
  await memberAppearance(page, {
    base, check, openPage, userA, userB, principalA: A.principal_id, principalB: B.principal_id,
  });
  await authorFollows(page, other, {
    base, openPage, check, holdAnswer, textContrast, switchActor,
  });
  await followingFeed(page, other, {
    base, openPage, check, holdAnswer, switchActor, textContrast,
  });
  await authorProfiles(page, other, {
    base, openPage, check, holdAnswer, textContrast,
    switchActor,
  });
  await authorHandles(page, {base,openPage,check,holdAnswer,textContrast});

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

export async function accountDeletion(page, { args, check, openPage }) {
  const { base } = args;
  const db = accountDeletionFixtureSql;
  const username = 'tacgia.xoa';
  const pen = 'Bút danh cần thu hồi';
  await openAccount(page, base, 'anonymous');
  await page.click('#use-registration');
  await credentials(page, REGISTER, username, SECRET_A);
  await page.click(REGISTER + '-submit');
  await setPenName(page, pen);
  const principal = (await read(page, '/api/v1/auth/session')).data.principal_id;
  const { handle } = (await read(page, '/api/v1/me/profile')).data;
  const { browserContextId } = await page.cdp.send('Target.createBrowserContext');
  const other = await openPage(browserContextId);
  await openAccount(other, base, 'anonymous');
  await credentials(other, LOGIN, username, SECRET_A);
  await other.click(LOGIN + '-submit');
  await other.waitFor(`${STANDING} === 'active'`, 'second independent browser session');
  await page.send('Page.bringToFront');

  const work = randomUUID();
  const revision = randomUUID();
  // A valid runtime Publication binding, cloned from the scratch seed. The owner came from the
  // real registration above. Only the fault trigger is fabricated.
  db(`BEGIN;
    INSERT INTO works (id, owner_id) VALUES ('${work}', '${principal}');
    INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
      SELECT '${revision}', '${work}', title, document, content_hash, '${principal}'
        FROM revisions WHERE work_id = '33333333-3333-4333-8333-333333333333';
    INSERT INTO work_revision_moderation (work_id, revision_id, decision_version, moderation, provenance_kind)
      VALUES ('${work}', '${revision}', 2, 'approved', 'legacy_bootstrap');
    INSERT INTO work_publications (work_id, audience, lifecycle, generation, intent_revision_id, intent_generation,
      pointer_revision_id, pointer_intent_generation, ever_public, public_epoch)
      VALUES ('${work}', 'public', 'active', 2, '${revision}', 2, '${revision}', 2, ARRAY['${revision}'::uuid], 1);
    INSERT INTO publications (work_id, revision_id, generation, published_at)
      VALUES ('${work}', '${revision}', 2, now());
    CREATE FUNCTION inject_account_deletion_failure() RETURNS trigger LANGUAGE plpgsql AS
      $$BEGIN RAISE EXCEPTION 'scratch-only rollback'; END;$$;
    CREATE TRIGGER inject_account_deletion_failure BEFORE INSERT ON account_deletions
      FOR EACH ROW EXECUTE FUNCTION inject_account_deletion_failure();
    COMMIT`);
  try {
    await page.goto(`${base}/tac-pham/${work}`);
    await page.waitFor(q('.vk-document'), 'Reader before deletion');
    await openAccount(page, base, 'active');
    await page.click('#open-account-deletion');
    await page.waitFor(`document.activeElement?.id === 'account-deletion-title'`, 'explicit opening focuses the warning');
    for (const width of [320, 1440]) {
      await page.viewport(width, 800);
      check(await page.eval(`(() => {
        const form = document.querySelector('#delete-account-form');
        const action = document.querySelector('#confirm-account-deletion-password');
        return form.scrollWidth <= form.clientWidth + 1 &&
          action.getBoundingClientRect().right <= window.innerWidth;
      })()`), `the warning and confirmation fit at ${width}px`);
    }
    await fill(page, '#account-delete-username', username);
    await fill(page, '#account-delete-password', 'mot-mat-khau-sai');
    await page.click('#confirm-account-deletion-password');
    await page.waitFor(contains('Tên đăng nhập hoặc mật khẩu chưa đúng'), 'wrong proof shows an error and keeps the account active');
    await fill(page, '#account-delete-password', SECRET_A);
    await page.click('#confirm-account-deletion-password');
    await page.waitFor(`${STANDING} === 'anonymous' && ${contains('Yêu cầu đã được tiếp nhận')}`, 'revoked at Identity while App rolls back');
    check((await read(other, '/api/v1/auth/session')).data.state === 'anonymous',
      'the other device lost its session before App recovered');
    check(db('SELECT count(*) FROM account_deletions') === '0', 'failed App transaction wrote no fence receipt');
  } finally {
    db('DROP TRIGGER inject_account_deletion_failure ON account_deletions; DROP FUNCTION inject_account_deletion_failure()');
  }
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline && db('SELECT count(*) FROM account_deletions') === '0') await sleep(150);
  await page.click('#check-account-deletion');
  await page.waitFor(contains('Tài khoản đã được xóa vĩnh viễn'), 'background retry committed on the App and status reconciled');
  await page.eval('history.back()');
  await page.waitFor(q('[data-testid=reader-author-deleted]'), 'Back from Account loads the neutral Reader replacement');
  check(await page.eval(`!document.querySelector('.vk-document') && !document.body.innerText.includes(${JSON.stringify(pen)})`),
    'the replacement has no original text or author name');
  const old = await fetch(`${base}/@${handle}`, { redirect: 'manual' });
  check(old.status === 410 && !old.headers.has('location'), 'the earlier Handle stays gone');

  await openAccount(page, base, 'anonymous');
  await page.click('#use-registration');
  await credentials(page, REGISTER, username, SECRET_A);
  await page.click(REGISTER + '-submit');
  await setPenName(page, 'Tác giả mới');
  const replacement = (await read(page, '/api/v1/auth/session')).data.principal_id;
  check(replacement !== principal, 'reusing the username creates a different principal');
  await signOut(page, base);

  const email = 'xoa-tai-khoan@example.test';
  const seen = new Set();
  await signIn(page, { args, seen }, email);
  await setPenName(page, 'Tác giả dùng mã');
  const suspended = (await read(page, '/api/v1/auth/session')).data.principal_id;
  db(`INSERT INTO principal_suspensions (id, principal_id, reason)
        VALUES ('${randomUUID()}', '${suspended}', 'Đình chỉ trong kiểm thử')`);
  await openAccount(page, base, 'suspended');
  await page.click('#open-account-deletion');
  await page.click('#account-delete-with-email');
  await fill(page, '#account-delete-email', email);
  await page.click('#send-account-deletion-code');
  await page.waitFor(q('#account-delete-code'), 'dedicated email OTP step');
  await fill(page, '#account-delete-code', await mailedCode(args.outbox, email, seen));
  await page.click('#confirm-account-deletion-code');
  await page.waitFor(`${STANDING} === 'anonymous' && ${contains('Tài khoản đã được xóa vĩnh viễn')}`, 'real email OTP retires the suspended account without issuing a session');
  await signIn(page, { args, seen }, email);
  await setPenName(page, 'Người dùng email mới');
  check((await read(page, '/api/v1/auth/session')).data.principal_id !== suspended,
    'the same email proves a different principal after deletion');
}

// Same Chrome driver and real password/editor journeys against the standalone host.
export async function standalone(page, {args, check, openPage}) {
  const base = args.base;
  const work = '33333333-3333-4333-8333-333333333333';
  const reader = base + '/tac-pham/' + work;
  const line = 'Phố cũ mưa giăng sợi mỏng,';
  const documentResponse = await fetch(reader, {headers: {accept:'text/html'}});
  const html = await documentResponse.text();
  check(documentResponse.ok && html.includes('<noscript>') && !html.includes(line),
    'direct Reader response is the static shell with the no-JS notice');
  await page.goto(reader);
  await page.waitFor(`document.querySelector('.vk-document')?.textContent.includes(${JSON.stringify(line)})`,
    'Reader text from the scratch publication API');
  check(await page.eval(`performance.getEntriesByType('resource').some(e => new URL(e.name).pathname === '/api/v1/works/${work}/reader')`),
    'Reader loaded the authorized HTTP projection');
  await page.goto(base + '/?q=' + encodeURIComponent('mưa'));
  await page.waitFor(q('.vk-public-row__link'), 'search results');
  await page.click('.vk-public-row__link');
  await page.waitFor(q('.vk-document'), 'client navigation to Reader');
  await page.eval('history.back()');
  await page.waitFor(q('.vk-public-row__link'), 'Back to search');
  await page.eval('history.forward()');
  await page.waitFor(q('.vk-document'), 'Forward to Reader');

  // A revisited work must wait for the current authority even if its earlier body was cached.
  await page.eval('history.back()');
  await page.waitFor(q('.vk-public-row__link'), 'cached search on Back');
  const outage = await fabricate(page, 'GET', '/api/v1/works/' + work + '/reader', 503,
    {error:{code:'dependency_unavailable'}});
  await page.click('.vk-public-row__link');
  await page.waitFor(q('[data-testid=public-unavailable]'), 'Reader API outage');
  await outage();
  check(!await page.eval(`document.body.textContent.includes(${JSON.stringify(line)})`),
    'failed revalidation keeps the old publication body out of the Reader');
  const readerDocument = await page.eval('performance.timeOrigin');
  await page.click('#retry-public');
  await page.waitFor(q('.vk-document'), 'retry obtains the current publication');
  check(await page.eval('performance.timeOrigin') === readerDocument, 'Reader recovery refetches without replacing the document');

  await page.goto(base + '/the/' + encodeURIComponent('ký-ức'));
  await page.waitFor(`document.querySelector('main h1')?.textContent.includes('Ký ức')`, 'public tag deep link');
  await page.goto(base + '/@nguoi-chua-co');
  await page.waitFor(q('main [data-panel="error:not-found"]'), 'unknown author uses shared NotFound in CSR');

  await page.goto(base + '/thong-bao');
  await page.waitFor(`${q('main [data-panel="error:not-found"]')} && document.title === 'Không tìm thấy — Vink'`,
    'confirmed guest Inbox uses shared NotFound in CSR');
  check(await page.eval(`location.pathname === '/thong-bao' && document.querySelectorAll('main h1').length === 1
    && !document.querySelector('main a[href="/toi"], #notification-page-title')`),
    'CSR Inbox keeps its URL and has no guest-only account prompt');

  await seedLegacyAppearance(page);
  await firstPaintAppearance(page, base, check, 'csr');
  await page.goto(base + '/toi');
  await page.waitFor(q('#use-registration'), 'anonymous password form');
  await expectDefaultAppearance(page, check, 'CSR guest after mount');
  await page.click('#use-registration');
  const username = 'spa' + Date.now().toString(36);
  await credentials(page, REGISTER, username, SECRET_A);
  await page.click(REGISTER + '-submit');
  await setPenName(page, 'Người viết SPA');
  await page.goto(base + '/toi');
  await page.waitFor(`${STANDING} === 'active'`, 'CSR account confirmation');
  await page.waitFor(q('#page-appearance'), 'CSR native member settings');
  await chooseAppearance(page, 'theme', 'dark');
  await chooseAppearance(page, 'text', 150);
  await page.waitFor(`document.documentElement.dataset.theme === 'dark' &&
    document.documentElement.dataset.text === '150'`, 'native member applies CSR choices');
  await page.capture('appearance-csr-native-active', {textScale: 150, textScaleLane: 'app-appearance-control'});
  await firstPaintAppearance(page, base, check, 'csr', 'active');
  await page.goto(base + '/toi');
  await page.waitFor(`${STANDING} === 'active'`, 'CSR account confirmation');
  await page.waitFor(`document.documentElement.dataset.theme === 'dark' &&
    document.documentElement.dataset.text === '150'`, 'CSR applies saved choices after mount and confirmation');
  const author = await read(page, '/api/v1/me/profile');
  check(author.status === 200 && Boolean(author.data?.handle), 'new author has a public handle');
  await page.goto(base + '/?mode=following');
  const discoveryLink = q('[data-feed-state="no-follows"] a[href="/?mode=authors"]');
  await page.waitFor(`${discoveryLink}?.textContent.trim() === 'Khám phá tác giả'`, 'signed-in CSR shows the empty Following CTA');
  const documentStart = await page.eval('performance.timeOrigin');
  await page.eval(`${discoveryLink}.focus()`);
  await page.key('Enter');
  await page.waitFor(`location.search === '?mode=authors' &&
    ${q('#explore-mode-authors')}?.getAttribute('aria-current') === 'page' &&
    ${q('#explore')}?.dataset.search === 'closed'`, 'CSR Enter opens Authors with Search Dock closed');
  check(await page.eval(`performance.timeOrigin === ${documentStart}`), 'the Following CTA uses CSR navigation');
  await page.eval('history.back()');
  await page.waitFor(`location.search === '?mode=following' && ${discoveryLink}`, 'CSR Back returns to the no-follows state');
  const id = await createDraft(page, 'Bản thảo trên Vink');
  check(await page.body() === DRAFT_TEXT, 'saved Vietnamese draft reopens after a full document reload');
  await page.typeInBody(' Chưa lưu.');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]')?.open`, 'unsaved exit guard');
  await page.click('#leave-stay');
  check((await page.body()).endsWith(' Chưa lưu.'), 'staying in the editor keeps the unsaved text');
  await page.click('#save');
  await page.waitStatus('saved');
  await page.waitFor(q('#audience-public'), 'author publication controls');
  await page.eval("document.querySelector('#audience-public').click()");
  await page.waitFor(q('#publication-primary'), 'request review of the saved revision');
  await page.eval("document.querySelector('#publication-primary').click()");
  await page.waitFor(`document.querySelector('#publication')?.dataset.phase === 'reconciled' && document.querySelector('#publication-status')?.textContent.trim() === 'Đang chờ kiểm duyệt'`, 'pending review');
  const publication = await read(page, '/api/v1/works/' + id + '/publication/state');
  check(publication.data?.revisions.length === 1 && publication.data.revisions[0].moderation === 'pending' && publication.data.intent?.revision_id === publication.data.revisions[0].revision_id && !publication.data.pointer,
    'author submission records one pending revision without publishing it');
  await page.goto(reader);
  await page.waitFor(q('#root-composer'), 'signed-in discussion composer');
  const comment = 'Lời nhận xét đang chờ duyệt.';
  await page.typeAtEnd('#root-composer', comment);
  await page.click('#root-composer-send');
  await page.waitFor(`document.querySelector('#own-comments')?.textContent.includes(${JSON.stringify(comment)})`, 'own pending comment');
  const discussion = await read(page, '/api/v1/works/' + work + '/discussion/public');
  check(discussion.status === 200 && !JSON.stringify(discussion.data).includes(comment),
    'a submitted comment appears only in its author’s overlay before approval');
  await page.goto(base + '/kiem-duyet');
  await page.waitFor(q('[data-panel="error:not-found"]'), 'retired moderation deep link');
  check((await read(page, '/api/v1/moderation/automation')).status === 403,
    'the ordinary account cannot read the administrator’s automation controls');
  await page.goto(base + '/toi');
  await page.waitFor(q('#sign-out'), 'signed-in account');
  await signOut(page, base);
  const result = await read(page, '/api/v1/drafts/' + id);
  check(result.status === 401, 'logout revokes the private draft read on the same origin');
  await expectDefaultAppearance(page, check, 'CSR logout');
  await page.waitFor(q('#use-registration'), 'account switch');
  await page.click('#use-registration');
  await credentials(page, REGISTER, username + 'b', SECRET_B);
  await page.click(REGISTER + '-submit');
  await setPenName(page, 'Bạn đọc SPA');
  await page.waitFor(`${q('#page-appearance')} && ${ROOT_DEFAULTS}`, 'CSR second native member gets its own defaults');
  check((await read(page, '/api/v1/drafts/' + id)).status === 404,
    'the second account cannot read the first account’s private draft');
  await page.goto(base + '/@' + encodeURIComponent(author.data.handle));
  await page.waitFor(`document.querySelector('#author-profile-heading')?.textContent.includes('Người viết SPA')`,
    'the second account can open the public author profile');
  check(!await page.eval("document.body.textContent.includes('Bản thảo trên Vink')"),
    'a pending work stays off the public author profile');
  await standaloneWorkViewCheck(page, { args, check });
  // #388: the work-view check ends signed out; the save journey starts as a guest.
  await readerSave(page, { args, check, openPage,
    reader: { username: username + 'b', password: SECRET_B },
    other: { username, password: SECRET_A } });
}
