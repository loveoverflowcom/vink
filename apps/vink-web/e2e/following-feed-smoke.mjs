// #296: native password cookies, current Publication/Graph, CDP-delayed real responses.
import { createHash, randomUUID } from 'node:crypto';
import { authorFixtureSql } from './author-fixtures.mjs';
import { q } from './auth-smoke.mjs';
import { captureRequest } from './auth-browser-network.mjs';

const API = '/api/v1/me/following-feed';
const FEED = '/?mode=following';
const ROWS = "document.querySelectorAll('#following-cards > li').length";
const FRAME = 'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))';
const state = (name) => q(`[data-feed-state="${name}"]`);
const literal = (value) => "'" + String(value).replaceAll("'", "''") + "'";
const own = (page) => page.eval("fetch('/api/v1/me/profile').then(r => r.json())");
const rowLink = (id) => `#work-${id} .vk-public-row__link`;

function publish(author, count, prefix) {
  const source = 'Mưa rơi qua ô cửa.\n\nMùa cũ vẫn còn trong trí nhớ.';
  const ids = [];
  const form = randomUUID();
  const slug = `form-feed-${form.slice(0, 8)}`;
  const label = count > 1 ? 'Thơ' : 'Truyện ngắn';
  const sql = Array.from({length: count}, (_, i) => {
    const id = randomUUID();
    const snapshot = randomUUID();
    ids.push(id);
    const title = `${prefix} ${i + 1}`;
    const hash = createHash('sha256').update('vink-document-content');
    for (const text of ['default', title, source]) {
      const bytes = Buffer.from(text);
      const length = Buffer.alloc(8);
      length.writeBigUInt64BE(BigInt(bytes.length));
      hash.update(length).update(bytes);
    }
    return `
      INSERT INTO works(id, owner_id) VALUES (${literal(id)},${literal(author)});
      INSERT INTO revisions(id, work_id, title, document, content_hash, created_by)
      VALUES (${literal(id)},${literal(id)},${literal(title)},${literal(JSON.stringify({format:'default', source}))},
              ${literal(hash.digest('hex'))},${literal(author)});
      INSERT INTO work_metadata_snapshots(id, work_id, metadata_version, digest)
      VALUES (${literal(snapshot)},${literal(id)},1,work_metadata_digest(${literal(form)}, ARRAY[]::uuid[]));
      INSERT INTO work_metadata_snapshot_tags(snapshot_id, slot, tag_id, facet)
      VALUES (${literal(snapshot)},0,${literal(form)},'form');
      INSERT INTO publications(work_id,revision_id,generation,published_at,metadata_snapshot_id)
      VALUES (${literal(id)},${literal(id)},1,clock_timestamp(),${literal(snapshot)});`;
  });
  authorFixtureSql(`BEGIN;
    INSERT INTO tags(id,facet,label,slug,status) VALUES (${literal(form)},'form',${literal(label)},${literal(slug)},'active');
    INSERT INTO tag_names(slug,tag_id,kind) VALUES (${literal(slug)},${literal(form)},'canonical');
    ${sql.join('\n')}
    COMMIT;`);
  return ids;
}

async function toggle(control, base, handle, after) {
  await control.goto(`${base}/@${handle}`);
  await control.waitFor(`${q('#follow-action')}?.textContent.trim() === ${JSON.stringify(after ? 'Theo dõi' : 'Bỏ theo dõi')}`,
    'real profile shows current Follow intent');
  await control.click('#follow-action');
  await control.waitFor(`${q('#follow-action')}?.textContent.trim() === ${JSON.stringify(after ? 'Bỏ theo dõi' : 'Theo dõi')}`,
    'server confirmed Follow change');
}

