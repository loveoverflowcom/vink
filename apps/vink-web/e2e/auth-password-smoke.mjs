// AUTH-PASSWORD-WEB-01: real Chrome, App, native Identity, separate scratch PostgreSQL.
// The mail carrier is a local outbox. Faults delay/drop a real response or explicitly inject
// a refusal; no synthetic success is used. Don't log request bodies, credentials or cookies.

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
    const reads = captureRequest(desk, 'GET', '/api/v1/author/works');
    await desk.goto(base+'/ban-viet');
    await desk.waitFor(q('[data-testid=draft-row]'), '#308 active scope loads its own draft');
    check(reads.count()===1, '#308 active scope starts exactly one initial read');
    const late = await holdAnswer(desk, 'GET', '/api/v1/author/works');
    await desk.send('Network.deleteCookies', {name:sessionCookie, url:base});
    await wakeDesk(desk);
    check(await late.held()===401, '#308 holding the real 401 from a private refresh after session loss');
    await late.finish();
    await desk.waitFor(q('#desk-sign-in'), '#308 late 401 reconciles through the session owner');
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
    login: ['#password-login-submit', '#use-registration', '#use-email-code'],
    registration: ['#password-register-submit', '#use-password', '#use-email-code'],
    email: ['#sign-in-send', '#use-password'],
    'pen-name': ['#pen-name-save'],
    closed: [],
  })[scenario];
  const r2 = actions !== undefined;
  const boundary = scenario === 'closed' ? 'App (sign-in disabled) + scratch PostgreSQL' :
    'native Identity + App + scratch PostgreSQL';
  const widths = r2 ? [320, 390, 834, 1280] : [320, 390, 1280];
  const scales = r2 ? [100, 150, 200] : [100, 200];
  const initial = await page.eval(`({
    theme: document.querySelector('#page-theme input:checked')?.value,
    text: document.querySelector('#page-text input:checked')?.value,
    themeAttribute: document.documentElement.getAttribute('data-theme'),
  })`);
  for (const width of widths) for (const theme of ['light', 'dark']) for (const scale of scales) {
    await page.viewport(width, width === 834 ? 600 : 900);
    await page.eval("document.documentElement.style.fontSize = ''");
    if (r2) {
      await chooseAppearance(page, 'theme', theme);
      await chooseAppearance(page, 'text', scale === 200 ? 150 : scale);
      await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 });
    }
    await page.waitFor(APPEARANCE_SETTLED, 'Auth appearance colours settle');
    const endTarget = actions?.at(-1) ?? anchor;
    const measured = await page.eval(`(async () => {
      if (!${r2}) document.documentElement.dataset.theme = '${theme}';
      if (!${r2} || ${scale} === 200) document.documentElement.style.fontSize = '${scale}%';
      const panel = ${q(PANEL)}, section = document.querySelector(${path});
      if (${r2}) scrollTo({top: 0, behavior: 'instant'});
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
      const appearance = document.getElementById('auth-appearance');
      let link = null;
      if (${r2}) {
        const style = getComputedStyle(appearance), bounds = appearance.getBoundingClientRect();
        const icon = appearance.querySelector('[data-appearance-icon="palette"]');
        const iconRect = icon?.getBoundingClientRect();
        const label = appearance.querySelector('.vk-ui-button__label');
        const labelRect = label?.getBoundingClientRect();
        const auth = entry.getBoundingClientRect();
        const account = panel.getBoundingClientRect();
        const colors = document.createElement('canvas').getContext('2d');
        colors.fillStyle = style.color;
        const foreground = colors.fillStyle;
        colors.fillStyle = style.getPropertyValue('--vk-primary').trim();
        link = {
          native: appearance.matches('a[href="#page-appearance"][rel=external]') && appearance.tabIndex === 0 &&
            appearance.textContent.trim() === 'Giao diện' && !appearance.hasAttribute('role') &&
            !appearance.hasAttribute('aria-expanded') && !appearance.hasAttribute('aria-haspopup') &&
            document.querySelectorAll('a[href="#page-appearance"]').length === 1 &&
            document.querySelectorAll('#page-appearance').length === 1 &&
            !document.querySelector('#profile-trigger, #profile-menu, .vk-ui-nav-utility'),
          icon: icon?.getAttribute('aria-hidden') === 'true' && getComputedStyle(icon).maskImage !== 'none' &&
            iconRect.width === 24 && iconRect.height === 24 && label?.checkVisibility() && iconRect.right <= labelRect.left,
          idle: style.backgroundColor === 'rgba(0, 0, 0, 0)' && style.textDecorationLine === 'none' &&
            style.borderTopWidth === '0px' && style.boxShadow === 'none' &&
            getComputedStyle(appearance, '::after').opacity === '0',
          primary: colors.fillStyle === foreground,
          contrast: contrast(appearance),
          width: bounds.width, height: bounds.height,
          placed: bounds.left >= -1 && bounds.right <= innerWidth + 1 &&
            (innerWidth < 760 ? bounds.top >= auth.bottom - 1 :
              bounds.bottom <= auth.top + 1 && bounds.top >= account.top && Math.abs(bounds.right - account.right) <= 1),
          inView: bounds.top >= 0 && bounds.bottom <= barTop + 1,
        };
      }
      return { logoAsset, logoVisible: Boolean(logo?.checkVisibility()), singleColumn, ordered,
        minContrast: contrasts.length ? Math.min(...contrasts) : null, ok: !!rect.width && !clipped && document.documentElement.scrollWidth <= innerWidth + 1 && font,
        width: rect.width, left: rect.left, font, device: devicePixelRatio, link,
        rootSize: getComputedStyle(document.documentElement).fontSize,
        focus: document.activeElement?.id,
        partial: (target?.getBoundingClientRect().bottom ?? rect.bottom) > barTop };
    })()`);
    check(measured.ok, `${scenario} at ${width}px / ${theme} / text ${scale}%: no horizontal clipping, Be Vietnam Pro loaded (${JSON.stringify(measured)})`);
    if (measured.minContrast !== null) check(measured.minContrast >= 4.5, `${scenario}: measured label/help/error contrast ≥ 4.5:1`);
    if (r2) check(measured.logoVisible && measured.logoAsset.includes(`/brand/vpen-r1-lockup-${theme}.svg`) &&
      measured.singleColumn && measured.ordered, `#327 ${scenario} ${width}/${theme}/${scale}: theme asset and one-column reading order`);
    if (r2) {
      const link = measured.link;
      check(measured.rootSize === `${16 * scale / 100}px` && link.native && link.icon && link.idle &&
        link.primary && link.contrast >= 4.5 && link.width >= 48 && link.height >= 48 && link.placed,
        `#483 ${scenario} ${width}/${theme}/${scale}: native Appearance text action, placement and ≥4.5:1 contrast (${JSON.stringify(link)})`);
    }
    const textScaleLane = r2 && scale !== 200 ? 'app-appearance-control' : 'dom-override-geometry-only';
    if (scenario === 'login' || (width === 390 && scale === 100) || (width === 320 && scale === 200 && theme === 'dark') ||
        (width === 1280 && scale === 100 && theme === 'light')) {
      await page.capture(`auth-password-${scenario}-${width}-${theme}-${scale}`, {
        boundary,
        requiredTargets: measured.link?.inView ? ['#auth-appearance'] : [],
        scenario, textScale: scale, textScaleLane, font: 'Be Vietnam Pro', ...measured,
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
    if (r2) {
      check(await page.eval(`(() => {
        const nav = document.querySelector('#bottom-navigation'), barTop = nav?.getClientRects().length ? nav.getBoundingClientRect().top : innerHeight;
        const targets = ${JSON.stringify(width <= 390 && scale === 200 ? [...actions, '#auth-appearance'] : ['#auth-appearance'])};
        return targets.every(sel => {
          const action = document.querySelector(sel);
          action.scrollIntoView({block:'center',behavior:'instant'});
          const r = action.getBoundingClientRect();
          return r.top >= 0 && r.bottom <= barTop + 1 &&
            action.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2));
        });
      })()`), `#483 ${scenario} Appearance is reachable without navigation covering it: ${width}/${theme}/${scale}`);
    }
  }
  await page.eval("document.documentElement.style.fontSize = ''");
  if (r2) {
    await chooseAppearance(page, 'theme', initial.theme);
    await chooseAppearance(page, 'text', initial.text);
  } else {
    await page.eval(initial.themeAttribute === null
      ? "document.documentElement.removeAttribute('data-theme')"
      : `document.documentElement.dataset.theme = ${JSON.stringify(initial.themeAttribute)}`);
  }
  await page.viewport(1280);
  await page.eval("scrollTo({top:0,behavior:'instant'})");
}

async function appearanceFragment(page) {
  await page.waitFor(`location.hash === '#page-appearance' && document.activeElement?.id === 'page-appearance'`,
    '#483 native activation focuses the Appearance section');
}

async function probeAppearanceInput(page, selector, value) {
  await fill(page, selector, value);
  await page.eval(`(() => {
    const input = ${q(selector)};
    const probe = window.appearanceProbe = { input, value: ${JSON.stringify(value)}, submitted: false };
    input.form.addEventListener('submit', () => { probe.submitted = true; }, { once: true });
  })()`);
}

async function preservedAppearanceInput(page, selector, check, method) {
  check(await page.eval(`(() => {
    const probe = window.appearanceProbe;
    const input = ${q(selector)};
    const section = document.getElementById('page-appearance').getBoundingClientRect();
    return input === probe?.input && input.value === probe.value && !probe.submitted &&
      section.top >= -1 && section.top < innerHeight && !document.querySelector('dialog[open]');
  })()`), `#483 ${method} scrolls in the same document without submitting or remounting the typed field`);
  await page.eval(`history.replaceState(history.state, '', location.pathname + location.search)`);
  await fill(page, selector, '');
}

async function appearancePoint(page) {
  return page.eval(`(() => {
    const link = ${q('#auth-appearance')};
    link.scrollIntoView({ block: 'center', behavior: 'instant' });
    const r = link.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  })()`);
}

async function pointerAppearance(page, check) {
  const field = REGISTER + '-username';
  await probeAppearanceInput(page, field, 'tam-thoi');
  const point = await appearancePoint(page);
  const opacity = `getComputedStyle(${q('#auth-appearance')}, '::after').opacity`;
  await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point });
  await page.waitFor(`${opacity} === '0.08'`, '#483 hover state layer');
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
  await page.waitFor(`${opacity} === '0.12'`, '#483 pressed state layer');
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
  await appearanceFragment(page);
  await preservedAppearanceInput(page, field, check, 'pointer');
}

