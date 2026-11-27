// #294 Social Follow Graph E2E: real Chrome, password sessions, App and scratch PostgreSQL.
import { randomUUID } from 'node:crypto';
import { authorFixtureSql } from './author-fixtures.mjs';
import { cookies, q } from './auth-smoke.mjs';
import { captureRequest } from './auth-browser-network.mjs';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const literal = (text) => "'" + String(text).replaceAll("'", "''") + "'";
const ownFollowing = '/api/v1/me/relationships/following';
const sessionCookie = '__Host-vink_session';
const ownProfile = (p) => p.eval("fetch('/api/v1/me/profile').then(r => r.json())");

async function checkLayoutAndA11y(page, check, textContrast, url, contrastSelectors, label) {
  // Audits switch between the guest and signed-in targets. Focus emulation lets
  // DOM checks run in a background tab, but PNG capture needs its compositor surface.
  await page.send('Page.bringToFront');
  await page.goto(url);
  await page.waitFor(`document.visibilityState === 'visible' && document.documentElement.dataset.hydrated === 'true' &&
    ${contrastSelectors.map(q).join(' && ')}`,
    `${label} visible with required targets`);
  for (const width of [320, 390, 834, 1024, 1440]) {
    for (const theme of ['light', 'dark']) {
      for (const scale of [100, 200]) {
        await page.viewport(width);
        const facts = await page.eval(`(async () => {
          const root = document.documentElement;
          root.dataset.theme = '${theme}';
          root.style.fontSize = '${scale}%';
          await document.fonts.ready;
          await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
          const overflow = document.documentElement.scrollWidth - window.innerWidth;
          const nestedLinks = document.querySelectorAll('a a').length;
          const targets = ${JSON.stringify(contrastSelectors)}
            .map((sel) => {
              const el = document.querySelector(sel);
              if (!el) return null;
              const r = el.getBoundingClientRect();
              return { sel, width: r.width, height: r.height };
            })
            .filter(Boolean);
          // #588 compact header: bold literary name on its own token, secondary handle, and the
          // permitted count beside the Follow action whenever the text column fits both.
          const heading = document.querySelector('.vk-author-identity h1');
          let header = null;
          if (heading) {
            const css = getComputedStyle(heading), rootPx = parseFloat(getComputedStyle(root).fontSize);
            const handle = heading.parentElement.querySelector('p');
            const count = document.querySelector('.vk-author-social :is(#profile-followers-link, #profile-followers-count)');
            const action = document.querySelector('.vk-author-social :is(#follow-action, #follow-sign-in)');
            const box = (el) => el?.getBoundingClientRect().toJSON();
            header = {
              ratio: parseFloat(css.fontSize) / rootPx, weight: Number(css.fontWeight),
              serif: css.fontFamily.includes('Literata'),
              clipped: heading.scrollWidth > heading.clientWidth + 1,
              handleRatio: handle ? parseFloat(getComputedStyle(handle).fontSize) / rootPx : null,
              wide: document.querySelector('.vk-author-public').clientWidth >= 40 * rootPx,
              count: box(count), action: box(action),
              actionText: action?.textContent.trim(), column: box(heading.parentElement),
              // The link is a flex container: one label item keeps the space after the number.
              countSpace: (() => {
                const num = count?.querySelector('.vk-rel-num');
                let text = num?.nextSibling;
                while (text && !(text.nodeType === Node.TEXT_NODE && text.textContent.trim())) text = text.nextSibling;
                if (!text) return null;
                const at = text.textContent.length - text.textContent.trimStart().length, r = document.createRange();
                r.setStart(text, at); r.setEnd(text, at + 1);
                return r.getBoundingClientRect().left - num.getBoundingClientRect().right;
              })(),
              stats: [...(document.querySelector('#profile-relationships')?.children ?? [])]
                .filter(el => el.textContent.trim()).length,
            };
          }
          return { overflow, nestedLinks, targets, header };
        })()`);
        if (facts.header) {
          const h = facts.header, at = `${label} header at ${width}px ${theme} ${scale}%`;
          const expected = h.wide ? 1.375 : 1.25;
          check(Math.abs(h.ratio - expected) < 0.01 && h.weight >= 700 && h.serif && !h.clipped,
            `${at}: name is bold Literata at ${expected}rem and wraps unclipped ${JSON.stringify(h)}`);
          check(h.handleRatio !== null && h.handleRatio <= 0.875 + 0.01 && h.handleRatio >= 0.8125 - 0.01,
            `${at}: handle stays secondary at 13–14px-equivalent (${h.handleRatio})`);
          if (h.count && h.action) {
            const sameRow = Math.abs((h.count.top + h.count.bottom) / 2 - (h.action.top + h.action.bottom) / 2) <= 2;
            const gap = h.action.left - h.count.right;
            // Two stats may wrap the action below them on a phone; one stat always fits.
            if (scale === 100 && (h.wide || (width >= 390 && h.stats === 1))) {
              check(sameRow && gap >= 8 && gap <= 24 && h.action.left >= h.column.left - 1,
                `${at}: count and Follow share one compact row in the text column ${JSON.stringify(h)}`);
            }
            check(h.countSpace === null || h.countSpace >= 2,
              `${at}: the rendered count keeps a space after the number (${h.countSpace}px)`);
            check(h.action.right <= width + 1 && h.count.right <= width + 1,
              `${at}: count and Follow stay inside the viewport ${JSON.stringify(h)}`);
          }
        }
        check(facts.overflow <= 1, `${label} zero overflow at ${width}px ${theme} ${scale}% (got ${facts.overflow})`);
        check(facts.nestedLinks === 0, `${label} zero nested links at ${width}px ${theme} ${scale}%`);
        for (const t of facts.targets) {
          check(
            t.width >= 44 && t.height >= 44,
            `${label} touch target ${t.sel} >= 44 × 44px at ${width}px ${theme} ${scale}% (got ${t.width}×${t.height})`
          );
        }
        if (width === 390 && scale === 100) {
          await page.waitFor(
            `!document.getAnimations().some(a => a.playState === 'running' &&
              a.effect?.target?.checkVisibility?.() && Number.isFinite(a.effect.getComputedTiming().endTime))`,
            `${label} ${theme} settled`
          );
          const text = label === 'follow-list' ? ['.vk-ui-byline__name', '.vk-ui-byline__handle'] :
            label === 'profile-read-unavailable' ? ['#profile-unavailable > h1', '#profile-unavailable > p'] : [];
          await textContrast(page, theme, label, [...contrastSelectors, ...text]);
          await page.key('Tab');
          const focus = await page.eval(`(() => {
            const luma = (raw) => {
              const rgb = raw.match(/[\\d.]+/g).map(Number).slice(0, 3);
              return rgb.reduce((v, x, i) => {
                const n = x / 255;
                return v + (n <= 0.04045 ? n / 12.92 : ((n + 0.055) / 1.055) ** 2.4) * [0.2126, 0.7152, 0.0722][i];
              }, 0);
            };
            const el = ${q(contrastSelectors[0])};
            el.focus();
            const css = getComputedStyle(el);
            let outside = el.parentElement;
            while (outside && ['transparent', 'rgba(0, 0, 0, 0)'].includes(getComputedStyle(outside).backgroundColor)) outside = outside.parentElement;
            const a = luma(css.outlineColor), b = luma(getComputedStyle(outside ?? document.documentElement).backgroundColor);
            return {visible: document.activeElement === el && css.outlineStyle !== 'none' && parseFloat(css.outlineWidth) > 0,
              contrast: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)};
          })()`);
          check(focus.visible && focus.contrast >= 3, `${label} focus ring at ${theme} has contrast >= 3:1 (${focus.contrast.toFixed(2)})`);
        }
        if (width <= 390 && ['follow-list', 'profile-read-unavailable'].includes(label)) {
          const last = label === 'follow-list' ? '#rel-load-more' : '#profile-retry';
          const clearance = await page.eval(`(async () => {
            const target = ${q(last)};
            target.scrollIntoView({block: 'end'});
            await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
            const rect = target.getBoundingClientRect();
            const barTop = ${q('.vk-ui-bottom-nav')}.getBoundingClientRect().top;
            const hit = target.contains(document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2));
            return {bottom: rect.bottom, barTop, hit};
          })()`);
          check(clearance.hit && clearance.bottom <= clearance.barTop + 1,
            `${label} final action clears bottom bar at ${width}px ${theme}/${scale}% ${JSON.stringify(clearance)}`);
        }
        if ((width === 320 && scale === 200) ||
            (width === 390 && scale === 100) ||
            (width === 1440 && theme === 'light' && scale === 100)) {
          const name = `${label}-${width}-${theme}-${scale}`;
          // Let the foreground target frame the new scroll position before CDP snapshots it.
          const context = await page.eval(`(async () => {
            ${q(contrastSelectors[0])}.scrollIntoView({block:'start', behavior:'instant'});
            await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
            const root = document.documentElement;
            return {
              path: location.pathname, visibility: document.visibilityState,
              focused: document.hasFocus(), readyState: document.readyState,
              hydrated: root.dataset.hydrated === 'true',
              width: innerWidth, height: innerHeight, dpr: devicePixelRatio,
              theme: root.dataset.theme, fontSize: getComputedStyle(root).fontSize, scrollY,
              wasmBytes: performance.getEntriesByType('resource')
                .find(entry => new URL(entry.name).pathname.endsWith('/vink_bg.wasm'))?.decodedBodySize,
            };
          })()`);
          try {
            await page.capture(name, {
              scenario: label, textScale: scale, requiredTargets: contrastSelectors,
              requestedScene: {width, height: 900, theme, scale},
            });
          } catch (error) {
            throw new Error(`${name} capture failed (${JSON.stringify(context)}): ${error.message}`, {cause: error});
          }
        }
      }
    }
  }
  await page.eval("document.documentElement.style.fontSize = ''; delete document.documentElement.dataset.theme");
  await page.viewport(1280);
}

