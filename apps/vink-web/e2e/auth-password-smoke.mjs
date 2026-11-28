// AUTH-PASSWORD-WEB-01: real Chrome, App, native Identity, separate scratch PostgreSQL.
// The mail carrier is a local outbox. Faults delay/drop a real response or explicitly inject
// a refusal; no synthetic success is used. Don't log request bodies, credentials or cookies.

import { checkAuthR31, checkModeSwitches, snapshotAuthAction, checkAuthPending } from './auth-r3-smoke.mjs';
import { accountDeletionFixtureSql } from './author-fixtures.mjs';
import { randomUUID } from 'node:crypto';
import { wakeDesk } from './desk-lifecycle.mjs';
import { followingFeed } from './following-feed-smoke.mjs';
import { authorFollows } from './author-follow-smoke.mjs';
import { authorHandles } from './author-handle-smoke.mjs';
import { authorProfiles } from './author-profile-smoke.mjs';
import { EDITOR_READY, HYDRATED, STANDING, account, cookies, fabricate, fill, mailedCode, openAccount, q, registrationProof, signIn, signOut } from './auth-smoke.mjs';
import { deskGuest } from './desk-empty-smoke.mjs';
import { captureRequest, holdAnswer } from './auth-browser-network.mjs';
import { APPEARANCE_SETTLED } from './visual-evidence.mjs';
import { nativeAuthorMetrics } from './work-view-author.mjs';
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
const notificationTab = '#bottom-navigation [data-destination=notifications]';
const openInbox = 'location.pathname === "/thong-bao"';
const guestTabs = `[...document.querySelectorAll('#bottom-navigation a')].map(a => a.getAttribute('href')).join('|') === '/|/ban-viet|/toi'`;

const read = (page, path) => page.eval(`fetch(${JSON.stringify(path)}).then(async r => ({
  status: r.status, cache: r.headers.get('cache-control'), data: r.ok ? await r.json() : null
}))`);

// Send only a real current or earlier scope from this scratch browser session.
async function notificationGateway(page, validScope, earlierScope = null) {
  return page.eval(`(async () => {
    const count = scope => fetch('/api/v1/me/notifications/count', {
      headers: { 'x-vink-notification-scope': scope }, cache: 'no-store',
    }).then(response => ({ status: response.status, noStore: response.headers.get('cache-control') === 'no-store' }));
    return { current: await count(${JSON.stringify(validScope)}),
      earlier: ${earlierScope !== null} ? await count(${JSON.stringify(earlierScope)}) : null };
  })()`);
}

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
  const authMode = ['login', 'registration', 'email'].includes(scenario);
  const actions = ({
    login: ['#password-login-submit', '#use-email-code', '#use-registration'],
    registration: ['#registration-send', '#use-password'],
    email: ['#sign-in-send', '#use-password'],
    'pen-name': ['#pen-name-save'],
    closed: [],
  })[scenario];
  const guest = actions !== undefined;
  const boundary = scenario === 'closed' ? 'App (sign-in disabled) + scratch PostgreSQL' :
    'native Identity + App + scratch PostgreSQL';
  const widths = guest ? [320, 390, 834, 1440] : [320, 390, 1280];
  for (const width of widths) for (const theme of ['light', 'dark']) for (const scale of authMode ? [100, 150, 200] : [100, 200]) {
    await page.viewport(width, width === 834 ? 600 : 900);
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: theme }] });
    await page.eval("document.documentElement.style.fontSize = ''");
    if (guest) await expectDefaultAppearance(page, check, scenario);
    await page.waitFor(APPEARANCE_SETTLED, 'Auth appearance colours settle');
    const endTarget = actions?.at(-1) ?? anchor;
    const measured = await page.eval(`(async () => {
      if (${scale} === 150) document.documentElement.dataset.text = '150';
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
    const actionsR31 = authMode ? await checkAuthR31(page, check, `/toi ${width}/${theme}/${scale}`, scenario) : undefined;
    const textScaleLane = scale === 100 ? 'default-100' : 'dom-override-geometry-only';
    if (((width === 390 || authMode && width === 1440) && scale === 100) || (width === 320 && scale === 200 && theme === 'dark') ||
        (width === 1280 && scale === 100 && theme === 'light')) {
      await page.capture(`auth-password-${scenario}-${width}-${theme}-${scale}`, {
        boundary,
        requiredTargets: measured.partial ? [] : [anchor],
        scenario, textScale: scale, textScaleLane, font: 'Be Vietnam Pro', ...measured, actionsR31,
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
    if (authMode && scale === 150) await page.eval("delete document.documentElement.dataset.text");
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
  check(await page.eval(`${q('#account-management-help')}?.getAttribute('href') === '/toi?quan-ly=xoa'`), 'Guest can find account support outside the login form');
  for (const id of [LOGIN + '-password', LOGIN + '-password-reveal', LOGIN + '-submit', '#use-email-code', '#use-registration', '#account-management-help']) {
    await page.key('Tab');
    check(await page.eval(`document.activeElement === ${q(id)} && document.activeElement.matches(':focus-visible')`), `#327 keyboard order and focus-visible: ${id}`);
  }
  await page.click('#use-email-code');
  await page.waitFor(`document.activeElement?.id === 'sign-in-address'`, 'email mode focuses its input');
  check(await page.eval(`${q('.vk-auth-entry .vk-auth-heading h2')}.textContent.trim() === 'Đăng nhập bằng email'
    && ${q('#sign-in-send')}.textContent.trim() === 'Gửi mã đăng nhập'
    && ${q('#use-password')}.textContent.trim() === 'Dùng mật khẩu'
    && !${q('#sign-in-email > p.vk-muted')}`),
    '#585 email mode identifies the method and each action without a duplicate introduction');
  await matrix(page, 'email', '#sign-in-email', check);
  await nativeAccountZoom(page, openPage, check, false);
  await page.click('#use-password');
  await page.waitFor(q(LOGIN), 'password mode after email');
  const { browserContextId } = await page.cdp.send('Target.createBrowserContext');
  try {
    const direct = await openPage(browserContextId);
    await openAccount(direct, await page.eval('location.origin'), 'anonymous');
    // A fresh CDP target can retain its initial about:blank entry.
    await direct.send('Page.resetNavigationHistory');
    const { entries, currentIndex } = await direct.send('Page.getNavigationHistory');
    check(entries.length === 1 && currentIndex === 0 && new URL(entries[0].url).pathname === '/toi',
      '#559 /toi is the only entry in the direct-entry fixture');
    await checkModeSwitches(direct, check, '/toi from direct entry');
  } finally {
    await page.cdp.send('Target.disposeBrowserContext', { browserContextId });
    await page.send('Page.bringToFront');
  }
}