async function emailAppearance(page, check) {
  await page.click('#use-email-code');
  await page.waitFor(`document.activeElement?.id === 'sign-in-address'`, 'email mode focuses its input');
  await matrix(page, 'email', '#sign-in-email', check);
  await page.viewport(390);
  await probeAppearanceInput(page, '#sign-in-address', 'thu@vi-du.vn');
  const point = await appearancePoint(page);
  await page.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [point] });
  await page.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await appearanceFragment(page);
  await preservedAppearanceInput(page, '#sign-in-address', check, 'touch');
  await page.click('#use-password');
  await page.waitFor(q(LOGIN), 'password mode after email');
  await page.viewport(1280);
}

const STORED_APPEARANCE = `document.documentElement.dataset.theme === 'dark' &&
  document.documentElement.dataset.text === '150' &&
  localStorage.getItem('vink.ui.theme') === 'dark' && localStorage.getItem('vink.ui.text') === '150'`;

async function appearanceReload(page, check) {
  await chooseAppearance(page, 'theme', 'dark');
  await chooseAppearance(page, 'text', 150);
  check(await page.eval(STORED_APPEARANCE), '#483 Appearance choices apply and persist from the Auth host');
  await page.eval("localStorage.setItem('vink.ui.text', '200')");
  const { identifier } = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `
    const observer = new MutationObserver(() => {
      if (document.documentElement?.dataset.text !== '150') return;
      window.appearanceBeforeHydration = document.documentElement.dataset.hydrated !== 'true' &&
        localStorage.getItem('vink.ui.text') === '150';
      observer.disconnect();
    });
    observer.observe(document, { subtree: true, attributes: true, attributeFilter: ['data-text'] });
  ` });
  await page.reload();
  await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier });
  await page.waitFor(q(LOGIN), 'guest form after Appearance reload');
  check(await page.eval(`window.appearanceBeforeHydration && ${STORED_APPEARANCE} &&
    document.querySelector('#page-text input:checked')?.value === '150' &&
    document.querySelector('#page-theme input:checked')?.value === 'dark'`),
    '#483 legacy 200 normalizes to 150 before hydration and the choices retain dark and 150 after reload');
}