async function relationshipR5Layouts(page, base, handle, {check, chooseAppearance}) {
  const account = base + '/toi';
  const openAppearance = async () => {
    await page.goto(account);
    await page.waitFor(q('#page-appearance'), 'member appearance controls ready');
  };
  await page.send('Page.bringToFront');
  await openAppearance();
  const previous = await page.eval("['theme', 'text'].map(group => document.querySelector('#page-' + group + ' input:checked').value)");

  const capture = async (width, theme, scale, state, direction, target) => {
    await page.viewport(width, 900);
    await page.goto(`${base}/@${handle}/${direction}`);
    const actualScale = scale === 200 ? 150 : scale;
    await page.waitFor(`${q('#rel-back-profile')} && ${q(target)} && ${q('.vk-rel-directions [aria-current=page]')} &&
      document.documentElement.dataset.hydrated === 'true' &&
      document.documentElement.dataset.theme === '${theme}' &&
      (document.documentElement.dataset.text || '100') === '${actualScale}'`, 'R5 state with active member appearance');
    if (scale === 200) await page.eval("document.documentElement.dataset.text = '200'");
    const facts = await page.eval(`(async () => {
      const root = document.documentElement;
      const back = ${q('#rel-back-profile')};
      const heading = ${q('#relationship-title')};
      const directions = ${q('.vk-rel-directions')};
      const current = directions.querySelector('[aria-current=page]');
      const artwork = ${q('#relationship-empty img')};
      if (artwork) await artwork.decode();
      scrollTo({top: 0, behavior: 'instant'});
      await document.fonts.ready;
      await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
      const title = heading.getBoundingClientRect();
      const link = back.getBoundingClientRect();
      const indicator = getComputedStyle(current, '::after');
      const empty = artwork?.closest('#relationship-empty')?.getBoundingClientRect();
      const slot = artwork?.closest('.vk-state-slot')?.getBoundingClientRect();
      const body = ${q('#relationship-list')};
      const wholeWords = element => {
        const text = [...element.childNodes].find(node => node.nodeType === Node.TEXT_NODE && node.textContent.trim());
        if (!text) return false;
        return [...text.textContent.matchAll(/\\S+/g)].every(word => {
          const range = new Range();
          range.setStart(text, word.index);
          range.setEnd(text, word.index + word[0].length);
          return range.getClientRects().length === 1;
        });
      };
      const names = [heading, ...body.querySelectorAll('.vk-ui-byline__name')];
      const handles = [...body.querySelectorAll('.vk-ui-identity-handle')];
      const fitsLines = (element, lines) => {
        const style = getComputedStyle(element);
        return element.clientHeight <= lines * parseFloat(style.lineHeight) + 1;
      };
      return {
        overflow: root.scrollWidth - innerWidth,
        nameClamp: names.every(el => getComputedStyle(el).webkitLineClamp === '2' && fitsLines(el, 2)),
        handleClamp: handles.every(el => getComputedStyle(el).textOverflow === 'ellipsis' && fitsLines(el, 1)),
        compactBack: back.textContent.trim() === 'Hồ sơ' && back.getAttribute('aria-label') === 'Về Hồ sơ',
        fullHeading: heading.getAttribute('aria-label').startsWith(heading.textContent.trim() + ' · '),
        oneHeading: body.querySelectorAll('h1').length === 1 && heading.tagName === 'H1',
        useUiFont: getComputedStyle(heading).fontFamily.includes('Be Vietnam Pro'),
        wholeWords: wholeWords(heading) && [...body.querySelectorAll('.vk-ui-byline__name')].every(wholeWords),
        backShape: link.width >= 48 && link.height >= 48 &&
          getComputedStyle(back).textDecorationLine === 'none' &&
          (innerWidth < 760 ? link.width === link.height && link.right <= title.left : link.bottom <= title.top),
        leftAligned: Math.abs(directions.firstElementChild.getBoundingClientRect().left -
          body.querySelector('header').getBoundingClientRect().left) <= 1,
        indicator: indicator.height === '3px' && indicator.content !== 'none',
        centered: !artwork || (artwork.alt === '' && artwork.naturalWidth === 512 && slot && empty &&
          Math.abs(slot.top + slot.bottom - empty.top - empty.bottom) <= 2),
      };
    })()`);
    check(facts.overflow <= 1 && facts.oneHeading && facts.useUiFont && facts.wholeWords && facts.backShape &&
      facts.nameClamp && facts.handleClamp && facts.compactBack && facts.fullHeading &&
      facts.leftAligned && facts.indicator && facts.centered,
      `#590 ${state} ${width}/${theme}/${scale}% ${JSON.stringify(facts)}`);
    await page.capture(`r5-relationship-${state}-${width}-${theme}-${scale}`, {
      textScale: scale,
      textScaleLane: scale === 200 ? 'dom-override-geometry-only' : 'app-appearance-control',
      requiredTargets: ['#rel-back-profile', '#relationship-title', '.vk-rel-directions [aria-current=page]', target],
    });
  };
  try {
    const states = [
      ['populated', 'nguoi-theo-doi', '.vk-rel-item a'],
      ['empty', 'dang-theo-doi', '#relationship-empty img'],
    ];
    for (const theme of ['light', 'dark']) {
      await chooseAppearance(page, 'theme', theme);
      for (const scale of [100, 150]) {
        await chooseAppearance(page, 'text', scale);
        for (const width of [390, 1440]) {
          for (const state of states) await capture(width, theme, scale, ...state);
        }
        await openAppearance();
      }
      for (const width of [320, 834]) {
        for (const state of states) await capture(width, theme, 200, ...state);
      }
      await openAppearance();
    }
  } finally {
    await openAppearance();
    await chooseAppearance(page, 'theme', previous[0]);
    await chooseAppearance(page, 'text', previous[1]);
    await page.viewport(1280);
  }
}