async function layout(page, check, textContrast) {
  await page.send('Page.bringToFront');
  const measure = `(() => ({
    overflow: document.documentElement.scrollWidth - innerWidth,
    nested: document.querySelectorAll('#following-cards a a').length,
    gap: document.querySelector('#following-cards').getBoundingClientRect().top -
      document.querySelector('.vk-explore-modes').getBoundingClientRect().bottom,
    targets: Array.from(document.querySelectorAll('.vk-explore-modes a, #following-cards .vk-ui-byline__identity'))
      .slice(0,4).map(el => [el.getBoundingClientRect().width, el.getBoundingClientRect().height])
  }))()`;
  for (const width of [320, 390, 834, 1024, 1440]) {
    await page.viewport(width, 900);
    for (const theme of ['light', 'dark']) {
      for (const text of [100, 200]) {
        await page.eval(`document.documentElement.dataset.theme = '${theme}';
          document.documentElement.style.fontSize = '${text}%'; document.fonts.ready.then(() => true)`);
        await page.eval(FRAME);
        const got = await page.eval(measure);
        check(got.overflow <= 1 && !got.nested && got.gap >= 16 && got.targets.length === 4 &&
          got.targets.every(([w, h]) => w >= 44 && h >= 44),
          `#296 ${width}/${theme}/${text}% overflow, links and touch targets (${JSON.stringify(got)})`);
        if (text === 100 && [390, 1440].includes(width)) {
          await textContrast(page, theme, 'following-feed', ['#explore-mode-works', '#explore-mode-following', '.vk-feed-date']);
          await page.capture(`feed-296-${width}-${theme}`, {
            requiredTargets: ['#explore-mode-following', '#following-cards', '.vk-feed-date'],
          });
        }
      }
    }
  }
  await page.eval("document.documentElement.style.fontSize = ''; document.documentElement.dataset.theme = 'light'");
  await page.viewport(1280, 900);
}

