// #294 Social Follow Graph E2E: real Chrome, password sessions, App and scratch PostgreSQL.
import { randomUUID } from 'node:crypto';
import { authorFixtureSql } from './author-fixtures.mjs';
import { q } from './auth-smoke.mjs';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const literal = (text) => "'" + String(text).replaceAll("'", "''") + "'";
const ownProfile = (p) => p.eval("fetch('/api/v1/me/profile').then(r => r.json())");

async function checkLayoutAndA11y(page, check, textContrast, url, contrastSelectors, label) {
  await page.goto(url);
  await page.waitFor(q(contrastSelectors[0]), `${label} loaded`);
  for (const width of [320, 390, 834, 1024, 1440]) {
    for (const theme of ['light', 'dark']) {
      for (const scale of [100, 200]) {
        await page.viewport(width);
        const facts = await page.eval(`(() => {
          const root = document.documentElement;
          root.dataset.theme = '${theme}';
          root.style.fontSize = '${scale}%';
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
            t.height >= 44,
            `${label} touch target ${t.sel} height >= 44px at ${width}px ${theme} ${scale}% (got ${t.height})`
          );
        }
        if (width === 390 && scale === 100) {
          await page.waitFor(
            `!document.getAnimations().some(a => a.playState === 'running' &&
              a.effect?.target?.checkVisibility?.() && Number.isFinite(a.effect.getComputedTiming().endTime))`,
            `${label} ${theme} settled`
          );
          await textContrast(page, theme, label, contrastSelectors);
        }
      }
    }
  }
  await page.eval("document.documentElement.style.fontSize = ''; delete document.documentElement.dataset.theme");
  await page.viewport(1280);
}

export async function authorFollows(page, other, { base, openPage, check, holdAnswer, textContrast, switchActor }) {
  console.log('Social Follow Graph (#294):');
  const profA = await ownProfile(page);
  const profB = await ownProfile(other);
  const handleA = profA.handle;
  const handleB = profB.handle;
  check(Boolean(handleA && handleB && handleA !== handleB), 'distinct initial author handles for A and B');

  // 1. SSR HTML & Guest CTA -> /toi?next= return + open-redirect rejection.
  const guest = await openPage();
  const ssrCheck = await guest.eval(`fetch(${JSON.stringify(`/@${handleB}`)}).then(async r => ({ status: r.status, text: await r.text() }))`);
  check(ssrCheck.status === 200 && ssrCheck.text.includes('profile-followers-link') && !ssrCheck.text.includes('Theo dõi lẫn nhau'),
    'SSR public profile renders followers link without mutual/viewer secrets');

  await guest.goto(`${base}/@${handleB}`);
  await guest.waitFor(q('#follow-sign-in'), 'guest sees follow sign-in CTA');
  const signInHref = await guest.eval(`${q('#follow-sign-in')}.getAttribute('href')`);
  check(signInHref === `/toi?next=%2F%40${encodeURIComponent(handleB)}`, `guest CTA points to /toi?next=... (${signInHref})`);

  await page.goto(`${base}/toi?next=${encodeURIComponent(`/@${handleB}`)}`);
  await page.waitFor(q('#me-return-link'), 'active session shows validated internal return link on /toi');
  check((await page.eval(`${q('#me-return-link')}.getAttribute('href')`)) === `/@${handleB}`, 'return link targets canonical author profile');

  await page.goto(`${base}/toi?next=https://evil.example/%40${handleB}`);
  await page.waitFor(q('#me-title'), 'Me loads with rejected external next');
  check(await page.eval(`!${q('#me-return-link')}`), 'open redirect next URL is rejected');

  // 2. Owner profile: no self-follow button; own Following link/count & /dang-theo-doi accessible even with following_list=self.
  await page.goto(`${base}/@${handleA}`);
  await page.waitFor(q('#profile-following-link'), 'owner sees Following link despite default self policy');
  check(await page.eval(`!${q('#follow-action')}`), 'owner sees no self-follow button on own profile');
  await page.click('#profile-following-link');
  await page.waitFor(q('#relationship-title'), 'owner navigates to own following list');
  check(await page.eval(`${q('#relationship-title')}.textContent.includes('đang theo dõi')`), 'own following page heading renders');

  // 3. Keyboard Follow/Unfollow keeping focus + Mutual matrix across A, B and Guest.
  await page.goto(`${base}/@${handleB}`);
  await page.waitFor(q('#follow-action'), 'member sees #follow-action on other profile');
  await page.eval(`${q('#follow-action')}.focus()`);
  await page.key('Enter');
  await page.waitFor(`${q('#follow-action')}?.textContent?.trim() === 'Bỏ theo dõi'`, 'keyboard Follow commits');
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

  // Touch / click Unfollow back to clean 0 edges for ABA test.
  await page.click('#follow-action');
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

  const heldFirst = await holdAnswer(page, 'PUT', `/api/v1/authors/${handleB}/relationship/me`);
  await page.click('#follow-action');
  check((await heldFirst.held()) === 200, 'real server committed PUT #1 (order 1) before ACK was held');
  check((await page.eval('window.__followEvents.length')) === 0, 'vink:follow-changed does NOT fire while ACK is pending');

  // Slow pending reveals #follow-reconcile after ~1s; clicking duplicate #follow-action does not double-send.
  await page.waitFor(q('#follow-reconcile'), 'reconcile control appears during slow/lost ACK');
  await page.click('#follow-reconcile');
  await page.waitFor(
    `${q('#follow-action')}?.textContent?.trim() === 'Bỏ theo dõi' && window.__followEvents.length === 1`,
    'reconciliation updates state cleanly and dispatches one invalidation event'
  );
  check(await page.eval(`${q('#profile-followers-link')}?.textContent?.includes('1 người theo dõi')`), 'reconcile updated followers count to 1');

  // Unfollow (order 2).
  await page.click('#follow-action');
  await page.waitFor(
    `${q('#follow-action')}?.textContent?.trim() === 'Theo dõi' && window.__followEvents.length === 2`,
    'Unfollow (order 2) settles and dispatches second invalidation event'
  );
  check(await page.eval(`${q('#profile-followers-link')}?.textContent?.includes('0 người theo dõi')`), 'count decremented to 0 after Unfollow');

  // Release delayed ACK of PUT #1 (order 1): MUST be completely ignored!
  await heldFirst.finish(false, true);
  await sleep(150);
  check(await page.eval(`${q('#follow-action')}?.textContent?.trim() === 'Theo dõi'`), 'stale late v1 ACK ignored; button remains Theo dõi');
  check(await page.eval(`${q('#profile-followers-link')}?.textContent?.includes('0 người theo dõi')`), 'stale late v1 ACK ignored; count remains 0');
  check((await page.eval('window.__followEvents.length')) === 2, 'stale late v1 ACK does not emit extra invalidation');

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
  await guest.waitFor(`document.querySelectorAll('.vk-rel-item').length === 12`, 'page 1 shows 12 followers');
  check(await guest.eval(`!${q('#relationship-count')}`), 'list page omits count when followers_count=self');

  // Simulate append failure -> existing 12 rows stay mounted + retry button works -> 15 rows load!
  const dropAppend = await holdAnswer(guest, 'GET', `/api/v1/authors/${handleB}/followers`);
  await guest.click('#rel-load-more');
  check((await dropAppend.held()) === 200, 'real page 2 held before network failure injection');
  await dropAppend.finish(true);
  await guest.waitFor(q('#rel-append-retry'), 'append error shows retry button');
  check((await guest.eval(`document.querySelectorAll('.vk-rel-item').length`)) === 12, 'append failure retains existing 12 rows');
  await guest.click('#rel-append-retry');
  await guest.waitFor(`document.querySelectorAll('.vk-rel-item').length === 15 && !${q('#rel-load-more')}`, 'append retry loads remaining 3 rows (total 15)');

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
  check((await guest.eval(`document.querySelectorAll('.vk-rel-item').length`)) === 0, 'mid-list revocation clears all previously loaded rows');

  await guest.goto(`${base}/@${handleB}`);
  await guest.waitFor(q('#profile-followers-count'), 'profile renders count span when list=self and count=everyone');
  check(await guest.eval(`!${q('#profile-followers-link')} && ${q('#profile-followers-count')}.textContent.includes('15 người theo dõi')`),
    'profile renders count without link when list=self and count=everyone');

  // 6. Restore list=everyone temporarily to run 5-viewport / 2-theme / 100-200% layout + contrast audit on both profile & list.
  authorFixtureSql(`UPDATE author_profiles SET policies = '{
    "pen_name":"everyone","avatar":"everyone","bio":"everyone","date_of_birth":"self",
    "followers_list":"everyone","followers_count":"everyone",
    "following_list":"self","following_count":"self"
  }'::jsonb WHERE principal_id = ${literal(profB.principal_id)}`);
  await checkLayoutAndA11y(page, check, textContrast, `${base}/@${handleB}`, ['#follow-action', '#profile-followers-link'], 'follow-profile');
  await checkLayoutAndA11y(guest, check, textContrast, `${base}/@${handleB}/nguoi-theo-doi`, ['#rel-back-profile', '#rel-load-more'], 'follow-list');

  // 7. A -> B -> A session switch in another tab during a held own-relationship read drops stale private payload cleanly.
  const tab = await openPage();
  await tab.goto(`${base}/@${handleA}/dang-theo-doi`);
  await tab.waitFor(q('#relationship-title'), 'tab opens A own following list');
  const heldOwn = await holdAnswer(tab, 'GET', '/api/v1/me/relationships/following');
  await tab.reload();
  check((await heldOwn.held()) === 200, 'A own-following read held in tab before A->B->A switch');
  await switchActor('b');
  await switchActor('a');
  await heldOwn.finish(false, true);
  await tab.waitFor(q('#relationship-title'), 'tab recovers cleanly after A->B->A session switch');

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
}