async function profileRecovery(guest, base, handle, { check, textContrast }) {
  const missing = `${base}/@chu-chua-co-nguoi-o`;
  const lookup = `${base}/@${handle}?cursor=tiep`;
  const heading = '#author-profile-heading';
  const retry = '#profile-retry';
  const missingExplore = '[data-panel="error:not-found"] a[href="/"]';
  const response = (url, method) => fetch(url, {method, redirect: 'manual'});
  for (const method of ['GET', 'HEAD']) {
    const missingPage = await response(missing, method);
    check(missingPage.status === 404 && missingPage.headers.get('cache-control') === 'no-store',
      `${method} to unknown profile returns 404 and no-store`);
    for (const path of ['nguoi-theo-doi', 'dang-theo-doi']) {
      const missingList = await response(`${missing}/${path}`, method);
      check(missingList.status === 404, `${method} to unknown profile ${path} returns 404`);
      if (method === 'GET') check((await missingList.text()).includes('data-panel="error:not-found"'),
        `unknown profile ${path} uses the same missing presentation`);
    }
  }
  await guest.send('Emulation.setScriptExecutionDisabled', {value: true});
  try {
    await guest.viewport(390);
    await guest.goto(missing);
    check(await guest.eval(`${q(heading)}?.textContent === 'Không tìm thấy trang này.' &&
      ${q(missingExplore)}?.textContent === 'Về Khám phá' &&
      document.querySelector('meta[name=robots][content=noindex]') &&
      !document.documentElement.dataset.hydrated`), '404 uses shared NotFound and offers Khám phá with JavaScript disabled');
    await guest.eval(`${q(heading)}.focus()`);
    await guest.key('Tab', 0, {settle: false});
    await guest.waitFor(`document.activeElement?.matches(${JSON.stringify(missingExplore)})`, '404 Tab reaches the recovery link');
    await guest.capture('profile-missing-390-light-100', {scenario: 'profile-missing', textScale: 100, boundary: 'real SSR, JS disabled'});
    await guest.key('Enter', 0, {settle: false});
    await guest.waitFor("location.pathname === '/'", '404 recovery link opens Khám phá by keyboard');
  } finally {
    await guest.send('Emulation.setScriptExecutionDisabled', {value: false});
    await guest.viewport(1280);
  }

  // Real SSR failure from the documented resolver gate, in the disposable #331 database.
  authorFixtureSql("UPDATE author_handle_rollout SET ready=false WHERE singleton");
  try {
    for (const method of ['GET', 'HEAD']) {
      check((await response(lookup, method)).status === 503, `${method} to temporarily unavailable profile returns 503`);
    }
    for (const [segment, apiDirection] of [['nguoi-theo-doi', 'followers'], ['dang-theo-doi', 'following']]) {
      const route = `${base}/@${handle}/${segment}?cursor=tiep`;
      check((await response(route, 'GET')).status === 503, `${apiDirection} list read error is 503`);
      await guest.goto(route);
      check(await guest.eval(`${q('#rel-retry')}?.form.elements.cursor.value === 'tiep' &&
        !${q('#relationship-empty')} && !${q('#relationship-count')}`), `${apiDirection} read error offers retry and is distinct from zero or empty`);
      await guest.send('Page.bringToFront');
      await guest.capture(`r5-relationship-read-error-${apiDirection}`, {
        textScale: 100, requiredTargets: ['#relationship-title', '#rel-retry'],
      });
    }
    await checkLayoutAndA11y(guest, check, textContrast, lookup, [retry], 'profile-read-unavailable');

    await guest.goto(base + '/');
    await guest.waitFor("document.documentElement.dataset.hydrated === 'true'", 'app entry hydrated');
    await guest.eval(`(() => {
      const saved = document.createElement('a');
      saved.id = 'scenario-profile-unavailable';
      saved.setAttribute('data-profile-return', 'true');
      saved.href = '/@${handle}?cursor=tiep';
      saved.textContent = 'Hồ sơ đã lưu';
      document.querySelector('main').prepend(saved);
    })()`);
    await guest.click('#scenario-profile-unavailable');
    await guest.waitFor(`document.activeElement?.id === 'author-profile-heading' &&
      ${q(retry)}`, 'in-app 503 puts focus on the recovery heading');
    authorFixtureSql("UPDATE author_handle_rollout SET ready=true WHERE singleton");
    await guest.eval(`${q(retry)}.focus()`);
    await guest.key('Space');
    await guest.waitFor(`document.activeElement?.id === 'author-profile-heading' &&
      !${q(retry)} && location.href === ${JSON.stringify(lookup)}`, 'hydrated retry keeps its resource and recovers heading focus');
    authorFixtureSql("UPDATE author_handle_rollout SET ready=false WHERE singleton");

    await guest.send('Emulation.setScriptExecutionDisabled', {value: true});
    try {
      await guest.goto(lookup);
      check(await guest.eval(`document.documentElement.dataset.hydrated !== 'true' &&
        ${q(heading)}?.textContent === 'Chưa tải được hồ sơ' &&
        ${q(retry)}?.form.elements.cursor.value === 'tiep'`), '503 SSR exposes a native retry of the same profile and cursor without JavaScript');
      authorFixtureSql("UPDATE author_handle_rollout SET ready=true WHERE singleton");
      await guest.eval(`${q(retry)}.focus()`);
      await guest.key('Enter', 0, {settle: false});
      await guest.waitFor(`document.readyState === 'complete' && location.href === ${JSON.stringify(lookup)} &&
        !${q(retry)} && ${q(heading)} && !document.documentElement.dataset.hydrated`, 'native retry restores the requested profile without JavaScript');
    } finally {
      await guest.send('Emulation.setScriptExecutionDisabled', {value: false});
    }
  } finally {
    authorFixtureSql("UPDATE author_handle_rollout SET ready=true WHERE singleton");
  }
}

async function graphReadUnavailable(page, url, writeUrl) {
  let requests = 0;
  let heldWrite;
  const listener = (event) => {
    if (event.sessionId !== page.sessionId || event.method !== 'Fetch.requestPaused') return;
    const {requestId, request} = event.params;
    if (request.url === writeUrl && request.method === 'PUT' && !heldWrite) {
      heldWrite = {requestId, status: event.params.responseStatusCode};
      return;
    }
    if (request.url !== url || request.method !== 'GET') {
      page.send('Fetch.continueRequest', {requestId}).catch(() => {});
      return;
    }
    requests++;
    page.send('Fetch.fulfillRequest', {
      requestId,
      responseCode: 503,
      responseHeaders: [{name: 'content-type', value: 'application/json'}],
      body: Buffer.from(JSON.stringify({error: {code: 'unavailable'}})).toString('base64'),
    }).catch(() => {});
  };
  page.cdp.listeners.push(listener);
  const patterns = [{urlPattern: url}];
  if (writeUrl) patterns.push({urlPattern: writeUrl, requestStage: 'Response'});
  await page.send('Fetch.enable', {patterns});
  return {
    requests: () => requests,
    async dropWrite() {
      for (let i = 0; !heldWrite && i < 160; i++) await sleep(50);
      if (!heldWrite) throw new Error('timed out holding the real Follow write');
      await page.send('Fetch.failRequest', {requestId: heldWrite.requestId, errorReason: 'ConnectionClosed'});
      return heldWrite.status;
    },
    async finish() {
      page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
      await page.send('Fetch.disable');
    },
  };
}