export async function followingFeed(control, other, {base, openPage, check, holdAnswer, switchActor, textContrast}) {
  console.log('Following Feed (#296):');
  const a = await own(control);
  const b = await own(other);
  check(a.handle && b.handle && a.handle !== b.handle, 'separate native accounts for Feed');
  const {browserContextId} = await control.cdp.send('Target.createBrowserContext');
  const guest = await openPage(browserContextId);
  const page = await openPage(); // shares the App cookie with control, and listens for ACK broadcasts.
  await guest.goto(base + '/');
  await guest.waitFor(q('#explore-mode-following'), 'Explore exposes Following mode');
  await guest.eval(`${q('#explore-mode-following')}.focus()`);
  await guest.key('Enter');
  await guest.waitFor(state('guest'), 'keyboard opens the guest Feed gate');
  const target = await guest.eval(`${q('#feed-sign-in')}.getAttribute('href')`);
  check(new URL(target, base).searchParams.get('next') === FEED, 'guest sign-in maintains the Feed return context');
  await guest.click('#feed-sign-in');
  await guest.waitFor("location.pathname==='/toi' && !!document.querySelector('#password-login')", 'CTA reaches real auth');

  await page.goto(base + target);
  await page.waitFor(q('#me-return-link'), 'active native account can resume the guest route');
  await page.click('#me-return-link');
  await page.waitFor(state('no-follows'), 'account with no active follows gets its own empty state');
  await toggle(control, base, b.handle, true);
  await page.waitFor(state('no-works'), 'confirmed Follow updates a live tab; author without eligible work is distinct');

  const bWorks = publish(b.principal_id, 16, 'Feed riêng · tác phẩm đã theo dõi');
  const aWorks = publish(a.principal_id, 1, 'Tác phẩm ngoài Feed');
  const withdraw = id => authorFixtureSql(`DELETE FROM publications WHERE work_id=${literal(id)};`);
  await page.send('Page.bringToFront');
  await page.eval("window.dispatchEvent(new Event('focus'))");
  await page.waitFor(`${ROWS}===12 && ${q('#feed-load-more')}`, 'fresh Feed shows the bounded first page');
  const ssr = await page.eval(`fetch(${JSON.stringify(FEED)}).then(async response => ({
    cache: response.headers.get('cache-control'), html: await response.text()
  }))`);
  check(ssr.cache?.includes('no-store') && ssr.html.includes('data-feed-state="checking"') &&
    !ssr.html.includes('tác phẩm đã theo dõi') && !ssr.html.includes(a.principal_id),
    'signed-in SSR HTML has an actor-free placeholder and no-store');
  check(await page.eval(`${q('#explore-mode-following')}.getAttribute('aria-current') === 'page' &&
    ${q('#following-cards time[datetime]')} && ${q('#following-cards .vk-public-row__tag')}?.textContent==='Thơ' &&
    !document.body.textContent.includes('Tác phẩm ngoài Feed')`),
    'browser displays first-public dates and bound public tags, and omits the viewer’s unrelated publication');
  await layout(page, check, textContrast);

  const failed = await holdAnswer(page, 'GET', API);
  await page.click('#feed-load-more');
  check(await failed.held() === 200, 'append failure starts from a real server result');
  await failed.finish(true);
  await page.waitFor(`${state('append-error')} && ${ROWS}===12`, 'append can fail while prior cards remain');

  const removed = await page.eval("document.querySelector('#following-cards > li').dataset.workId");
  withdraw(removed);
  await page.click('#feed-load-more');
  await page.waitFor(`${ROWS}===15 && !${q(rowLink(removed))}`, 'next page revalidates previously displayed rows');
  const anchor = await page.eval("Array.from(document.querySelectorAll('#following-cards > li')).at(-2).dataset.workId");
  const selector = rowLink(anchor);
  const sameDocument = await page.eval('performance.timeOrigin');
  const enter = async () => {
    await page.eval(`${q(selector)}.focus()`);
    await page.key('Enter');
    await page.waitFor(`location.pathname === '/tac-pham/${anchor}' && ${q('#reader-paper')}`, 'keyboard WorkCard opens real Reader');
  };
  await enter();
  check(await page.eval(`${q('#reader-return')}.getAttribute('href') === '${FEED}#work-${anchor}'`),
    'Reader return link keeps the mode and anchor, with no private query cursor');
  await page.eval('history.back()');
  await page.waitFor(`location.search === '?mode=following' && document.activeElement === ${q(selector)}`, 'Back validates and focuses a row from the appended page');
  check(await page.eval(`performance.timeOrigin === ${sameDocument}`), 'Feed → Reader → Back uses the hydrated document');
  await enter();
  withdraw(anchor);
  await page.eval('history.back()');
  await page.waitFor(`${ROWS}===12 && !${q(selector)} && document.activeElement?.id === 'community-stream'`,
    'Back to revoked anchor uses the named result region');
  await page.eval('history.forward()');
  await page.waitFor(`location.pathname === '/tac-pham/${anchor}' &&
    document.body.textContent.includes('Không tìm thấy trang này.') && !${q('#reader-paper')}`, 'Forward cannot read a withdrawn work');
  await page.eval('history.back()');
  await page.waitFor(`${ROWS}===12 && ${q('#feed-load-more')}`, 'second Back re-establishes live head');

  // Drive the profile and Reader through the actual history stack. On the first
  // popstate go Back again in the same event, while the intermediate page is pending.
  const authorLink = "#following-cards > li:first-child .vk-ui-byline__identity";
  await page.eval(`${q(authorLink)}.focus()`);
  await page.key('Enter');
  await page.waitFor(`location.pathname === '/@${b.handle}' && ${q('#author-profile-heading')}`, 'Feed byline opens profile');
  await page.eval('history.back()');
  await page.waitFor(`${ROWS}===12 && document.activeElement === ${q(authorLink)}`, 'Back from profile focuses the live Feed byline');
  const workLink = '#following-cards > li:first-child .vk-public-row__link';
  await page.eval(`${q(workLink)}.focus()`);
  await page.key('Enter');
  await page.waitFor(q('#reader-author'), 'Reader author link is present');
  const profile = await holdAnswer(page, 'GET', '/api/v1/authors/' + b.handle);
  await page.click('#reader-author');
  check(await profile.held() === 200, 'real profile response is held before rendering');
  await page.eval("window.addEventListener('popstate', () => setTimeout(() => history.back(), 0), {once:true}); history.back()");
  await page.waitFor(`location.pathname==='/' && location.search === '?mode=following'`, 'rapid history traversal returns to the Feed URL');
  await profile.finish(false, true);
  await page.waitFor(`${ROWS}===12 && ${q('#explore-mode-following')} && !${q('#author-profile-heading')} && !${q('#reader-paper')}`,
    'late intermediate profile cannot replace the Feed at its history URL');

  // Real document traversal matching #293: a second Back before the intermediate
  // Profile document hydrates must not start the router against the wrong SSR DOM.
  await page.click(authorLink);
  await page.waitFor(`location.pathname === '/@${b.handle}' && ${q('#author-profile-heading')}`, 'profile ready before new document');
  await page.goto(base + '/?q=khong-co-bai-feed-296');
  await page.send('Network.setCacheDisabled', {cacheDisabled:true});
  const wasm = await holdAnswer(page, 'GET', '/pkg/vink_bg.wasm');
  await page.eval('history.back()');
  await page.waitFor(`location.pathname === '/@${b.handle}' && ${q('#author-profile-heading')} &&
    !document.documentElement.dataset.hydrated`, 'first Back reaches unhydrated profile SSR');
  check(await wasm.held() === 200, 'profile WASM is held at the real response boundary');
  await page.eval('history.back()');
  await page.waitFor("location.pathname==='/' && location.search==='?mode=following'",
    'second Back changes the URL before profile hydration');
  await wasm.finish(false);
  await page.send('Network.setCacheDisabled', {cacheDisabled:false});
  await page.waitFor(`${ROWS}===12 && document.documentElement.dataset.hydrated==='true' &&
    !${q('#author-profile-heading')}`, 'two-document Back hydrates the current Feed instead of the stale profile');
  await page.waitFor(`document.activeElement === ${q(authorLink)} ||
    document.activeElement?.id==='community-stream'`, 'two-document Back restores a valid focus target');

  const publicReads = captureRequest(page, 'GET', '/api/v1/works');
  const append = await holdAnswer(page, 'GET', API);
  await page.click('#feed-load-more');
  check(await append.held() === 200, 'hold real append before the Graph ACK');
  await toggle(control, base, b.handle, false);
  await page.waitFor(`${state('no-follows')} && ${ROWS}===0`, 'confirmed Unfollow fences the open chain');
  await page.eval(`${q('#explore-mode-works')}.focus()`);
  await append.finish(false, true);
  check(await page.eval(`${ROWS}===0 && document.activeElement?.id==='explore-mode-works'`),
    'old append cannot restore rows or move focus after Unfollow');

  await toggle(control, base, b.handle, true);
  await page.waitFor(`${ROWS}===12 && ${q('#feed-load-more')}`, 'new Follow restarts from the current head');
  const oldSession = await holdAnswer(page, 'GET', API);
  await page.click('#feed-load-more');
  check(await oldSession.held() === 200, 'A append is held before the native session changes');
  await switchActor('b', control);
  await page.waitFor(`${state('no-follows')} && ${ROWS}===0`, 'B never renders A’s page chain');
  await switchActor('a', control);
  await page.waitFor(`${ROWS}===12`, 'new session for A begins with the first page');
  await oldSession.finish(false, true);
  check(await page.eval(`${ROWS}===12 && location.search==='?mode=following'`), 'stale A completion cannot append in the new A session');
  check(publicReads.count() === 0, 'the private mode never requests the public collection endpoint');
  publicReads.close();

  // Revoke the real credential server side without the UI's BroadcastChannel notification.
  const logout = await page.eval("fetch('/api/v1/auth/session',{method:'DELETE'}).then(r=>r.status)");
  check(logout === 204, 'native authority revoked the active Feed session');
  await page.click('#feed-load-more');
  await page.waitFor(`${state('expired')} && ${ROWS}===0`, 'a Feed 401 clears cards and has a distinct expired-session action');
  await switchActor('a', control, 'anonymous');

  authorFixtureSql(`DELETE FROM publications WHERE work_id IN (${[...bWorks, ...aWorks].map(literal).join(',')});`);
  await toggle(control, base, b.handle, false);
  check(page.errors.length === 0 && guest.errors.length === 0,
    `Feed tabs have no console or hydration errors: ${[...page.errors,...guest.errors].join(' | ')}`);
  await control.cdp.send('Target.disposeBrowserContext', {browserContextId});
  await control.send('Page.bringToFront');
  await control.goto(base + '/toi');
}
