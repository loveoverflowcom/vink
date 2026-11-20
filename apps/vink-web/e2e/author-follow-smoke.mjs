// #294 Social Follow Graph E2E: real Chrome, password sessions, App and scratch PostgreSQL.
import { randomUUID } from 'node:crypto';
import { authorFixtureSql } from './author-fixtures.mjs';
import { q } from './auth-smoke.mjs';
import { captureRequest } from './auth-browser-network.mjs';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const literal = (text) => "'" + String(text).replaceAll("'", "''") + "'";
const ownProfile = (p) => p.eval("fetch('/api/v1/me/profile').then(r => r.json())");

async function checkLayoutAndA11y(page, check, textContrast, url, contrastSelectors, label) {
  await page.goto(url);
  await page.waitFor(contrastSelectors.map(q).join(' && '), `${label} loaded`);
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
          return { overflow, nestedLinks, targets };
        })()`);
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
          await page.eval(`${q(contrastSelectors[0])}.scrollIntoView({block:'start'})`);
          await page.capture(`${label}-${width}-${theme}-${scale}`,
            {scenario:label, textScale:scale, requiredTargets:contrastSelectors});

        }
      }
    }
  }
  await page.eval("document.documentElement.style.fontSize = ''; delete document.documentElement.dataset.theme");
  await page.viewport(1280);
}

async function profileRecovery(guest, base, handle, { check, textContrast }) {
  const missing = `${base}/@chu-chua-co-nguoi-o`;
  const lookup = `${base}/@${handle}?cursor=tiep`;
  const heading = '#author-profile-heading';
  const retry = '#profile-retry';
  const response = (url, method) => fetch(url, {method, redirect: 'manual'});
  for (const method of ['GET', 'HEAD']) {
    check((await response(missing, method)).status === 404, `${method} to unknown profile returns 404`);
  }
  await guest.goto(missing);
  check(await guest.eval(`${q(heading)}?.textContent === 'Không tìm thấy hồ sơ' &&
    ${q('#profile-unavailable-explore')}?.getAttribute('href') === '/'`), '404 explains the address and links to Khám phá');

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
      await guest.goto(missing);
      check(await guest.eval(`${q('#profile-unavailable-explore')}?.getAttribute('href') === '/' &&
        !document.documentElement.dataset.hydrated`), '404 offers Khám phá with JavaScript disabled');
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

export async function authorFollows(page, other, { base, openPage, check, holdAnswer, textContrast, switchActor }) {
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
  await page.goto(`${base}/@${handleA}`);
  await page.waitFor(q('#profile-following-link'), 'owner sees Following link despite default self policy');
  await page.waitFor(q('#own-profile-edit-link'), 'owner sees the Edit Profile action');
  check(await page.eval(`!${q('#follow-action')} && ${q('#own-profile-edit-link')}.getAttribute('href') === '/toi/chinh-ho-so'`),
    'owner can edit the profile and sees no self-follow action');
  await page.click('#profile-following-link');
  await page.waitFor(q('#relationship-title'), 'owner navigates to own following list');
  check(await page.eval(`${q('#relationship-title')}.textContent.includes('đang theo dõi')`), 'own following page heading renders');
  await page.waitFor("document.activeElement?.id === 'relationship-title'", 'following list navigation focuses its heading');
  await page.eval('history.back()');
  await page.waitFor("document.activeElement?.id === 'profile-following-link'", 'Back restores the profile Following link');


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

  // 5. 15 followers + independent 4-policy matrix + append retry + mid-list live permission loss.
  const followerSql = [];
  for (let i = 0; i < 15; i++) {
    const uid = randomUUID();
    const h = `ban-theo-doi-${i.toString().padStart(2, '0')}-${randomUUID().slice(0, 6)}`;
    followerSql.push(`
      INSERT INTO principals (id, display_name) VALUES (${literal(uid)}, ${literal(`Bạn đọc ${i + 1}`)});
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
  check(await guest.eval(`!${q('#relationship-count')}`), 'list page omits count when followers_count=self');

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
      check(await guest.eval(`Boolean(${q('#relationship-count')}) === ${count === 'everyone'} && Boolean(${q('#relationship-empty')})`),
        'empty Following and real zero count are distinct from a hidden count');
      await guest.eval('history.back()');
      await guest.waitFor("document.activeElement?.id === 'rel-other-direction'", 'Back returns to the authorized direction link');
    }
  }
  authorFixtureSql(`UPDATE author_profiles SET policies = policies ||
    '{"followers_list":"everyone","followers_count":"self"}'::jsonb
    WHERE principal_id = ${literal(profB.principal_id)};`);
  await checkLayoutAndA11y(guest, check, textContrast, `${base}/@${handleB}`, ['#profile-followers-link'], 'follow-list-without-count');

  // 6. Restore list=everyone temporarily to run 5-viewport / 2-theme / 100-200% layout + contrast audit on both profile & list.
  authorFixtureSql(`UPDATE author_profiles SET policies = '{
    "pen_name":"everyone","avatar":"everyone","bio":"everyone","date_of_birth":"self",
    "followers_list":"everyone","followers_count":"everyone",
    "following_list":"self","following_count":"self"
  }'::jsonb WHERE principal_id = ${literal(profB.principal_id)}`);
  await checkLayoutAndA11y(page, check, textContrast, `${base}/@${handleB}`, ['#follow-action', '#profile-followers-link'], 'follow-profile');
  await checkLayoutAndA11y(guest, check, textContrast, `${base}/@${handleB}/nguoi-theo-doi`, ['#rel-back-profile', '#rel-load-more', '.vk-rel-item a'], 'follow-list');

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
  const currentEmpty = `${q('#relationship-count')}?.textContent?.trim() === '0 đang theo dõi' && !${q(staleRow)}`;
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