export async function authorFollows(page, other, { base, openPage, check, holdAnswer, textContrast, switchActor, chooseAppearance }) {
  console.log('Social Follow Graph (#294):');
  const profA = await ownProfile(page);
  const profB = await ownProfile(other);
  const handleA = profA.handle;
  const handleB = profB.handle;
  check(Boolean(handleA && handleB && handleA !== handleB), 'distinct initial author handles for A and B');

  // 1. SSR HTML & Guest CTA -> /toi?next= return + open-redirect rejection.
  const {browserContextId: guestContext} = await page.cdp.send('Target.createBrowserContext');
  const guest = await openPage(guestContext);
  await guest.goto(`${base}/@${handleB}`);
  const ssrCheck = await guest.eval(`fetch(${JSON.stringify(`/@${handleB}`)}).then(async r => ({ status: r.status, text: await r.text() }))`);
  check(ssrCheck.status === 200 && ssrCheck.text.includes('profile-followers-link') && !ssrCheck.text.includes('Theo dõi lẫn nhau'),
    'SSR public profile renders followers link without mutual/viewer secrets');

  await guest.waitFor(q('#follow-sign-in'), 'guest sees follow sign-in CTA');
  check(await guest.eval(`${q('#follow-sign-in')}.textContent.trim() === 'Theo dõi' &&
    !document.querySelector('main').textContent.includes('Đăng nhập để theo dõi') &&
    ${q('.vk-author-social')}.contains(${q('#follow-sign-in')})`),
    'guest sees only the compact Theo dõi action in the header social row');
  const signInHref = await guest.eval(`${q('#follow-sign-in')}.getAttribute('href')`);
  const signIn = new URL(signInHref, base);
  check(signIn.origin === new URL(base).origin && signIn.pathname === '/toi' &&
    signIn.searchParams.get('next') === `/@${handleB}`, `guest CTA points to /toi?next=... (${signInHref})`);

  await page.goto(`${base}/toi?next=${encodeURIComponent(`/@${handleB}`)}`);
  await page.waitFor(q('#me-return-link'), 'active session shows validated internal return link on /toi');
  check((await page.eval(`${q('#me-return-link')}.getAttribute('href')`)) === `/@${handleB}`, 'return link targets canonical author profile');

  await page.goto(`${base}/toi?next=https://evil.example/%40${handleB}`);
  await page.waitFor(q('#me-title'), 'Me loads with rejected external next');
  check(await page.eval(`!${q('#me-return-link')}`), 'open redirect next URL is rejected');

  // 2. Owner profile: no self-follow button; own Following link/count & /dang-theo-doi accessible even with following_list=self.
  const ownerGraphDown = await graphReadUnavailable(page, `${base}/api/v1/authors/${handleA}/relationship/me`);
  try {
    await page.goto(`${base}/@${handleA}`);
    await page.waitFor(q('#own-profile-edit-link'), 'own profile proves ownership despite Graph 503');
    check(ownerGraphDown.requests() === 1 && await page.eval(`!${q('#follow-action')} && !${q('#follow-retry')} &&
      ${q('#own-profile-edit-link')}.getAttribute('href') === '/toi/chinh-ho-so'`),
      'Graph-only 503 preserves Chỉnh hồ sơ and hides viewer-only controls');
  } finally {
    await ownerGraphDown.finish();
  }
  await page.goto(`${base}/@${handleA}`);
  await page.waitFor(q('#profile-following-link'), 'owner sees Following link despite default self policy');
  await page.waitFor(q('#own-profile-edit-link'), 'owner sees the Edit Profile action');
  check(await page.eval(`!${q('#follow-action')} && ${q('#own-profile-edit-link')}.getAttribute('href') === '/toi/chinh-ho-so'`),
    'owner can edit the profile and sees no self-follow action');
  await page.click('#profile-following-link');
  await page.waitFor(q('#relationship-title'), 'owner navigates to own following list');
  check(await page.eval(`${q('#relationship-title')}.textContent.trim() === ${JSON.stringify(profA.pen_name)} &&
    ${q('#relationship-title')}.getAttribute('aria-label').endsWith('Đang theo dõi')`),
    'own Following has a concise identity and the accessible direction');
  await page.waitFor("document.activeElement?.id === 'relationship-title'", 'following list navigation focuses its heading');
  await page.eval('history.back()');
  await page.waitFor("document.activeElement?.id === 'profile-following-link'", 'Back restores the profile Following link');

  const suspensionId = literal(randomUUID());
  authorFixtureSql(`INSERT INTO principal_suspensions (id, principal_id, reason)
    VALUES (${suspensionId}, ${literal(profA.principal_id)}, 'Kiểm thử quyền đọc quan hệ')`);
  try {
    await page.goto(`${base}/@${handleA}/dang-theo-doi`);
    await page.waitFor(q('#relationship-private'), 'suspended owner sees the public denial of a private list');
    const authority = await page.eval(`(async () => {
      const standing = await fetch('/api/v1/auth/session').then(r => r.json());
      const relationship = await fetch(${JSON.stringify(ownFollowing)});
      const body = await relationship.json();
      return {standing: standing.state, status: relationship.status, code: body.error?.code,
        projectionHidden: !${q('#relationship-count, #relationship-empty, #rel-other-direction')}};
    })()`);
    check(authority.standing === 'suspended' && authority.status === 401 &&
      authority.code === 'unauthenticated' && authority.projectionHidden,
    `real App suspension fences the own endpoint and private UI projection (${JSON.stringify(authority)})`);
  } finally {
    authorFixtureSql(`UPDATE principal_suspensions SET lifted_at=now() WHERE id=${suspensionId}`);
  }

  // 3. Keyboard Follow/Unfollow keeping focus + Mutual matrix across A, B and Guest.
  await page.goto(`${base}/@${handleB}`);
  await page.waitFor(q('#follow-action'), 'member sees #follow-action on other profile');
  await page.eval(`${q('#follow-action')}.focus()`);
  await page.key('Enter');
  await page.waitFor(`${q('#follow-action')}?.textContent?.trim() === 'Bỏ theo dõi' &&
    ${q('#follow-status')}?.textContent?.trim() === 'Đã theo dõi tác giả.'`, 'keyboard Follow commits and announces once');
  check(await page.eval(`document.activeElement?.id === 'follow-action'`), 'focus stays on #follow-action after keyboard Follow');
  check(await page.eval(`${q('#profile-followers-link')}?.textContent?.includes('1 người theo dõi')`), 'followers count updates in-place to 1');

  await other.goto(`${base}/@${handleA}`);
  await other.waitFor(q('#follow-action'), 'B sees #follow-action on A');
  await other.click('#follow-action');
  await other.waitFor(q('#follow-mutual'), 'B sees Mutual badge when B follows A back');
  check(await other.eval(`${q('#follow-mutual')}.textContent.includes('Theo dõi lẫn nhau')`), 'Mutual badge text on B');

  await page.reload();
  await page.waitFor(q('#follow-mutual'), 'A sees Mutual badge on B');
  await guest.goto(`${base}/@${handleB}`);
  await guest.waitFor(q('#follow-sign-in'), 'guest profile settles');
  check(await guest.eval(`!${q('#follow-mutual')}`), 'guest never sees Mutual badge');
  await other.goto(`${base}/@${handleB}/nguoi-theo-doi`);
  await other.waitFor(q('.vk-rel-item .vk-follow-mutual'), 'owner sees Mutual in their private list overlay');
  await guest.goto(`${base}/@${handleB}/nguoi-theo-doi`);
  check(await guest.eval(`Boolean(${q('.vk-rel-item')}) && !${q('.vk-rel-item .vk-follow-mutual')}`),
    'the same public list has no Mutual badge');
  await other.goto(`${base}/@${handleA}`);
  await other.waitFor(q('#follow-mutual'), 'second member has their authorized Mutual state again');

  // A touch Unfollow and a pointer Unfollow reset the active edges before the late-ACK case.
  await page.viewport(390);
  const tap = await page.eval(`(() => {
    const el = ${q('#follow-action')};
    el.scrollIntoView({block:'center'});
    const r = el.getBoundingClientRect();
    return {x:r.x + r.width / 2, y:r.y + r.height / 2};
  })()`);
  await page.send('Input.dispatchTouchEvent', {type:'touchStart', touchPoints:[tap]});
  await page.send('Input.dispatchTouchEvent', {type:'touchEnd', touchPoints:[]});
  await page.viewport(1280);
  await page.waitFor(`${q('#follow-action')}?.textContent?.trim() === 'Theo dõi' && !${q('#follow-mutual')}`, 'A unfollows B cleanly');
  await other.click('#follow-action');
  await other.waitFor(`${q('#follow-action')}?.textContent?.trim() === 'Theo dõi' && !${q('#follow-mutual')}`, 'B unfollows A cleanly');

  // 4. CDP holdAnswer ABA lost-ACK -> reconcile -> Unfollow -> late v1 ACK ignored + event timing.
  await page.reload();
  await page.waitFor(q('#follow-action'), 'ready for ABA lost-ACK test');
  await page.eval(`(() => {
    window.__followEvents = [];
    window.addEventListener('vink:follow-changed', (e) => window.__followEvents.push(e.detail));
  })()`);

  const actionPath = `/api/v1/authors/${handleB}/relationship/me`;
  const writes = captureRequest(page, 'PUT', actionPath);
  const heldFirst = await holdAnswer(page, 'PUT', actionPath);
  await page.viewport(320);
  await page.eval(`(async () => {
    document.documentElement.style.fontSize = '200%';
    await document.fonts.ready;
    const node = ${q('#follow-action')};
    node.focus();
    window.__followGeometry = {node, rect: node.getBoundingClientRect().toJSON(), live: ${q('#follow-status')}};
  })()`);
  await page.key('Space');
  check((await heldFirst.held()) === 200, 'real server committed the first PUT of the ABA case before ACK was held');
  await page.waitFor(`${q('#follow-action')}?.getAttribute('aria-busy') === 'true'`, 'held Follow exposes busy state');
  const pending = await page.eval(`(() => {
    const node = ${q('#follow-action')}, rect = node.getBoundingClientRect().toJSON();
    const before = window.__followGeometry;
    const moved = ['x','y','width','height'].some(dim => Math.abs(rect[dim] - before.rect[dim]) > 0.5);
    return {same: node === before.node && document.activeElement === node,
      live: ${q('#follow-status')} === before.live, moved, rect, before: before.rect};
  })()`);
  check(pending.same && pending.live && !pending.moved, `320px/200% pending keeps the node, focus, live region and rect ${JSON.stringify(pending)}`);
  await page.key('Enter');
  check(writes.count() === 1, 'keyboard reactivation while pending sends no second write');
  check((await page.eval('window.__followEvents.length')) === 0, 'vink:follow-changed does NOT fire while ACK is pending');

  // Slow pending reveals #follow-reconcile after ~1s; clicking duplicate #follow-action does not double-send.
  await page.waitFor(q('#follow-reconcile'), 'reconcile control appears during slow/lost ACK');
  await page.click('#follow-reconcile');
  await page.waitFor(
    `${q('#follow-action')}?.textContent?.trim() === 'Bỏ theo dõi' && window.__followEvents.length === 1`,
    'reconciliation updates state cleanly and dispatches one invalidation event'
  );
  await page.waitFor(`${q('#profile-followers-link')}?.textContent?.includes('1 người theo dõi')`, 'reconcile updated followers count to 1');
  await page.waitFor("document.activeElement?.id === 'follow-action'", 'focus returns from the dismissed reconciliation action');
  await page.eval("document.documentElement.style.fontSize = ''");
  await page.viewport(1280);

  // Unfollow as the newer intent.
  await page.click('#follow-action');
  await page.waitFor(
    `${q('#follow-action')}?.textContent?.trim() === 'Theo dõi' && window.__followEvents.length === 2`,
    'newer Unfollow settles and dispatches second invalidation event'
  );
  check(await page.eval(`${q('#profile-followers-link')}?.textContent?.includes('0 người theo dõi')`), 'count decremented to 0 after Unfollow');

  // Release the older delayed ACK; the newer accepted intent must remain visible.
  await heldFirst.finish(false, true);
  await sleep(150);
  check(await page.eval(`${q('#follow-action')}?.textContent?.trim() === 'Theo dõi'`), 'stale late v1 ACK ignored; button remains Theo dõi');
  check(await page.eval(`${q('#profile-followers-link')}?.textContent?.includes('0 người theo dõi')`), 'stale late v1 ACK ignored; count remains 0');
  check((await page.eval('window.__followEvents.length')) === 2, 'stale late v1 ACK does not emit extra invalidation');
  check(await page.eval(`${q('#follow-status')}?.textContent.trim() === 'Đã bỏ theo dõi tác giả.'`),
    'stale late v1 ACK cannot replace the announcement for the latest action');
  writes.close();

  await page.eval(`(() => {
    const live = ${q('#follow-status')};
    window.__followNotices = [];
    const observe = new MutationObserver(() => window.__followNotices.push(live.textContent.trim()));
    observe.observe(live, {subtree: true, characterData: true, childList: true});
  })()`);
  const lostAck = await holdAnswer(page, 'PUT', actionPath);
  await page.click('#follow-action');
  check((await lostAck.held()) === 200, 'real Follow succeeds before the ACK is dropped');
  await lostAck.finish(true);
  await page.waitFor(`${q('#follow-action')}?.textContent.trim() === 'Bỏ theo dõi' &&
    window.__followEvents.length === 3`, 'lost ACK reconciles automatically from the real receipt');
  check(await page.eval(`window.__followNotices.some(text => text.startsWith('Chưa nhận')) &&
    window.__followNotices.filter(text => text === 'Đã đối chiếu trạng thái mới nhất từ máy chủ.').length === 1`),
    'lost ACK exposes reconciliation and announces its new result once');
  await page.click('#follow-action');
  await page.waitFor(`${q('#follow-action')}?.textContent.trim() === 'Theo dõi' && window.__followEvents.length === 4`,
    'reset edge after the dropped-ACK test');

  // A confirmed PUT can intentionally return an empty projection for all four read fields.
  const setPolicies = (changes) => authorFixtureSql(`UPDATE author_profiles SET policies = policies ||
    ${literal(JSON.stringify(changes))}::jsonb WHERE principal_id = ${literal(profB.principal_id)}`);
  const allHidden = {followers_list: 'self', followers_count: 'self', following_list: 'self', following_count: 'self'};
  const mixedGrants = {followers_list: 'everyone', followers_count: 'self', following_list: 'self', following_count: 'everyone'};
  const profileApi = `${base}/api/v1/authors/${handleB}`;
  const noFacts = "!document.querySelector('#profile-followers-link, #profile-following-link, #profile-followers-count, #profile-following-count')";
  check(await page.eval(`Boolean(${q('#profile-followers-link')})`), 'old public link is present before all four grants are revoked');
  setPolicies(allHidden);
  const unexpectedRead = captureRequest(page, 'GET', new URL(profileApi).pathname);
  await page.click('#follow-action');
  await page.waitFor(`${noFacts} && ${q('#follow-action')}?.textContent.trim() === 'Bỏ theo dõi'`,
    'authoritative empty PUT removes both directions without a second read');
  check(unexpectedRead.count() === 0 && await page.eval(`!${q('#profile-relationships-retry')}`),
    'confirmed hidden fields produce neither stale facts nor a retry');
  unexpectedRead.close();

  // A lost ACK confirms the edge from its receipt. Its separate public summary can fail.
  setPolicies(mixedGrants);
  await page.reload();
  await page.waitFor(`${q('#follow-action')}?.textContent.trim() === 'Bỏ theo dõi' &&
    ${q('#profile-followers-link')} && ${q('#profile-following-count')}`, 'mixed grants are visible before isolated summary failure');
  const summaryDown = await graphReadUnavailable(page, profileApi, base + actionPath);
  try {
    await page.click('#follow-action');
    check(await summaryDown.dropWrite() === 200, 'receipt fixture commits before only summary transport fails');
    await page.waitFor(`${q('#follow-action')}?.textContent.trim() === 'Theo dõi' &&
      ${q('#profile-relationships-retry')} && ${noFacts}`, 'summary failure hides old facts separately from the confirmed edge');
    check(summaryDown.requests() > 0, 'only the public summary is refused after the lost ACK');
  } finally {
    await summaryDown.finish();
  }
  await page.eval(`window.__summaryProfile = ${q('.vk-author-public')}`);
  const sessionControls = await openPage();
  await switchActor('guest', sessionControls);
  await page.waitFor(`${q('#follow-sign-in')} && ${q('#profile-relationships-retry')} && ${noFacts}`,
    'same profile gives the signed-out visitor a public summary retry');
  check(await page.eval(`window.__summaryProfile === ${q('.vk-author-public')}`),
    'the page displaying the summary stayed mounted during sign-out');
  const guestRetry = await holdAnswer(page, 'GET', new URL(profileApi).pathname);
  await page.click('#profile-relationships-retry');
  check(await guestRetry.held() === 200, 'guest retry reads a real public summary');
  await switchActor('a', sessionControls, 'anonymous');
  await page.waitFor(q('#follow-action'), 'a replacement viewer session appears during the public read');
  await guestRetry.finish(false, true);
  await page.waitFor(`${q('#profile-followers-link')}?.textContent.trim() === 'Người theo dõi' &&
    ${q('#profile-following-count')}?.textContent.trim() === '0 đang theo dõi' &&
    !${q('#profile-followers-count')} && !${q('#profile-following-link')}`,
    'retry restores a list without count and an independent zero count without list');
  console.log('  ok  verified empty PUT, public summary retry as guest and separate fields after a viewer change');

  // 5. 15 followers + independent 4-policy matrix + append retry + mid-list live permission loss.
  const followerSql = [];
  const lastFollowers = [];
  for (let i = 0; i < 15; i++) {
    const uid = randomUUID();
    const h = `ban-theo-doi-${i.toString().padStart(2, '0')}-${randomUUID().slice(0, 6)}`;
    const name = i === 0 ? 'Bạn đọc Bên Thềm Hiên Nhà Qua Những Mùa Mưa Tháng Mười Một' : `Bạn đọc ${i + 1}`;
    if (i >= 12) lastFollowers.push(literal(uid));
    followerSql.push(`
      INSERT INTO principals (id, display_name) VALUES (${literal(uid)}, ${literal(name)});
      INSERT INTO principal_bindings (principal_id, authority, subject_id) VALUES (${literal(uid)}, 'vink-identity', ${literal(`e2e-f-${uid}`)});
      INSERT INTO author_handle_names (principal_id, handle) VALUES (${literal(uid)}, ${literal(h)});
      INSERT INTO author_profiles (principal_id, current_handle, setup_complete) VALUES (${literal(uid)}, ${literal(h)}, true);
      INSERT INTO author_follows (follower_id, followee_id, active, version, accepted_order, active_since)
      VALUES (${literal(uid)}, ${literal(profB.principal_id)}, true, 1, 1, clock_timestamp() - interval '${i} seconds');
    `);
  }
  authorFixtureSql(`BEGIN;
    ${followerSql.join('\n')}
    UPDATE author_profiles SET policies = '{
      "pen_name":"everyone","avatar":"everyone","bio":"everyone","date_of_birth":"self",
      "followers_list":"everyone","followers_count":"self",
      "following_list":"self","following_count":"self"
    }'::jsonb WHERE principal_id = ${literal(profB.principal_id)};
  COMMIT;`);

  // Check list=everyone, count=self: profile link shows "Người theo dõi" (no count), list page shows 12 rows without #relationship-count.
  await guest.goto(`${base}/@${handleB}`);
  await guest.waitFor(q('#profile-followers-link'), 'guest sees list link when followers_list=everyone and count=self');
  check(await guest.eval(`${q('#profile-followers-link')}.textContent.trim() === 'Người theo dõi'`), 'profile hides count when followers_count=self');

  await guest.click('#profile-followers-link');
  await guest.waitFor(`document.querySelectorAll('.vk-rel-item').length === 12 && ${q('#rel-load-more')}`, 'hydrated page 1 shows 12 followers and can append');
  check(await guest.eval(`!${q('#relationship-count')} && !${q('.vk-rel-directions[aria-label]')} &&
    !${q('#rel-other-direction')} && ${q('#relationship-list')}.textContent.includes('Người theo dõi')`),
    'one permitted direction reads as a label and the private count and direction are absent');

  const firstPerson = '.vk-rel-item a[id^="rel-person-"]';
  await guest.eval(`${q(firstPerson)}.focus()`);
  await guest.key('Enter');
  await guest.waitFor("document.activeElement?.id === 'author-profile-heading'", 'person link opens the correct profile and focuses H1');
  await guest.eval('history.back()');
  await guest.waitFor(`document.activeElement === ${q(firstPerson)}`, 'Back restores the person link on the list');
  check(await guest.eval(`${q(firstPerson)}.getAttribute('aria-label')?.includes('@') &&
    ${q(firstPerson)}.querySelector('.vk-ui-byline__handle')`), 'person link exposes the handle in its byline and accessible name');

  // A transport failure leaves the rows and the focused pagination button mounted.
  const dropAppend = await holdAnswer(guest, 'GET', `/api/v1/authors/${handleB}/followers`);
  await guest.eval(`window.__append = ${q('#rel-load-more')}; window.__person = ${q(firstPerson)}; window.__append.focus()`);
  await guest.key('Enter');
  check((await dropAppend.held()) === 200, 'real page 2 held before network failure injection');
  await dropAppend.finish(true);
  await guest.waitFor(`${q('#rel-load-more')}?.textContent.trim() === 'Thử lại tải thêm'`, 'same pagination action becomes Retry');
  check(await guest.eval(`document.querySelectorAll('.vk-rel-item').length === 12 &&
    window.__person === ${q(firstPerson)} && window.__append === document.activeElement`),
    'append error preserves existing rows, the button and keyboard focus');
  await guest.key('Space');
  await guest.waitFor(`document.querySelectorAll('.vk-rel-item').length === 15 && !${q('#rel-load-more')}`, 'append retry loads remaining 3 rows (total 15)');
  await guest.waitFor("document.activeElement === document.querySelectorAll('.vk-rel-item a')[12]", 'successful retry brings focus to the first new row');

  const appendedId = await guest.eval("document.activeElement.id");
  const appendedLink = q('#' + appendedId);
  const listPath = `/@${handleB}/nguoi-theo-doi`;
  await guest.key('Enter');
  await guest.waitFor("document.activeElement?.id === 'author-profile-heading'", 'open a profile from appended page 2');
  await guest.eval('history.back()');
  await guest.waitFor(`document.activeElement === ${appendedLink}`, 'Back freshly reads page 2 and restores the appended anchor');
  check(await guest.eval(`(() => {
    const entry = history.state.vinkProfileReturn;
    return entry.source === ${JSON.stringify(listPath)} && typeof entry.cursor === 'string' &&
      Object.keys(entry).sort().join(',') === 'cursor,href,id,source';
  })()`), 'Back stores only a cursor and navigation locators in history');

  check(await guest.eval(`${q('#rel-first-page')}?.getAttribute('href') === ${JSON.stringify(listPath)} &&
    ${q('#rel-first-page')}.textContent.trim() === 'Về trang đầu'`),
    'the restored page has a native first-page link');
  await guest.eval(`${q('#rel-first-page')}.focus()`);
  await guest.key('Enter');
  await guest.waitFor(`document.activeElement?.id === 'relationship-title' &&
    document.querySelectorAll('.vk-rel-item').length === 12 && ${q('#rel-load-more')} &&
    !${q('#rel-first-page')} && !history.state?.vinkProfileReturn`,
    'keyboard first-page recovery clears the saved cursor and focuses the fresh heading');
  console.log('  ok  restored page offers first-page recovery by keyboard');

  await guest.eval(`${q('#rel-load-more')}.focus()`);
  await guest.key('Enter');
  await guest.waitFor(`document.activeElement === ${appendedLink}`, 'page 2 can be appended again after first-page recovery');

  const appendedEdge = `followee_id = ${literal(profB.principal_id)} AND follower_id =
    (SELECT principal_id FROM author_profiles WHERE current_handle = ${literal(appendedId.slice('rel-person-'.length))})`;
  const activeSince = authorFixtureSql(`SELECT active_since FROM author_follows WHERE ${appendedEdge}`);
  await guest.key('Enter');
  await guest.waitFor("document.activeElement?.id === 'author-profile-heading'", 'open the same appended profile before revocation');
  authorFixtureSql(`UPDATE author_follows SET active=false, version=version+1, accepted_order=accepted_order+1,
    active_since=NULL WHERE ${appendedEdge}`);
  try {
    await guest.eval('history.back()');
    await guest.waitFor(`document.activeElement?.id === 'relationship-title' && !${appendedLink}`, 'Back focuses H1 if the saved row disappears from the fresh page');
    console.log('  ok  verified page 2 anchor and H1 fallback from freshly read rows');
  } finally {
    authorFixtureSql(`UPDATE author_follows SET active=true, version=version+1, accepted_order=accepted_order+1,
      active_since=${literal(activeSince)} WHERE ${appendedEdge}`);
  }

  setPolicies({following_list: 'everyone'});
  await guest.goto(`${base}/@${handleB}`);
  await guest.goto(base + listPath);
  await guest.waitFor(`${q('#rel-load-more')} && ${q('#rel-other-direction')}`, 'opposite direction is initially allowed');
  setPolicies({following_list: 'self'});
  const otherDirection = await holdAnswer(guest, 'GET', new URL(profileApi).pathname);
  await guest.click('#rel-load-more');
  check(await otherDirection.held() === 200 && await guest.eval(`!${q('#rel-other-direction')}`),
    'append reads the opposite permission and withholds the previous link while checking');
  await otherDirection.finish(false, true);
  await guest.waitFor(`!${q('#rel-other-direction')} && !${q('#rel-direction-retry')} &&
    document.querySelectorAll('.vk-rel-item').length === 15`, 'append revalidates and removes a newly hidden opposite direction');
  console.log('  ok  append revalidates the opposite direction independently');

  setPolicies({following_list: 'everyone'});
  await guest.goto(`${base}/@${handleB}`);
  await guest.goto(base + listPath);
  await guest.waitFor(`${q('#rel-load-more')} && ${q('#rel-other-direction')}`, 'opposite direction restored from fresh reads');
  const directionDown = await graphReadUnavailable(guest, profileApi);
  try {
    await guest.click('#rel-load-more');
    await guest.waitFor(`${q('#rel-direction-retry')} && !${q('#rel-other-direction')} &&
      document.querySelectorAll('.vk-rel-item').length === 15`, 'opposite-direction failure is retryable while the current list remains readable');
    check(directionDown.requests() > 0, 'only the opposite permission read receives 503');
  } finally {
    await directionDown.finish();
  }
  await guest.click('#rel-direction-retry');
  await guest.waitFor(q('#rel-other-direction'), 'retry uses the current grants of the opposite direction');
  setPolicies({following_list: 'self'});


  await guest.goto(`${base}/@${handleB}/nguoi-theo-doi`);
  await guest.waitFor(q('#rel-load-more'), 'first page offers an append cursor');
  const expiredAnchor = await guest.eval("document.querySelector('.vk-rel-item:last-child a').getAttribute('href').slice(2)");
  const edge = `followee_id = ${literal(profB.principal_id)} AND follower_id =
    (SELECT principal_id FROM author_profiles WHERE current_handle = ${literal(expiredAnchor)})`;
  authorFixtureSql(`UPDATE author_follows SET active=false, version=version+1, accepted_order=accepted_order+1,
    active_since=NULL WHERE ${edge}`);
  try {
    await guest.eval(`${q('#rel-load-more')}.focus()`);
    await guest.key('Enter');
    await guest.waitFor(`document.activeElement?.id === 'relationship-title' &&
      !${q("#rel-person-" + expiredAnchor)} && document.querySelectorAll(".vk-rel-item").length === 12`,
      'expired cursor replaces the chain and returns focus to the heading');
  } finally {
    authorFixtureSql(`UPDATE author_follows SET active=true, version=version+1, accepted_order=accepted_order+1,
      active_since=clock_timestamp() WHERE ${edge}`);
  }

  // Mid-list permission revocation: reload page 1 (12 rows), flip policy to followers_list=self, followers_count=everyone,
  // then click Load More -> MUST immediately clear ALL rows and render private notice!
  await guest.goto(`${base}/@${handleB}/nguoi-theo-doi`);
  await guest.waitFor(`document.querySelectorAll('.vk-rel-item').length === 12 && ${q('#rel-load-more')}`, 'fresh page 1 before permission revocation');

  authorFixtureSql(`UPDATE author_profiles SET policies = '{
    "pen_name":"everyone","avatar":"everyone","bio":"everyone","date_of_birth":"self",
    "followers_list":"self","followers_count":"everyone",
    "following_list":"self","following_count":"self"
  }'::jsonb WHERE principal_id = ${literal(profB.principal_id)}`);

  await guest.click('#rel-load-more');
  await guest.waitFor(q('#relationship-private'), 'mid-list revocation switches page to private state');
  await guest.waitFor("document.activeElement?.id === 'relationship-title'", 'revocation restores focus to the list heading');
  check((await guest.eval(`document.querySelectorAll('.vk-rel-item').length`)) === 0, 'mid-list revocation clears all previously loaded rows');
  await guest.send('Page.bringToFront');
  await guest.viewport(390);
  await guest.capture('r5-relationship-denied', {textScale: 100, requiredTargets: ['#relationship-title', '#relationship-private']});
  await guest.viewport(1280);

  await guest.goto(`${base}/@${handleB}`);
  await guest.waitFor(q('#profile-followers-count'), 'profile renders count span when list=self and count=everyone');
  check(await guest.eval(`!${q('#profile-followers-link')} && ${q('#profile-followers-count')}.textContent.includes('15 người theo dõi')`),
    'profile renders count without link when list=self and count=everyone');

  const directionPolicies = [['everyone', 'everyone'], ['everyone', 'self'], ['self', 'everyone'], ['self', 'self']];
  for (const [list, count] of directionPolicies) {
    authorFixtureSql(`UPDATE author_profiles SET policies = policies ||
      '{"followers_list":"${list}","followers_count":"${count}","following_list":"${list}","following_count":"${count}"}'::jsonb
      WHERE principal_id = ${literal(profB.principal_id)};`);
    await guest.goto(`${base}/@${handleB}`);
    for (const [direction, allowedCount] of [['followers', 15], ['following', 0]]) {
      const link = q(`#profile-${direction}-link`), stat = q(`#profile-${direction}-count`);
      const shown = await guest.eval(`({link: ${link}?.textContent.trim(), stat: ${stat}?.textContent.trim()})`);
      check(Boolean(shown.link) === (list === 'everyone') &&
        Boolean(shown.stat) === (list === 'self' && count === 'everyone') &&
        /^\d/.test(shown.link ?? shown.stat ?? '') === (count === 'everyone') &&
        (count !== 'everyone' || (shown.link ?? shown.stat).startsWith(String(allowedCount))),
        `${direction}: independent list=${list} and count=${count} (${JSON.stringify(shown)})`);
    }
    if (list === 'everyone') {
      await guest.waitFor("document.documentElement.dataset.hydrated === 'true'", 'profile navigation has hydrated');
      await guest.click('#profile-followers-link');
      await guest.waitFor("document.activeElement?.id === 'relationship-title'", 'followers heading receives app navigation focus');
      await guest.waitFor(q('#rel-other-direction'), 'permitted Following direction is available from the list');
      await guest.eval(`${q('#rel-other-direction')}.focus()`);
      await guest.key('Enter');
      await guest.waitFor(`document.activeElement?.id === 'relationship-title' && location.pathname.endsWith('/dang-theo-doi')`,
        'direction link moves keyboard focus to Following H1');
      const allowed = count === 'everyone';
      const emptyTitle = allowed ? 'Chưa theo dõi tác giả nào.' : 'Danh sách chưa có mục để hiển thị.';
      check(await guest.eval(`(${q('#relationship-count')}?.textContent ?? null) === ${JSON.stringify(allowed ? '0' : null)} &&
        ${q('#relationship-empty h2')}?.textContent === ${JSON.stringify(emptyTitle)}`),
        'empty Following distinguishes authorized zero from an undisclosed count');
      await guest.eval('history.back()');
      await guest.waitFor("document.activeElement?.id === 'rel-other-direction'", 'Back returns to the authorized direction link');
    }
  }
  authorFixtureSql(`UPDATE author_profiles SET policies = policies ||
    '{"followers_list":"everyone","followers_count":"self"}'::jsonb
    WHERE principal_id = ${literal(profB.principal_id)};`);
  await checkLayoutAndA11y(guest, check, textContrast, `${base}/@${handleB}`, ['#profile-followers-link', '#follow-sign-in'], 'follow-list-without-count');

  // Keep an anchor when the rows beyond it disappear. The new 12 count is the full
  // authorized count, independent of the empty continuation.
  setPolicies({followers_count: 'everyone', following_list: 'everyone', following_count: 'everyone'});
  let nextHref;
  await guest.send('Emulation.setScriptExecutionDisabled', {value: true});
  try {
    await guest.goto(base + listPath);
    nextHref = await guest.eval(`${q('#rel-next-link')}.getAttribute('href')`);
    const {nodes} = await guest.send('Accessibility.getFullAXTree');
    check(nodes.some(node => !node.ignored && node.role?.value === 'heading' &&
      node.name?.value === `${profB.pen_name} · Người theo dõi`), 'SSR heading exposes the author and direction to accessibility APIs');
    check(await guest.eval(`!document.documentElement.dataset.hydrated &&
      ${q('#rel-back-profile')}.getAttribute('href') === '/@${handleB}' &&
      ${q('#rel-other-direction')}.getAttribute('href') === '/@${handleB}/dang-theo-doi' &&
      ${q('[aria-current=page] #relationship-count')}?.textContent === '15' &&
      document.querySelectorAll('.vk-rel-item').length === 12`),
      'SSR emits canonical links and the authorized total independently of rendered rows');
    await guest.eval(`${q('#rel-back-profile')}.focus()`);
    await guest.key('Enter', 0, {settle: false});
    await guest.waitFor(`location.pathname === '/@${handleB}' && ${q('#author-profile-heading')}`,
      'the shared Back activates its canonical href by keyboard without JavaScript');
  } finally {
    await guest.send('Emulation.setScriptExecutionDisabled', {value: false});
  }

  const edgeTail = `followee_id=${literal(profB.principal_id)} AND follower_id IN (${lastFollowers.join(',')})`;
  const removed = JSON.parse(authorFixtureSql(`SELECT json_agg(f) FROM
    (SELECT follower_id, active_since FROM author_follows WHERE ${edgeTail}) f`));
  authorFixtureSql(`UPDATE author_follows SET active=false, version=version+1,
    accepted_order=accepted_order+1, active_since=NULL WHERE ${edgeTail}`);
  try {
    await guest.goto(base + nextHref);
    await guest.waitFor(q('#rel-first-page'), 'empty continuation is recoverable at its real cursor URL');
    check(await guest.eval(`${q('#relationship-empty')}.textContent.trim() === 'Trang này không còn người nào.' &&
      !${q('#relationship-empty img')} && ${q('#relationship-count')}?.textContent === '12'`),
      'empty continuation preserves the independent total and uses continued-page copy');
    await guest.click('#rel-first-page');
    await guest.waitFor(`location.search === '' && ${q('.vk-rel-item')}`, 'first-page link returns to the populated list');
  } finally {
    authorFixtureSql(removed.map(({follower_id, active_since}) => `UPDATE author_follows SET active=true,
      version=version+1, accepted_order=accepted_order+1, active_since=${literal(active_since)}
      WHERE followee_id=${literal(profB.principal_id)} AND follower_id=${literal(follower_id)};`).join('\n'));
  }

  await guest.goto(base + listPath);
  await guest.waitFor(`document.documentElement.dataset.hydrated === 'true' && ${q('#rel-back-profile')}`, 'direct list entry is hydrated');
  await guest.eval(`${q('#relationship-title')}.focus()`);
  await guest.key('Tab', 8);
  await guest.waitFor("document.activeElement?.id === 'rel-back-profile'", 'Back is reachable just before the focused list heading');
  await guest.key('Enter');
  await guest.waitFor("document.activeElement?.id === 'author-profile-heading'", 'BackLink reaches the profile and focuses its H1');
  await guest.eval('history.back()');
  await guest.waitFor("document.activeElement?.id === 'rel-back-profile'", 'browser Back returns focus to BackLink');

  const longName = 'Người Đi Qua Những Mùa Mưa Bên Hiên Nhà Tháng Mười Một';
  const rename = name => authorFixtureSql(`UPDATE principals SET display_name=${literal(name)}
    WHERE id=${literal(profB.principal_id)}`);
  rename(longName);
  try {
    await relationshipR5Layouts(page, base, handleB, {check, chooseAppearance});
    const unbroken = 'Nguyễn'.repeat(12);
    rename(unbroken);
    await page.viewport(320, 900);
    await page.goto(base + listPath);
    await page.waitFor(q('#relationship-title'), 'long unbroken bút danh from Product SSR');
    const identity = await page.eval(`(() => {
      const heading = ${q('#relationship-title')};
      const lineHeight = parseFloat(getComputedStyle(heading).lineHeight);
      return { name: heading.textContent.trim(), label: heading.getAttribute('aria-label'),
        lineHeight, height: heading.clientHeight, contentHeight: heading.scrollHeight,
        overflow: document.documentElement.scrollWidth - innerWidth };
    })()`);
    check(identity.name === unbroken && identity.label.startsWith(unbroken + ' · ') &&
      identity.height <= identity.lineHeight * 2 + 1 && identity.contentHeight > identity.height && identity.overflow <= 1,
      '#605 long unbroken Vietnamese name overflows only inside its visual clamp');
    const { nodes } = await page.send('Accessibility.getFullAXTree');
    check(nodes.some(node => node.role?.value === 'heading' && node.name?.value === identity.label),
      '#605 clipped name is still read in full by the accessibility tree');
  } finally {
    rename(profB.pen_name);
  }

  // 6. Restore list=everyone temporarily to run 5-viewport / 2-theme / 100-200% layout + contrast audit on both profile & list.
  authorFixtureSql(`UPDATE author_profiles SET policies = '{
    "pen_name":"everyone","avatar":"everyone","bio":"everyone","date_of_birth":"self",
    "followers_list":"everyone","followers_count":"everyone",
    "following_list":"self","following_count":"self"
  }'::jsonb WHERE principal_id = ${literal(profB.principal_id)}`);
  await checkLayoutAndA11y(page, check, textContrast, `${base}/@${handleB}`, ['#follow-action', '#profile-followers-link'], 'follow-profile');
  await checkLayoutAndA11y(guest, check, textContrast, `${base}/@${handleB}/nguoi-theo-doi`, ['#rel-back-profile', '#rel-load-more', '.vk-rel-item a'], 'follow-list');

  // Create 15 edges for A so its real own cursor can be sent with B's cookie.
  const fixtureMembers = `SELECT follower_id FROM author_follows
    WHERE followee_id=${literal(profB.principal_id)} AND follower_id IN
      (SELECT principal_id FROM author_profiles WHERE current_handle LIKE 'ban-theo-doi-%')`;
  authorFixtureSql(`INSERT INTO author_follows
    (follower_id, followee_id, active, version, accepted_order, active_since)
    SELECT ${literal(profA.principal_id)}, follower_id, true, 1, 1, active_since
    FROM author_follows WHERE followee_id=${literal(profB.principal_id)} AND follower_id IN (${fixtureMembers})`);
  const raceTab = await openPage();
  const ownListUrl = `${base}/@${handleA}/dang-theo-doi`;
  const cookieA = (await cookies(page, base))[sessionCookie];
  const cookieB = (await cookies(other, base))[sessionCookie];
  check(cookieA && cookieB && cookieA.value !== cookieB.value, 'race fixture has two real browser sessions');
  const useCookie = async ({name, value, sameSite}) => {
    const {success} = await raceTab.send('Network.setCookie',
      {name, value, url: base, path: '/', secure: true, httpOnly: true, sameSite});
    check(success, 'scratch browser session cookie installed');
  };
  const ownFirst = await page.eval(`fetch(${JSON.stringify(ownFollowing)}).then(r => r.json())`);
  const cursorPath = `${ownFollowing}?cursor=${encodeURIComponent(ownFirst.next_cursor)}`;
  const bPage = await other.eval(`fetch(${JSON.stringify(ownFollowing)}).then(r => r.json())`);
  const bRestart = await other.eval(`fetch(${JSON.stringify(cursorPath)}).then(r => r.json())`);
  check(ownFirst.author?.handle === handleA && ownFirst.next_cursor &&
    bPage.state === 'page' && bPage.author.handle === handleB &&
    bRestart.state === 'restart' && bRestart.author.handle === handleB,
    'scratch App returns B Page and B Restart with A’s cursor');
  const checkPrivateFallback = async (label) => {
    await raceTab.waitFor(`${q('#relationship-private')} && !${q('#relationship-count')} &&
      !${q('.vk-rel-item')} && !${q('#rel-load-more')}`, label);
    check(await raceTab.eval(`location.pathname === ${JSON.stringify(new URL(ownListUrl).pathname)} &&
      !Array.from(document.querySelectorAll('h1')).some(h => h.textContent.includes(${JSON.stringify(bPage.author.pen_name)}))`),
      'private fallback still belongs to requested A URL');
  };
  try {
    const owner = await holdAnswer(raceTab, 'GET', `/api/v1/authors/${handleA}/relationship/me`);
    await raceTab.goto(ownListUrl);
    check(await owner.held() === 200, 'real Graph ownership read held before switching the cookie');
    const privateRead = captureRequest(raceTab, 'GET', ownFollowing);
    await useCookie(cookieB);
    await owner.finish(false, true);
    await checkPrivateFallback('B private Page cannot replace a list at A URL');
    check(privateRead.count() > 0, 'initial list attempted the private endpoint after A’s Graph answer');
    privateRead.close();

    await useCookie(cookieA);
    await raceTab.reload();
    await raceTab.waitFor(`${q('#rel-load-more')} && ${q('#relationship-count')}`,
      'A own Following offers its own append cursor again');
    const append = await holdAnswer(raceTab, 'GET', ownFollowing);
    await useCookie(cookieB);
    await raceTab.click('#rel-load-more');
    check(await append.held() === 200, 'private append gets a real reply after the cookie changes');
    await append.finish(false, true);
    await checkPrivateFallback('B private Restart cannot preserve or replace A’s private page');
    console.log('  ok  verified real scratch A/B Page and Restart across a cookie race');
  } finally {
    await useCookie(cookieA);
    authorFixtureSql(`UPDATE author_follows SET active=false, version=version+1,
      accepted_order=accepted_order+1, active_since=NULL
      WHERE follower_id=${literal(profA.principal_id)} AND followee_id IN (${fixtureMembers})`);
  }

  // 7. Hold a nonempty page from A. After B and a different A session, the fixture's
  // now-inactive edge is absent. Releasing the old successful response must not reinsert it.
  authorFixtureSql(`UPDATE author_follows SET active=true, version=version+1,
    accepted_order=accepted_order+1, active_since=clock_timestamp()
    WHERE follower_id=${literal(profA.principal_id)} AND followee_id=${literal(profB.principal_id)};`);
  const tab = await openPage();
  await tab.goto(`${base}/@${handleA}/dang-theo-doi`);
  const staleRow = `.vk-rel-item a[href="/@${handleB}"]`;
  await tab.waitFor(q(staleRow), 'A own following initially contains B');
  const heldOwn = await holdAnswer(tab, 'GET', '/api/v1/me/relationships/following');
  await tab.reload();
  check((await heldOwn.held()) === 200, 'A nonempty own-following read held before session switch');
  await switchActor('b');
  await tab.waitFor(q('#relationship-private'), 'B session cannot display A own Following');
  authorFixtureSql(`UPDATE author_follows SET active=false, version=version+1,
    accepted_order=accepted_order+1, active_since=NULL
    WHERE follower_id=${literal(profA.principal_id)} AND followee_id=${literal(profB.principal_id)};`);
  await switchActor('a');
  const currentEmpty = `${q('#relationship-count')}?.textContent?.trim() === '0' &&
    ${q('#relationship-empty h2')}?.textContent === 'Chưa theo dõi tác giả nào.' && !${q(staleRow)}`;
  await tab.waitFor(currentEmpty, 'new A session reads current empty own Following');
  await heldOwn.finish(false, true);
  check(await tab.eval(currentEmpty), 'late nonempty own-list ACK cannot restore a prior session’s edge');

  await page.goto(`${base}/@${handleB}`);
  await page.waitFor(`${q('#follow-action')}?.textContent?.trim() === 'Theo dõi'`, 'new A session can act on B again');
  await page.click('#follow-action');
  await page.waitFor(`${q('#profile-followers-link')}?.textContent?.includes('16 người theo dõi')`, 'write overlays an updated public list and count');
  authorFixtureSql(`UPDATE author_profiles SET policies = policies ||
    '{"followers_list":"self","followers_count":"self"}'::jsonb
    WHERE principal_id = ${literal(profB.principal_id)};`);
  await page.eval("window.dispatchEvent(new Event('focus'))");
  await page.waitFor(`${q('#author-profile-heading')} && !${q('#profile-followers-link')} && !${q('#profile-followers-count')}`,
    'focus refresh removes revoked link and count even after a confirmed Follow write');

  check(guest.errors.length === 0 && page.errors.length === 0 && other.errors.length === 0 && tab.errors.length === 0,
    `zero console/page errors during follow flows (${[...guest.errors, ...page.errors, ...other.errors, ...tab.errors].join(' | ')})`);

  // Clean up test edges (deactivating rather than deleting to satisfy retained-edge trigger)
  // and restore default profile policies so existing authorProfiles / authorHandles tests start clean.
  authorFixtureSql(`BEGIN;
    UPDATE author_follows
       SET active = false,
           version = version + 1,
           accepted_order = accepted_order + 1,
           active_since = NULL
     WHERE active;
    UPDATE author_profiles
       SET policies = '{
         "pen_name":"everyone","avatar":"everyone","bio":"everyone","date_of_birth":"self",
         "followers_list":"everyone","followers_count":"everyone",
         "following_list":"self","following_count":"self"
       }'::jsonb
     WHERE principal_id IN (${literal(profA.principal_id)}, ${literal(profB.principal_id)});
  COMMIT;`);
  await profileRecovery(guest, base, handleB, {check, textContrast});
}