export async function closedAccountAppearance(page, { args, check }) {
  await page.goto(args.base + '/toi');
  await page.waitFor(`${HYDRATED} && ${q('#sign-in-closed')} && ${q('#auth-appearance')}`, 'closed sign-in on Tôi');
  check(await page.eval(`!${q('#password-login, #password-register, #sign-in-email, #pen-name-form')} &&
    ${q('#sign-in-closed')}.textContent.includes('Vink chưa mở đăng nhập')`), '#327 closed gate has no Auth form');
  await matrix(page, 'closed', '#sign-in-closed', check);
  await page.eval(`${q('#auth-appearance')}.focus()`);
  await page.key('Enter');
  await appearanceFragment(page);
}

async function authR2Login(page, check) {
  check(await page.eval(`(() => {
    const entry = document.querySelector('.vk-auth-entry'), form = ${q(LOGIN)};
    return entry?.contains(form) && entry.querySelectorAll('form').length === 1
      && !document.querySelector('#page-account .vk-account-status, #pen-name-form')
      && !form.querySelector('a[href*="quen"], [role=tablist]')
      && document.activeElement !== form.querySelector('input')
      && ${q('#auth-appearance')}.getAttribute('href') === '#page-appearance'
      && ${q('#page-appearance-title')}.textContent.trim() === 'Giao diện'
      && ${q(LOGIN + '-submit')}.textContent.trim() === 'Đăng nhập'
      && ${q('#use-registration')}.textContent.trim() === 'Tạo tài khoản'
      && ${q('#use-email-code')}.textContent.trim() === 'Dùng mã email';
  })()`), '#327 login renders one R2 form, without the old guest status card, a premature pen-name step, duplicated actions, or autofocus');
  await matrix(page, 'login', LOGIN, check);
  await probeAppearanceInput(page, LOGIN + '-username', 'tam-thoi');
  for (const id of [LOGIN + '-password', LOGIN + '-password-reveal', LOGIN + '-submit', '#use-registration', '#use-email-code', '#auth-appearance']) {
    await page.key('Tab');
    check(await page.eval(`document.activeElement === ${q(id)} && document.activeElement.matches(':focus-visible')`), `#327 keyboard order and focus-visible: ${id}`);
  }
  await page.waitFor(`(() => {
    const link = ${q('#auth-appearance')}, style = getComputedStyle(link);
    return style.outlineStyle === 'solid' && parseFloat(style.outlineWidth) >= 2 &&
      getComputedStyle(link, '::after').opacity === '0.12';
  })()`, '#483 visible keyboard focus with the state layer');
  await page.key('Enter');
  await appearanceFragment(page);
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('#page-theme input[type=radio]:focus-visible')`),
    '#327 the next Tab reaches an Appearance radio with visible keyboard focus');
  await preservedAppearanceInput(page, LOGIN + '-username', check, 'keyboard');
  await emailAppearance(page, check);
  await appearanceReload(page, check);
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

  await authR2Login(page, check);
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
  check(await page.eval(STORED_APPEARANCE), '#483 Appearance survives switching from login to registration');
  await matrix(page, 'registration', REGISTER, check);
  await pointerAppearance(page, check);
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
  const writerReview = await holdAnswer(page, 'HEAD', workQueuePath);
  await openAccount(page, base, 'active');
  check(await writerReview.held() === 403, 'F-18 the bodyless authorization check refuses an ordinary writer');
  await writerReview.finish(false, true);
  check(await page.eval(`${STORED_APPEARANCE} && !${q('#auth-appearance')} && !!${q('#page-appearance')}`),
    '#483 the same stored Appearance applies after real sign-in and pen-name completion');
  await chooseAppearance(page, 'theme', 'system');
  await chooseAppearance(page, 'text', 100);
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

// Same Chrome driver and real password/editor journeys against the standalone host.
export async function standalone(page, {args, check}) {
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
  await page.click('#retry-public');
  await page.waitFor(q('.vk-document'), 'retry obtains the current publication');

  await page.goto(base + '/the/' + encodeURIComponent('ký-ức'));
  await page.waitFor(`document.querySelector('main h1')?.textContent.includes('Ký ức')`, 'public tag deep link');
  await page.goto(base + '/@nguoi-chua-co');
  await page.waitFor(q('#profile-unavailable'), 'unknown author has a settled profile state');

  await page.goto(base + '/toi');
  await page.waitFor(q('#use-registration'), 'anonymous password form');
  await page.click('#use-registration');
  const username = 'spa' + Date.now().toString(36);
  await credentials(page, REGISTER, username, SECRET_A);
  await page.click(REGISTER + '-submit');
  await setPenName(page, 'Người viết SPA');
  const author = await read(page, '/api/v1/me/profile');
  check(author.status === 200 && Boolean(author.data?.handle), 'new author has a public handle');
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
  await page.waitFor(`document.querySelector('#publication')?.dataset.phase === 'reconciled' && document.querySelector('#publication-requested')`, 'pending review');
  const publication = await read(page, '/api/v1/works/' + id + '/publication/state');
  check(publication.data?.revisions.length === 1 && publication.data.revisions[0].moderation === 'pending' && !publication.data.pointer,
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
  await page.waitFor(q('#moderation-home'), 'moderation deep link');
  check((await read(page, '/api/v1/moderation/automation')).status === 403,
    'the ordinary account cannot read the administrator’s automation controls');
  await page.goto(base + '/toi');
  await page.waitFor(q('#sign-out'), 'signed-in account');
  await signOut(page, base);
  const result = await read(page, '/api/v1/drafts/' + id);
  check(result.status === 401, 'logout revokes the private draft read on the same origin');
  await page.waitFor(q('#use-registration'), 'account switch');
  await page.click('#use-registration');
  await credentials(page, REGISTER, username + 'b', SECRET_B);
  await page.click(REGISTER + '-submit');
  await setPenName(page, 'Bạn đọc SPA');
  check((await read(page, '/api/v1/drafts/' + id)).status === 404,
    'the second account cannot read the first account’s private draft');
  await page.goto(base + '/@' + encodeURIComponent(author.data.handle));
  await page.waitFor(`document.querySelector('#author-profile-heading')?.textContent.includes('Người viết SPA')`,
    'the second account can open the public author profile');
  check(!await page.eval("document.body.textContent.includes('Bản thảo trên Vink')"),
    'a pending work stays off the public author profile');
  await standaloneWorkViewCheck(page, { args, check });
}