async function memberAppearance(page, { base, check, openPage, other, userA, principalA, principalB }) {
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

    // Reuse B's real Identity cookie from the isolated test profile. The shared login
    // quota is also exercised by the later password recovery fixture.
    const bSession = (await cookies(other, base))[sessionCookie];
    check(bSession?.httpOnly && bSession.secure, 'B already has a real browser session');
    await page.send('Network.setCookie', {name: bSession.name, value: bSession.value, url: base,
      path: '/', secure: true, httpOnly: true, sameSite: bSession.sameSite});
    const notifyTabs = () => page.eval(`(() => {
      const channel = new BroadcastChannel('vink-session');
      channel.postMessage('changed');
      channel.close();
    })()`);
    await notifyTabs();
    await tab.waitFor(`${q('#page-appearance')} && ${ROOT_DEFAULTS}`, 'cross-tab B receives its defaults');
    await page.waitFor(`${q('#page-appearance')} && ${ROOT_DEFAULTS}`, 'B does not inherit A choices');
    check((await account(page)).body.principal_id === principalB, 'real account B receives defaults while A preferences are saved');
    await chooseAppearance(page, 'theme', 'light');
    await page.waitFor(`document.documentElement.dataset.theme === 'light'`, 'B saves its own theme');
    await chooseAppearance(page, 'theme', 'system');
    await page.send('Network.deleteCookies', {name: sessionCookie, url: base});
    await notifyTabs();
    await page.waitFor(`${STANDING} === 'anonymous'`, 'the shared browser loses B’s cookie');
    await expectDefaultAppearance(page, check, 'B’s session is absent from the shared browser');
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
  await page.waitFor(`document.activeElement?.id === 'registration-address'`, 'registration focuses the email field');
  await matrix(page, 'registration', '#registration-email', check);
  const registrationEmail = 'nhap-da-luu@vi-du.vn';
  await registrationProof(page, args.outbox, registrationEmail, ctx.seen);
  check(await page.eval(`!${q(LOGIN)} && ${q(REGISTER + '-password')}.autocomplete === 'new-password'`), 'proof completion accepts new credentials');
  await page.viewport(834, 1112);
  await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
  await page.waitFor(APPEARANCE_SETTLED, 'Registration screenshot appearance ready');
  await page.capture('auth-r5-register-credentials-834-light-100', {
    scenario: 'register-credentials', textScale: 100, requiredTargets: [REGISTER, REGISTER + '-username'],
  });
  await page.viewport(1280);
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
  await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
  const registerAction = await snapshotAuthAction(page, REGISTER + '-submit');
  await page.key('Enter');
  check(await page.eval('window.authComposition.submits === 1'), 'Enter after composition submits the registration form once');
  check(await heldRegistration.held() === 200, 'registration commits through the App with DOM autofill and Unicode NFD');
  await page.waitFor(`${q(REGISTER + '-submit')}.getAttribute('aria-busy') === 'true'`, 'registration pending');
  await checkAuthPending(page, check, REGISTER + '-submit', registerAction, 'person-add', 'Tạo tài khoản',
    REGISTER.slice(1) + '-username');
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
  check((await snapshotAuthAction(page, '#sign-out')).icon === 'logout' &&
    await page.eval("document.querySelector('#sign-out').classList.contains('vk-ui-button--tonal')"),
    '#625 pen-name gate offers Tonal Logout');
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
  const accountSettings = '.vk-account-settings';
  await page.waitFor(`${q('#profile-privacy-link')} && ${q('#account-email')}`, '#602 real account settings and email state');
  for (const [width, scale] of [[320, 200], [390, 100], [390, 150], [834, 150], [1440, 100]]) {
    await page.viewport(width, width === 390 ? 844 : 900);
    for (const theme of ['light', 'dark']) {
      await chooseAppearance(page, 'theme', theme);
      await chooseAppearance(page, 'text', scale === 150 ? 150 : 100);
      await page.eval(`document.documentElement.style.fontSize = '${scale === 200 ? '200%' : ''}'`);
      await page.waitFor(`document.documentElement.dataset.theme === '${theme}' &&
        getComputedStyle(document.documentElement).fontSize === '${16 * scale / 100}px' &&
        ${APPEARANCE_SETTLED}`, 'member appearance matrix');
      await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 });
      const settings = await page.eval(`(() => {
        const region = document.querySelector('${accountSettings}'), edge = region.getBoundingClientRect();
        const rows = ['#profile-privacy-link', '#password-credential', '#account-email', '#page-appearance-title', '#sign-out']
          .map(selector => region.querySelector(selector));
        const axes = rows.map(row => row.querySelector('svg, [data-appearance-icon]').getBoundingClientRect().left);
        const transparent = 'rgba(0, 0, 0, 0)';
        const settingsFit = rows.every((row, index) => {
          const rect = row.getBoundingClientRect(), style = getComputedStyle(row);
          return rect.height >= 48 && rect.left >= edge.left && rect.right <= edge.right + 1 &&
            row.scrollWidth <= row.clientWidth + 1 && style.backgroundColor === transparent &&
            style.borderTopColor === transparent &&
            (index === 0 ? style.textDecorationLine === 'none' : rows[index - 1].getBoundingClientRect().bottom <= rect.top);
        });
        const palette = getComputedStyle(document.documentElement).getPropertyValue('--vk-primary-container').trim();
        const colors = document.createElement('canvas').getContext('2d');
        const color = value => { colors.fillStyle = value; return colors.fillStyle; };
        const choicesFit = [...region.querySelectorAll('fieldset[role=radiogroup]')].every(group => {
          const options = [...group.querySelectorAll('label')];
          return group.querySelectorAll('input:checked').length === 1 && options.every(option => {
            const rect = option.getBoundingClientRect();
            const selected = Boolean(option.querySelector(':checked'));
            return rect.width >= 48 && rect.height >= 48 && rect.left >= edge.left && rect.right <= edge.right + 1 &&
              option.scrollWidth <= option.clientWidth + 1 &&
              (color(getComputedStyle(option).backgroundColor) === color(palette)) === selected &&
              (getComputedStyle(option, '::after').opacity === '1') === selected;
          });
        });
        return { settingsFit, axisDelta: Math.max(...axes) - Math.min(...axes),
          choicesFit, overflow: Math.max(0, document.documentElement.scrollWidth - innerWidth) };
      })()`);
      check(settings.settingsFit && settings.axisDelta <= 1 && settings.choicesFit && settings.overflow <= 1,
        `#602 ${width}px/${scale}% ${theme}: flat settings and native segmented targets fit (${JSON.stringify(settings)})`);
      const capture = { textScale: scale, textScaleLane: scale === 200
        ? 'dom-override-geometry-only' : scale === 150 ? 'app-appearance-control' : 'default-100' };
      for (const [region, selector] of [['settings', '#account-settings-title'], ['appearance', '#page-appearance-title']]) {
        await page.eval(`document.querySelector('${selector}').scrollIntoView({block:'start', behavior:'instant'})`);
        await page.capture(`account-${region}-ssr-${width}-${theme}-${scale}`, { ...capture, requiredTargets: [selector] });
      }
    }
  }
  await page.eval("document.documentElement.style.fontSize = ''");
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  check(nodes.some(node => node.role?.value === 'link' && node.name?.value === 'Quyền riêng tư') &&
    ['Chế độ màu', 'Cỡ chữ', 'Thanh điều hướng'].every(name => nodes.some(node =>
      node.role?.value === 'radiogroup' && node.name?.value === name &&
        (name !== 'Thanh điều hướng' || node.description?.value?.includes('Trên màn hình vừa')))),
    '#602 AX exposes the privacy link and each named radio group, including the rail description');
  await page.eval("document.querySelector('#account-settings-title').setAttribute('tabindex', '-1'); document.querySelector('#account-settings-title').focus()");
  await page.key('Tab');
  check(await page.eval("document.activeElement?.id === 'profile-privacy-link' && getComputedStyle(document.activeElement).outlineStyle === 'solid'"),
    '#602 Tab reaches the full privacy row with a visible focus ring');
  await page.eval("document.querySelector('#page-theme input:checked').focus()");
  await page.key('ArrowRight');
  await page.waitFor("document.querySelector('#page-theme input:checked')?.value === 'system'", '#602 native Arrow changes the checked theme');
  await page.key('Tab');
  check(await page.eval("document.activeElement?.matches('#page-text input:checked')"),
    '#602 Tab moves from the selected theme to the selected text size');
  await chooseAppearance(page, 'theme', 'dark');
  await chooseAppearance(page, 'text', 150);
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
  const beforeLogout = (await account(page)).body;
  check(beforeLogout.principal_id === A.principal_id, 'fault 503 logout: account still valid and displayed');
  const desktop = await page.eval('({ width: innerWidth, height: innerHeight })');
  await page.viewport(390, 844);
  await page.waitFor(`${q(notificationTab)}?.checkVisibility()`, '#608 a valid Identity cookie offers the mobile Notifications link');
  const accepted = await notificationGateway(page, beforeLogout.notification_scope);
  check(accepted.current.noStore && [200, 503].includes(accepted.current.status),
    '#608 the real App Gateway accepts the scope before checking Notifications availability');
  await page.click(notificationTab);
  await page.waitFor(`${openInbox} && ${q(notificationTab)}?.getAttribute('aria-current') === 'page'`, '#608 valid session selects Inbox');
  await page.click('#bottom-navigation a[href="/toi"]');
  await page.waitFor(q('#sign-out'), 'Account is ready after leaving Inbox');
  const logoutPath = '/api/v1/auth/session';
  const logoutRequests = captureRequest(page, 'DELETE', logoutPath);
  const logoutResponse = await holdAnswer(page, 'DELETE', logoutPath);
  const logout = await snapshotAuthAction(page, '#sign-out');
  check(await page.eval("document.querySelector('#sign-out').classList.contains('vk-ui-button--text')"),
    '#625 Settings offers Text Logout');
  await page.click('#sign-out');
  check(await logoutResponse.held() === 204, 'real 204 confirms logout');
  await checkAuthPending(page, check, '#sign-out', logout, 'logout', 'Đăng xuất', 'sign-out');
  await page.key('Space');
  check(logoutRequests.count() === 1, '#625 click then Space during pending sends one logout');
  await logoutResponse.finish();
  logoutRequests.close();
  await page.waitFor(guestTabs, '#608 confirmed logout restores exactly the three public destinations');
  const revoked = await notificationGateway(page, beforeLogout.notification_scope);
  check(revoked.current.status === 401 && revoked.current.noStore, '#608 the real Gateway fences the scope after logout');
  await page.goto(base + '/thong-bao');
  await page.waitFor(`${openInbox} && ${guestTabs} && ${q('main a[href="/"] > svg[data-icon=explore]')}`,
    '#608 anonymous direct Inbox uses the shared NotFound link with leading Explore');
  await page.viewport(desktop.width, desktop.height);
  await openAccount(page, base, 'anonymous');
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
  const replaced = await notificationGateway(page, returnedA.notification_scope, beforeLogout.notification_scope);
  check(returnedA.notification_scope !== beforeLogout.notification_scope &&
    [200, 503].includes(replaced.current.status) && replaced.earlier.status === 401 && replaced.earlier.noStore,
    '#608 the real Gateway rejects the earlier scope even after re-login to the same principal');
  check(returnedA.principal_id === A.principal_id && returnedA.display_name === penA &&
    bodyText(await page.draft(draftA)) === DRAFT_TEXT, 'password returns to the same principal, pen name and stored draft');

  console.log('Auth password: read-only verified email and OTP continuity:');
  await openAccount(page, base, 'active');
  await page.waitFor(`${q('#account-email')}?.textContent.includes(${JSON.stringify(registrationEmail)})`, 'settings show the verified email');
  const binding = await read(page, '/api/v1/auth/email');
  check(binding.status === 200 && binding.cache === 'no-store' && binding.data.verified_email === registrationEmail &&
    binding.data.principal_id === A.principal_id, 'private email is bound to the signed-in principal');
  await signOut(page, base);
  await signIn(page, ctx, registrationEmail);
  check((await account(page)).body.principal_id === A.principal_id && bodyText(await page.draft(draftA)) === DRAFT_TEXT &&
    await page.eval(`!${q('#pen-name-form')}`), '#327/#329 sign-in with the registration mailbox returns to the same writer and saved draft, without onboarding');

  console.log('Auth password: email account adds a credential without changing its session:');
  const { browserContextId } = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
  const other = await openPage(browserContextId);
  await openAccount(other, base, 'anonymous');
  await signIn(other, ctx, email);
  const B = await setPenName(other, penB);
  const wrongAccount = await notificationGateway(other, B.notification_scope, returnedA.notification_scope);
  check([200, 503].includes(wrongAccount.current.status) && wrongAccount.earlier.status === 401 && wrongAccount.earlier.noStore,
    '#608 the real Gateway rejects account A scope under account B cookie');
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
  await other.waitFor(`${q('#account-email')}?.textContent.includes(${JSON.stringify(email)})`,
    'email-only and enrolled accounts show the same verified mailbox');
  check(await other.eval(`!${q('#account-email button')}`), 'the enrolled account shows a read-only address');
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
    base, check, openPage, other, userA, principalA: A.principal_id, principalB: B.principal_id,
  });
  await authorFollows(page, other, {
    base, openPage, check, holdAnswer, textContrast, switchActor, chooseAppearance,
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
  const login = await snapshotAuthAction(page, LOGIN + '-submit');
  await page.click(LOGIN + '-submit');
  check(await lostLogin.held() === 200, 'the server issued a session before the login ACK is lost');
  await checkAuthPending(page, check, LOGIN + '-submit', login, 'lock', 'Đăng nhập', 'password-login-submit');
  await page.key('Enter');
  await lostLogin.finish(true);
  await page.waitFor(`${STANDING} === 'active' || (${q('#password-login-retry')} && ${q('#password-login-retry')}.getAttribute('aria-disabled') !== 'true')`, 'login readback finished');
  const actual = (await read(page, '/api/v1/auth/session')).data;
  check(loginLoss.count() === 1 && (actual.state === 'active' && actual.principal_id === A.principal_id ||
    actual.state === 'anonymous' && await page.eval(`${q('#password-login-retry')}.getAttribute('aria-disabled') !== 'true'`)),
    'lost login ACK follows the actual cookie and never replays the mutation');
  loginLoss.close();
  if (actual.state === 'active') await signOut(page, base);
  await page.click('#use-registration');
  await registrationProof(page, args.outbox, 'collision@example.test', ctx.seen);
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
  const deletePath = '/toi?quan-ly=xoa';
  const ordinary = async label => check(await page.eval(
    "!document.querySelector('#open-account-deletion, #delete-account-form')"), label);
  const goToDeletion = async selector => {
    await page.click(selector);
    await page.waitFor(`${q('#page-account-deletion')} && ${q('#open-account-deletion')} && !${q('.vk-auth-entry')}`, 'dedicated account deletion view');
  };
  await openAccount(page, base, 'anonymous');
  await ordinary('guest Login has no destructive control');
  const direct = await fetch(base + deletePath);
  const html = await direct.text();
  check(direct.ok && html.includes('id="page-account-deletion"') && !html.includes('id="password-login"'),
    'signed-out SSR deep link renders the dedicated page');
  await goToDeletion('#account-management-help');
  await page.click('#open-account-deletion');
  await page.waitFor("document.activeElement?.id === 'account-deletion-title'", 'deletion warning owns focus');
  await page.eval(`${q('#delete-account-form')}.querySelectorAll('button').forEach(button => { if (button.textContent.trim() === 'Hủy') button.focus(); })`);
  await page.key('Enter');
  await page.waitFor(`!${q('#delete-account-form')} && document.activeElement?.id === 'open-account-deletion'`, 'Cancel dismisses proof and restores focus');
  check((await read(page, '/api/v1/auth/session')).data.state === 'anonymous', 'dedicated guest page grants no session');
  await page.click('#page-account-deletion .vk-ui-back-link');
  await page.waitFor(`${q(LOGIN)} && !location.search`, 'dedicated Back returns to Auth');
  await page.click('#use-registration');
  await ordinary('guest Registration has no destructive control');
  await registrationProof(page, args.outbox, randomUUID() + '@example.test');
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
    await ordinary('Active settings shows a link, without mounting proof controls');
    check(await page.eval("Boolean(document.querySelector('#account-settings-title').parentElement.querySelector('#account-deletion-settings-link'))"),
      'Active deletion entry belongs to the settings section');
    await goToDeletion('#account-deletion-settings-link');
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
  await page.waitFor(q(LOGIN), 'Browser Back returns from the dedicated mode to Account');
  await page.eval('history.back()');
  await page.waitFor(q('[data-testid=reader-author-deleted]'), 'Back from Account loads the neutral Reader replacement');
  check(await page.eval(`!document.querySelector('.vk-document') && !document.body.innerText.includes(${JSON.stringify(pen)})`),
    'the replacement has no original text or author name');
  const old = await fetch(`${base}/@${handle}`, { redirect: 'manual' });
  check(old.status === 410 && !old.headers.has('location'), 'the earlier Handle stays gone');

  await openAccount(page, base, 'anonymous');
  await page.click('#use-registration');
  await registrationProof(page, args.outbox, randomUUID() + '@example.test');
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
  await ordinary('Suspended Account has an account support link only');
  await goToDeletion('#account-management-help');
  await page.click('#open-account-deletion');
  await page.click('#account-delete-with-email');
  await fill(page, '#account-delete-email', email);
  await page.click('#send-account-deletion-code');
  await page.waitFor(q('#account-delete-code'), 'dedicated email OTP step');
  await fill(page, '#account-delete-code', await mailedCode(args.outbox, email, seen));
  const answer = await holdAnswer(page, 'POST', '/api/v1/auth/account-deletion/confirm');
  await page.click('#confirm-account-deletion-code');
  check([200, 202].includes(await answer.held()), 'real email OTP was accepted for the suspended account');
  await answer.finish(true);
  await page.waitFor(`${STANDING} === 'anonymous' && ${contains('Chưa nhận được kết quả xóa tài khoản')}`,
    'lost confirmation ACK keeps a status receipt on the dedicated page after sign-out');
  await page.click('#check-account-deletion');
  await page.waitFor(contains('Tài khoản đã được xóa vĩnh viễn'), 'status resolves the real OTP deletion without issuing a session');
  check((await read(page, '/api/v1/auth/session')).data.state === 'anonymous', 'status check did not mint a session');
  await page.click('#page-account-deletion .vk-ui-back-link');
  await page.waitFor(q(LOGIN), 'return to ordinary Auth to register a new principal');
  await signIn(page, { args, seen }, email);
  await setPenName(page, 'Người dùng email mới');
  check((await read(page, '/api/v1/auth/session')).data.principal_id !== suspended,
    'the same email proves a different principal after deletion');
}

// Same Chrome driver and real password/editor journeys against the standalone host.
export async function standalone(page, {args, check, openPage}) {
  const base = args.base;
  await page.goto(base + '/toi?quan-ly=xoa');
  await page.waitFor(`${STANDING} === 'anonymous' && ${q('#open-account-deletion')}`,
    'CSR resolves the dedicated deletion deep link as a guest');
  check(await page.eval(`!${q('.vk-auth-entry')} && !${q('#delete-account-form')}`),
    'CSR dedicated entry waits for explicit consent before showing the form');
  await page.click('#page-account-deletion .vk-ui-back-link');
  await page.waitFor(`${q(LOGIN)} && !location.search`, 'CSR Back reaches ordinary Auth');
  check(await page.eval(`!${q('#open-account-deletion')} && !${q('#delete-account-form')}`),
    'CSR ordinary Auth contains no deletion UI');
  const work = '33333333-3333-4333-8333-333333333333';
  const reader = base + '/tac-pham/' + work;
  const line = 'Phố cũ mưa giăng sợi mỏng,';
  const documentResponse = await fetch(reader, {headers: {accept:'text/html'}});
  const html = await documentResponse.text();
  check(documentResponse.ok && html.includes('<noscript>') && !html.includes(line),
    'direct Reader response is the static shell with the no-JS notice');
  await page.send('Emulation.setTimezoneOverride',{timezoneId:'Pacific/Kiritimati'});
  const pinned = await page.send('Page.addScriptToEvaluateOnNewDocument',{source:"Date.now = () => Date.parse('2099-01-01T00:00:00Z')"});
  await page.goto(reader);
  await page.waitFor(`document.querySelector('.vk-work-byline__date time')?.title.includes('GMT+14:00')`, 'standalone viewer-local time');
  check(await page.eval(`(() => {
    const t=document.querySelector('.vk-work-byline__date time');
    return t.textContent===new Intl.DateTimeFormat('vi-VN',{timeZone:'Pacific/Kiritimati'}).format(new Date(t.dateTime));
  })()`), 'CSR renders the Reader publication instant in the pinned viewer calendar');
  await page.send('Page.removeScriptToEvaluateOnNewDocument',{identifier:pinned.identifier});
  await page.send('Emulation.setTimezoneOverride',{timezoneId:''});
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

  await page.goto(base + '/ban-viet');
  await page.waitFor(q('.vk-desk-access--guest'), '#612 standalone resolves the guest Desk');
  check(await page.eval(`Boolean(document.querySelector('#desk-sign-in.vk-ui-button--filled')) &&
    Boolean(document.querySelector('.vk-desk-access--guest a.vk-ui-button--tonal > svg[data-icon=explore]:first-child')) &&
    !performance.getEntriesByType('resource').some(e =>
      ['/api/v1/author/works', '/api/v1/drafts'].includes(new URL(e.name).pathname))`),
    '#612 standalone guest actions render without any private list request');
  await page.click('#desk-sign-in');
  await page.waitFor(`location.pathname === '/toi' && new URLSearchParams(location.search).get('next') === '/ban-viet' && ${q(LOGIN)}`,
    '#612 standalone guest sign-in preserves the Desk return target');

  await seedLegacyAppearance(page);
  await firstPaintAppearance(page, base, check, 'csr');
  await page.goto(base + '/toi');
  await page.waitFor(q('#use-registration'), 'anonymous password form');
  await expectDefaultAppearance(page, check, 'CSR guest after mount');
  await checkAuthR31(page, check, 'standalone /toi');
  await checkModeSwitches(page, check, 'standalone /toi');
  await page.click('#use-registration');
  // Hold a real resend after the server queues it. Going back keeps the new address step
  // even when the previous mailbox's response reaches the still-mounted registration form.
  await registrationProof(page, args.outbox, randomUUID() + '@example.test');
  await page.waitFor("document.querySelector('#registration-resend')?.getAttribute('aria-disabled') !== 'true' && !document.querySelector('#registration-resend')?.disabled",
    'registration resend cooldown elapsed', 75000);
  const resend = await holdAnswer(page, 'POST', API + '/register/challenges');
  await page.click('#registration-resend');
  check(await resend.held() === 202, 'Identity accepted a second registration mail request');
  await page.click('#registration-back');
  await page.waitFor(q('#registration-address'), 'choose another mailbox while resend is pending');
  await resend.finish();
  await page.eval('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
  check(await page.eval("Boolean(document.querySelector('#registration-address')) && !document.querySelector('#registration-code')"),
    'the delayed answer cannot reopen the abandoned mailbox');
  await page.goto(base + '/toi');
  await page.waitFor(q('#use-registration'), 'reload leaves the unverified browser a guest');
  await page.click('#use-registration');
  const username = 'spa' + Date.now().toString(36);
  await registrationProof(page, args.outbox, randomUUID() + '@example.test');
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
  await page.click('#profile-privacy-link');
  await page.waitFor("document.activeElement?.id === 'quyen-rieng-tu'", 'CSR opens and focuses the privacy deep link');
  check(await page.eval("document.querySelector('#profile-back')?.getAttribute('href') === '/toi#edit-profile-link' && " +
    "Boolean(document.querySelector('.vk-profile-form select.vk-ui-select--icon-tonal'))"),
    'CSR mounts the profile and its actual icon-capable select');
  await page.eval("document.querySelector('#profile-bio-policy').focus()");
  await page.key('Space');
  await page.waitFor("document.querySelector('#profile-bio-policy').matches(':open')", 'CSR privacy picker open');
  await page.key('ArrowDown');
  await page.key('Enter');
  await page.waitFor("document.activeElement?.id==='profile-bio-policy-cancel'", 'CSR Cancel receives focus');
  const ax=await page.send('Accessibility.getFullAXTree');
  check(ax.nodes.some(node=>node.role?.value==='button' && node.name?.value==='Hủy' &&
    node.description?.value?.includes('chưa thể công khai lại')), '#609 AX exposes the consequence at the focused button');
  await page.key('Escape');
  check((await read(page,'/api/v1/me/profile')).data.profile_version===author.data.profile_version,
    '#609 Escape sends no privacy change in CSR');
  await fill(page, '#profile-bio', 'Giới thiệu viết trên CSR');
  check((await read(page,'/api/v1/me/profile')).data.bio===author.data.bio &&
    !await page.eval("Boolean(document.querySelector('#profile-save'))"),
    '#609 standalone keeps unfinished bio local');
  await page.key('Enter', 2);
  await page.waitFor("document.querySelector('#profile-bio-feedback')?.textContent.includes('Đã lưu')",
    'CSR confirms the inline bio');
  const savedProfile=(await read(page,'/api/v1/me/profile')).data;
  check(savedProfile.bio==='Giới thiệu viết trên CSR' && savedProfile.profile_version===author.data.profile_version+1 &&
    savedProfile.pen_name===author.data.pen_name, '#609 standalone commits only bio with the next version');
  const heldBio=await holdAnswer(page,'PUT','/api/v1/me/profile');
  await fill(page,'#profile-bio','Giữ phản hồi');
  await page.key('Enter',2);
  check(await heldBio.held()===200,'CSR holds the actual bio ACK');
  const peer=await openPage();
  await peer.goto(base+'/toi/chinh-ho-so');
  await peer.waitFor(`${q('#profile-bio')}?.value==='Giữ phản hồi'`,'peer reads committed bio');
  await fill(peer,'#profile-pen-name','Bút danh ở tab khác');
  await peer.key('Enter');
  await peer.waitFor(`${q('#profile-pen-name-feedback')}?.textContent.includes('Đã lưu')`,'peer committed its name while bio ACK was delayed');
  await heldBio.finish();
  await page.waitFor(`${q('#profile-pen-name')}?.value==='Bút danh ở tab khác' && ${q('#profile-bio')}?.value==='Giữ phản hồi'`,
    'broadcast received before a late ACK still refreshes the untouched field');
  await page.eval("document.querySelector('#profile-bio-policy').focus()");
  await page.key('Space');
  await page.waitFor("document.querySelector('#profile-bio-policy').matches(':open')",'CSR reopens privacy picker');
  await page.key('ArrowDown');
  await page.key('Enter');
  await page.click('#profile-bio-policy-confirm');
  await page.waitFor("document.activeElement?.id==='profile-bio-policy-row' && !document.querySelector('select#profile-bio-policy')",
    'confirmation retains focus at the newly locked privacy row');
  await page.click('#profile-back');
  await page.waitFor("document.activeElement?.id === 'edit-profile-link'", 'CSR Back restores the account entry');
  const authorPath = '/@' + author.data.handle;
  const followingPath = authorPath + '/dang-theo-doi';
  await page.goto(base + followingPath);
  await page.waitFor(`${q('#relationship-empty img')} && ${q('#relationship-count')}?.textContent === '0'`,
    'CSR deep link loads the confirmed owner empty list from the App');
  check(await page.eval(`${q('#rel-back-profile')}?.getAttribute('href') === ${JSON.stringify(authorPath)} &&
    ${q('#rel-other-direction')}?.getAttribute('href') === ${JSON.stringify(authorPath + '/nguoi-theo-doi')}`),
    'CSR owner projection shows the two canonical destinations');
  await page.click('#rel-back-profile');
  await page.waitFor(`location.pathname === ${JSON.stringify(authorPath)} &&
    document.activeElement?.id === 'author-profile-heading'`, 'CSR shared Back focuses the canonical profile');
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
  await registrationProof(page, args.outbox, randomUUID() + '@example.test');
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
  await page.goto(base + followingPath);
  await page.waitFor(q('#relationship-private'), 'CSR second member cannot open the owner’s private Following');
  check(await page.eval(`!${q('#relationship-count, #relationship-empty, #rel-other-direction')}`),
    'CSR denied list does not expose a count, empty illustration or alternative direction');
  const { browserContextId } = await page.cdp.send('Target.createBrowserContext');
  try {
    const owner = await openPage(browserContextId);
    await owner.goto(base + '/toi');
    await owner.waitFor(q(LOGIN + '-username'), 'native author sign-in');
    await credentials(owner, LOGIN, username, SECRET_A);
    await owner.click(LOGIN + '-submit');
    await owner.waitFor(`${STANDING} === 'active'`, 'independent native author session');
    await page.send('Page.bringToFront');
    await nativeAuthorMetrics(owner, page, {base, check});
  } finally {
    await page.cdp.send('Target.disposeBrowserContext', { browserContextId });
  }
  await page.send('Page.bringToFront');
  await standaloneWorkViewCheck(page, { args, check });
  // #388 starts signed out and measures baseline text in this disposable Chrome profile.
  // The appearance checks above deliberately saved settings for one of its readers.
  await page.eval(`for (const key of Object.keys(localStorage)) {
    if (key.startsWith('vink.ui.member.')) localStorage.removeItem(key);
  }`);
  await readerSave(page, { args, check, openPage,
    reader: { username: username + 'b', password: SECRET_B },
    other: { username, password: SECRET_A } });
}
