// #296: native password cookies, current Publication/Graph, CDP-delayed real responses.
import { createHash, randomUUID } from 'node:crypto';
import { authorFixtureSql } from './author-fixtures.mjs';
import { fill, q } from './auth-smoke.mjs';
import { captureRequest } from './auth-browser-network.mjs';

const API = '/api/v1/me/following-feed';
const FEED = '/?mode=following';
const AUTHORS = '/?mode=authors';
const POINTER_TIME = '2026-11-19T18:00:00Z';
const HEADING = '#following-feed-heading';
const EMPTY_HEADING = '#following-feed-empty-heading';
const NO_FOLLOWS = '[data-feed-state="no-follows"]';
const DISCOVER_AUTHORS = NO_FOLLOWS + ' a';
const ROWS = "document.querySelectorAll('#following-cards > li').length";
const FRAME = 'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))';
const state = (name) => q(`[data-feed-state="${name}"]`);
const literal = (value) => "'" + String(value).replaceAll("'", "''") + "'";
const own = (page) => page.eval("fetch('/api/v1/me/profile').then(r => r.json())");
const rowLink = (id) => `#work-${id} .vk-public-row__link`;

function publish(author, count, prefix) {
  const poem = 'Mưa rơi qua ô cửa.\nGió khuya lay giàn hoa.\n\nMùa cũ vẫn còn trong trí nhớ.';
  const ids = [];
  const form = randomUUID();
  const [label, slug] = count > 1 ? ['Thơ', 'thơ'] : ['Truyện ngắn', 'truyện-ngắn'];
  const sql = Array.from({length: count}, (_, i) => {
    const id = randomUUID();
    const snapshot = randomUUID();
    ids.push(id);
    const title = `${prefix} ${i + 1}`;
    const source = count > 1 && i === 0 ? poem + '\n' + 'Mưa vẫn rơi bên hiên nhà cũ.\n'.repeat(12) : poem;
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
      VALUES (${literal(id)},${literal(id)},1,${literal(POINTER_TIME)}::timestamptz + ${i} * interval '1 minute',${literal(snapshot)});`;
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

async function focusEvidence(page, selector, check, name, {capture = true} = {}) {
  const facts = await page.eval(`(() => {
    const target = ${q(selector)};
    const selector = ${JSON.stringify(selector)};
    const style = getComputedStyle(target, selector.includes('vk-public-row__link') ? '::after' : null);
    const bg = getComputedStyle(document.body).backgroundColor;
    const light = color => color.match(/[0-9.]+/g).slice(0, 3).map(value => value / 255)
      .map(value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4)
      .reduce((sum, value, i) => sum + [0.2126, 0.7152, 0.0722][i] * value, 0);
    const contrastWith = background => {
      const [high, low] = [light(style.outlineColor), light(background)].sort((a, b) => b - a);
      return (high + 0.05) / (low + 0.05);
    };
    const ratio = target.matches('a.vk-ui-button')
      ? Math.min(contrastWith(bg), contrastWith(style.backgroundColor))
      : contrastWith(bg);
    const {top, bottom} = target.getBoundingClientRect();
    const nav = document.querySelector('#bottom-navigation').getBoundingClientRect();
    return {focused: document.activeElement === target, outline: parseFloat(style.outlineWidth),
      style: style.outlineStyle, color: style.outlineColor, bg, ratio, top, bottom,
      limit: nav.height > 0 ? nav.top : innerHeight};
  })()`);
  check(facts.focused && facts.outline >= 2 && facts.style !== 'none' && facts.ratio >= 3 &&
    facts.top >= 0 && facts.bottom <= facts.limit, `${name} visible focus and contrast (${JSON.stringify(facts)})`);
  if (capture) await page.capture(name, { requiredTargets: [selector] });
}

async function setFeedAppearance(page, theme, text) {
  await page.eval(`document.documentElement.dataset.theme = '${theme}';
    if (${text} === 150) document.documentElement.dataset.text = '150';
    else delete document.documentElement.dataset.text;
    document.documentElement.style.fontSize = ${text} === 200 ? '200%' : '';
    scrollTo(0, 0); document.fonts.ready.then(() => true)`);
  await page.eval(FRAME);
  // Measure both colors after the background transition has reached the selected theme.
  await page.eval(`Promise.allSettled(document.querySelector('#explore').getAnimations({subtree: true})
    .filter(animation => animation instanceof CSSTransition).map(animation => animation.finished)).then(() => true)`);
}

async function noFollowsLayout(page, check, textContrast) {
  await page.send('Page.bringToFront');
  const measure = `(() => {
    const region = ${q('#community-stream')};
    const panel = ${q(NO_FOLLOWS)};
    const heading = ${q(EMPTY_HEADING)};
    const action = ${q(DISCOVER_AUTHORS)};
    const style = getComputedStyle(action);
    const rect = element => element.getBoundingClientRect();
    const midpoint = element => (rect(element).left + rect(element).right) / 2;
    const within = (child, parent) => rect(child).left >= rect(parent).left &&
      rect(child).right <= rect(parent).right;
    return {
      overflow: Math.max(0, document.documentElement.scrollWidth - document.documentElement.clientWidth),
      tabs: Array.from(document.querySelectorAll('.vk-explore-modes a'), tab =>
        [tab.id, tab.scrollWidth <= tab.clientWidth, tab.getBoundingClientRect().height >= 44]),
      centerDelta: Math.abs(midpoint(action) - midpoint(region)),
      height: rect(action).height,
      radius: style.borderTopLeftRadius,
      decoration: style.textDecorationLine,
      title: heading.textContent.trim(),
      link: action.textContent.trim(),
      checked: action.getAttribute('href') === '${AUTHORS}' && panel.querySelectorAll('a').length === 1 &&
        within(action, region) && within(heading, region) && within(panel.querySelector('p'), region) &&
        within(action.querySelector('.vk-ui-button__label'), action) &&
        !${q(HEADING)}.getClientRects().length &&
        style.backgroundColor !== 'rgba(0, 0, 0, 0)' && getComputedStyle(panel).boxShadow === 'none' &&
        panel.querySelectorAll('svg[aria-hidden="true"]').length === 2,
    };
  })()`;
  for (const width of [320, 390, 834]) {
    await page.viewport(width, 900);
    for (const theme of ['light', 'dark']) {
      for (const text of [100, 150, 200]) {
        const name = `feed-355-r1-after-${width}-${theme}-${text}`;
        await setFeedAppearance(page, theme, text);
        const got = await page.eval(measure);
        check(got.tabs.map(([id]) => id).join(',') === 'explore-mode-works,explore-mode-following,explore-mode-authors' &&
          got.tabs.every(([, fits, reachable]) => fits && reachable), `${name}: three persistent tabs fit (${JSON.stringify(got.tabs)})`);
        check(got.checked && got.overflow === 0 && got.centerDelta <= 1 && got.height >= 48 &&
          got.radius === '14px' && got.decoration === 'none' && got.title === 'Chưa theo dõi tác giả nào' &&
          got.link === 'Khám phá tác giả', `${name}: centered empty state and tonal link (${JSON.stringify(got)})`);
        const inView = await page.eval(`(() => {
          const action = ${q(DISCOVER_AUTHORS)};
          const bar = ${q('#bottom-navigation')}.getBoundingClientRect();
          const limit = bar.height > 0 ? bar.top : innerHeight;
          if (action.getBoundingClientRect().bottom > limit) action.scrollIntoView({block: 'end'});
          const rect = action.getBoundingClientRect();
          return rect.top >= 0 && rect.bottom <= limit &&
            action.contains(document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2));
        })()`);
        check(inView, `${name}: the full link can scroll clear of the bottom bar`);
        if (text !== 150 || width === 320) {
          await page.capture(name, {requiredTargets: [EMPTY_HEADING, DISCOVER_AUTHORS],
            textScaleLane: text === 200 ? 'dom-override-geometry-only' : 'app-text-scale'});
        }
        await textContrast(page, theme, 'following-empty', [EMPTY_HEADING, NO_FOLLOWS + ' p', DISCOVER_AUTHORS]);
        await page.eval(`${q(DISCOVER_AUTHORS)}.focus()`);
        await page.key('Tab', 8);
        await page.key('Tab');
        await focusEvidence(page, DISCOVER_AUTHORS, check, `${name}-focus`, {capture: width === 320 && text === 200});
        await page.eval('document.activeElement.blur()');
      }
    }
  }
  const {nodes} = await page.send('Accessibility.getFullAXTree');
  check(nodes.some(node => node.role?.value === 'region' && node.name?.value === 'Đang theo dõi') &&
    nodes.some(node => node.role?.value === 'heading' && node.name?.value === 'Chưa theo dõi tác giả nào') &&
    nodes.some(node => node.role?.value === 'link' && node.name?.value === 'Khám phá tác giả'),
    'no-follows exposes the named region, current heading and native link in the accessibility tree');
  await setFeedAppearance(page, 'light', 100);
  await page.viewport(390, 900);
}

async function layout(page, check, textContrast) {
  await page.send('Page.bringToFront');
  const measure = `(() => ({
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    nested: document.querySelectorAll('#following-cards a a').length,
    gap: document.querySelector('#following-cards').getBoundingClientRect().top -
      document.querySelector('.vk-explore-modes').getBoundingClientRect().bottom,
    nav: (() => {
      const rail = document.querySelector('#navigation');
      const bar = document.querySelector('#bottom-navigation');
      const items = [...bar.querySelectorAll('a[href]')];
      const current = items.find(item => item.getAttribute('aria-current') === 'page');
      const icons = items.map(item => item.querySelector('.vk-ui-nav-item__indicator :is(.vk-ui-icon, .vk-ui-avatar)')?.getBoundingClientRect());
      const navRect = bar.getBoundingClientRect();
      return { rail: rail.getBoundingClientRect().width, mobile: navRect.height > 0,
        railTargets: [...rail.querySelectorAll('#navigation-primary a:not(.vk-ui-nav-utility)')].map(item => item.getAttribute('href')),
        railAccount: Boolean(rail.querySelector('#navigation-account a[href="/toi"]')) && !rail.querySelector('#profile-trigger, .vk-ui-nav-utility:not(#notification-bell)'), count: items.length, current: current?.getAttribute('href'),
        pill: current?.querySelector('.vk-ui-nav-item__indicator')?.getBoundingClientRect().toJSON(),
        iconDelta: icons.every(Boolean) ? Math.max(...icons.map(r => r.top + r.height / 2)) - Math.min(...icons.map(r => r.top + r.height / 2)) : null,
        axisDelta: icons.every(Boolean) ? Math.max(...icons.map((r, i) => {
          const target = items[i].getBoundingClientRect();
          return Math.abs((r.left + r.right - target.left - target.right) / 2);
        })) : null,
      };
    })(),
    targets: Array.from(document.querySelectorAll('.vk-explore-modes a, #following-cards .vk-ui-byline__identity'))
      .filter(el => el.getClientRects().length > 0).slice(0,4).map(el => [el.getBoundingClientRect().width, el.getBoundingClientRect().height])
  }))()`;
  for (const width of [320, 390, 834, 1024, 1440]) {
    await page.viewport(width, 900);
    for (const theme of ['light', 'dark']) {
      for (const text of [100, 150, 200]) {
        await setFeedAppearance(page, theme, text);
        const got = await page.eval(measure);
        const scaleLabel = text === 200 ? 'stress 200% (root font-size override)' : `app CSS ${text}%`;
        check(got.overflow === 0 && !got.nested && got.gap >= 16 && got.targets.length === 4 &&
          got.targets.every(([w, h]) => w >= 44 && h >= 44),
          `#296 ${width}/${theme}/${scaleLabel} overflow, links and touch targets (${JSON.stringify(got)})`);
        const nav = got.nav;
        check(width < 760
          ? nav.mobile && nav.count === 3 && nav.current === '/' && nav.pill &&
            nav.pill.width <= 64 && nav.pill.height <= 32 && nav.axisDelta <= 1 && nav.iconDelta <= 1
          : !nav.mobile && nav.rail > 0 && nav.railAccount && nav.railTargets.join(',') === '/,/ban-viet',
          `#355 shell destinations and icon alignment ${width}/${theme}/${scaleLabel} (${JSON.stringify(nav)})`);
        if ([320, 390].includes(width) || (text === 100 && [834, 1440].includes(width))) {
          const modeTargets = ['#explore-mode-works', '#explore-mode-following', '#explore-mode-authors'];
          await textContrast(page, theme, 'following-feed', [
            ...modeTargets, '.vk-feed-date',
            '.vk-public-row__title', '.vk-public-row__excerpt', '.vk-public-row__byline .vk-ui-byline__name',
          ]);
          await page.capture(`feed-355-${width}-${theme}-${text}`, {
            requiredTargets: ['#explore-mode-following', '#following-cards', '.vk-feed-date'],
            textScaleLane: text === 200 ? 'dom-override-geometry-only' : 'app-text-scale',
          });
          if (width === 390 && text === 100) {
            await page.eval(`${q('#explore-mode-works')}.focus()`);
            await page.key('Tab');
            await focusEvidence(page, '#explore-mode-following', check, `feed-355-mode-focus-${theme}`, {capture: false});
            await page.eval('document.activeElement.blur()');
          }
        }
      }
    }
  }
  await setFeedAppearance(page, 'light', 100);
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
  check(await guest.eval(`${q('#feed-sign-in')}.classList.contains('vk-ui-button--filled')`),
    '#405 guest Feed sign-in uses a filled primary button like guest Desk');
  check(new URL(target, base).searchParams.get('next') === FEED, 'guest sign-in maintains the Feed return context');
  await guest.click('#feed-sign-in');
  await guest.waitFor("location.pathname==='/toi' && !!document.querySelector('#password-login')", 'CTA reaches real auth');

  await page.goto(base + target);
  await page.waitFor(q('#me-return-link'), 'active native account can resume the guest route');
  await page.click('#me-return-link');
  await page.waitFor(state('no-follows'), 'account with no active follows gets its own empty state');
  await noFollowsLayout(page, check, textContrast);
  await page.eval(`${q(DISCOVER_AUTHORS)}.focus()`);
  await page.key('Space');
  check(await page.eval(`location.search === '${FEED.slice(1)}'`), 'Space does not activate the native link');
  await page.key('Enter');
  const authorsReady = `location.search === '${AUTHORS.slice(1)}' && ${q('#explore-mode-authors')}?.getAttribute('aria-current') === 'page' && ${q('#explore')}?.dataset.search === 'closed'`;
  await page.waitFor(authorsReady, 'Enter opens the real author-search mode');
  await page.eval('history.back()');
  await page.waitFor(`location.search === '${FEED.slice(1)}' && ${q(DISCOVER_AUTHORS)}`,
    'Back restores the empty Follow mode');
  await page.eval('history.forward()');
  await page.waitFor(authorsReady, 'Forward restores the Authors mode');
  await page.click('#open-public-search');
  await fill(page, '#author-search-query', '@' + b.handle);
  await page.key('Enter');
  const authorResult = `.vk-author-search a[href="/@${b.handle}"][data-profile-return]`;
  await page.waitFor(q(authorResult), 'exact Handle finds the author before their first public work');
  await page.click(authorResult);
  await page.waitFor(`${q('#author-profile-heading')} && ${q('#follow-action')}?.textContent.trim() === 'Theo dõi'`,
    'search result reaches the correct profile and current Follow action');
  const followAck = await holdAnswer(page, 'PUT', '/api/v1/authors/' + b.handle + '/relationship/me');
  await page.click('#follow-action');
  check(await followAck.held() === 200, 'Follow from search profile commits before the ACK is lost');
  await followAck.finish(true);
  await page.waitFor(`${q('#follow-action')}?.textContent.trim() === 'Bỏ theo dõi'`,
    'profile reconciles the real Follow receipt');
  await page.eval('history.back()');
  await page.waitFor(`location.search.includes('mode=authors') && ${q('#explore-mode-following')}`,
    'Back preserves author search');
  await page.click('#explore-mode-following');
  await page.waitFor(state('no-works'), 'the newly followed author has no eligible work');
  check(await page.eval(`!${q(DISCOVER_AUTHORS)} && !!${q(HEADING)}`),
    'no-works retains its state and heading without the discovery CTA');
  await page.capture('feed-355-no-works', {requiredTargets: ['[data-feed-state="no-works"]']});

  const bWorks = publish(b.principal_id, 16, 'Feed riêng · tác phẩm đã theo dõi');
  const aWorks = publish(a.principal_id, 1, 'Tác phẩm ngoài Feed');
  await toggle(control, base, b.handle, false);
  await page.waitFor(state('no-follows'), 'Unfollow clears Feed even when the author has public work');
  await toggle(control, base, b.handle, true);
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
  const reposted = bWorks.at(-5);
  authorFixtureSql(`BEGIN;
  CREATE TEMP TABLE withdrawn ON COMMIT DROP AS
    SELECT * FROM publications WHERE work_id=${literal(reposted)};
  DELETE FROM publications WHERE work_id=${literal(reposted)};
  INSERT INTO publications(work_id,revision_id,generation,published_at,metadata_snapshot_id)
  SELECT work_id, revision_id, generation + 1, clock_timestamp() + interval '1 day', metadata_snapshot_id
  FROM withdrawn; COMMIT;`);
  const expected = JSON.parse(authorFixtureSql(`SELECT json_agg(json_build_object(
    'id', f.work_id, 'first', f.first_published_at, 'pointer', p.published_at
  ) ORDER BY f.first_published_at DESC, f.work_id DESC)
    FROM work_first_publications f JOIN publications p USING(work_id)
    WHERE f.provenance='exact' AND f.work_id IN (${bWorks.map(literal).join(',')});`));
  await page.goto(base + FEED);
  await page.waitFor(`${ROWS}===12 && ${q('#feed-load-more')}`, 'direct reload reads the current Feed after republishing');
  const workFacts = await page.eval(`Array.from(document.querySelectorAll('#following-cards > li'), row => ({
    id: row.dataset.workId, date: row.querySelector('time')?.dateTime,
    label: row.querySelector('time')?.textContent, excerpt: row.querySelector('.vk-public-row__excerpt')?.textContent,
    space: getComputedStyle(row.querySelector('.vk-public-row__excerpt')).whiteSpace,
    metadata: row.querySelector('.vk-public-row__eyebrow')?.contains(row.querySelector('time')) &&
      !row.querySelector('time')?.closest('a'),
  }))`);
  check(expected[0].id !== reposted && Date.parse(expected.find(row => row.id === reposted).pointer) > Date.parse(expected[0].first) &&
    JSON.stringify(workFacts.map(row => row.id)) === JSON.stringify(expected.slice(0, 12).map(row => row.id)) &&
    workFacts.every((row, i) => row.metadata && Date.parse(row.date) === Date.parse(expected[i].first) &&
      Date.parse(row.date) !== Date.parse(expected[i].pointer) &&
      row.label === new Intl.DateTimeFormat('vi-VN', {timeZone: 'Asia/Ho_Chi_Minh'}).format(new Date(row.date)) &&
      row.excerpt.includes('\nGió khuya lay giàn hoa.\n\n') && row.space === 'pre-wrap'),
    'PG exact facts drive order and dates after re-Follow and repost; poem lines and stanza are preserved');
  await layout(page, check, textContrast);

  await page.viewport(390, 900);
  const firstTitle = '#following-cards > li:first-child .vk-public-row__link';
  const firstAuthor = '#following-cards > li:first-child .vk-ui-byline__identity';
  await page.eval(`${q('#explore-mode-works')}.focus()`);
  for (const selector of ['#explore-mode-following', '#explore-mode-authors', firstTitle, firstAuthor]) {
    await page.key('Tab');
    check(await page.eval(`document.activeElement === ${q(selector)}`), `Tab reaches ${selector} once`);
  }
  await focusEvidence(page, firstAuthor, check, 'feed-355-byline-focus');
  await page.key('Tab', 8); // CDP Shift bit.
  await focusEvidence(page, firstTitle, check, 'feed-355-title-focus');

  await page.send('Emulation.setEmulatedMedia', {features: [{name:'prefers-reduced-motion', value:'reduce'}]});
  const failed = await holdAnswer(page, 'GET', API);
  await page.eval(`${q('#feed-load-more')}.focus()`);
  await page.key('Space');
  check(await failed.held() === 200, 'Space submits append to the real provider');
  check(await page.eval(`matchMedia('(prefers-reduced-motion: reduce)').matches &&
    ${q('#feed-load-more')}.getAttribute('aria-busy') === 'true' &&
    ${q('#feed-load-more-note')}?.textContent.includes('Đang tải thêm')`), 'append pending remains expressed in text with reduced motion');
  await failed.finish(true);
  await page.send('Emulation.setEmulatedMedia', {features: []});
  await page.waitFor(`${state('append-error')} && ${ROWS}===12`, 'append can fail while prior cards remain');
  await page.send('Emulation.setSafeAreaInsetsOverride', {insets: {bottom: 24, left: 20, right: 20}});
  await page.eval('scrollTo(0, document.documentElement.scrollHeight)');
  await page.eval(FRAME);
  check(await page.eval(`(() => {
    const status = ${state('append-error')}.getBoundingClientRect();
    const bar = ${q('#bottom-navigation')};
    return status.bottom <= bar.getBoundingClientRect().top &&
      parseFloat(getComputedStyle(bar).paddingBottom) >= 24;
  })()`), 'emulated safe area leaves the append error and retry above the bottom bar');
  await focusEvidence(page, '#feed-load-more', check, 'feed-355-append-retry-focus', {capture: false});
  await page.capture('feed-355-append-error', {requiredTargets: ['#feed-load-more', '[data-feed-state="append-error"]']});
  await page.send('Emulation.setSafeAreaInsetsOverride', {insets: {bottom: 0, left: 0, right: 0}});
  await page.viewport(320, 900);
  await page.eval("document.documentElement.style.fontSize = '200%'; document.fonts.ready");

  const removed = await page.eval("document.querySelector('#following-cards > li').dataset.workId");
  withdraw(removed);
  await page.click('#feed-load-more');
  await page.waitFor(`${ROWS}===15 && !${q(rowLink(removed))}`, 'next page revalidates previously displayed rows');
  const anchor = await page.eval("Array.from(document.querySelectorAll('#following-cards > li')).at(-2).dataset.workId");
  const selector = rowLink(anchor);
  const sameDocument = await page.eval('performance.timeOrigin');
  const otherAuthor = randomUUID();
  const otherHandle = 'vink-feed-race-author';
  authorFixtureSql(`BEGIN;
    INSERT INTO principals(id, display_name) VALUES (${literal(otherAuthor)}, 'Tác giả đổi Follow');
    INSERT INTO principal_bindings(principal_id, authority, subject_id)
      VALUES (${literal(otherAuthor)}, 'vink-identity', ${literal(otherAuthor)});
    INSERT INTO author_handle_names(principal_id, handle) VALUES (${literal(otherAuthor)}, '${otherHandle}');
    INSERT INTO author_profiles(principal_id, current_handle) VALUES (${literal(otherAuthor)}, '${otherHandle}');
    COMMIT;`);
  const enter = async () => {
    await page.eval(`${q(selector)}.focus()`);
    await page.key('Enter');
    await page.waitFor(`location.pathname === '/tac-pham/${anchor}' && ${q('#reader-paper')}`, 'keyboard WorkCard opens real Reader');
  };
  await enter();
  check(await page.eval(`${q('#reader-return')}.getAttribute('href') === '${FEED}#work-${anchor}'`),
    'Reader return link keeps the mode and anchor, with no private query cursor');
  const initialError = await holdAnswer(page, 'GET', API);
  await page.eval('history.back()');
  check(await initialError.held() === 200, 'Back requests fresh authorization from the real provider');
  await page.waitFor(`${state('loading')} && ${ROWS}===0 && document.activeElement?.id !== 'following-feed-heading'`,
    'Back is pending, with zero cached rows and no fallback focus');
  await initialError.finish(true);
  await page.waitFor(`${state('error')} && ${ROWS}===0 && document.activeElement?.id !== 'following-feed-heading'`,
    'a transport error does not decide that the anchor is gone');
  await page.eval(`${q('#feed-restart')}.focus()`);
  await page.key('Enter');
  await page.waitFor(`location.search === '?mode=following' && document.activeElement === ${q(selector)}`, 'retry validates and focuses the row from the appended page');
  await focusEvidence(page, selector, check, 'feed-355-back-title');
  check(await page.eval(`performance.timeOrigin === ${sameDocument}`), 'Feed → Reader → Back uses the hydrated document');
  await enter();
  // Hold the frame queued for the already painted anchor, then commit an unrelated Follow
  // while the replacement read is still on the wire.
  await page.eval(`(() => {
    const requestFrame = window.requestAnimationFrame;
    const queued = [];
    let runningFirst = false;
    window.fixtureFeedFrames = queued;
    window.requestAnimationFrame = callback => {
      if (location.search !== '?mode=following' || !${q(selector)}) {
        return requestFrame.call(window, callback);
      }
      if (runningFirst) return requestFrame.call(window, time => queued.push(() => callback(time)));
      return requestFrame.call(window, time => {
        runningFirst = true;
        try { callback(time); } finally { runningFirst = false; }
      });
    };
    window.releaseFixtureFeedFrames = () => {
      window.requestAnimationFrame = requestFrame;
      for (const callback of queued.splice(0)) callback();
    };
  })()`);
  await page.eval('history.back()');
  await page.waitFor(`window.fixtureFeedFrames.length > 0 && ${q(selector)}`,
    'accepted around read painted the appended anchor before the held focus frame');
  const restart = await holdAnswer(page, 'GET', API);
  await toggle(control, base, otherHandle, true);
  check(await restart.held() === 200, 'another author’s confirmed Follow invalidates the pending focus request');
  await page.send('Page.bringToFront');
  await page.waitFor(`${state('loading')} && ${ROWS}===0`, 'replacement read owns the empty pending stream');
  await page.eval('window.releaseFixtureFeedFrames()');
  await page.eval(FRAME);
  check(await page.eval(`document.activeElement !== ${q(HEADING)}`), 'old focus frame cannot focus fallback during a replacement read');
  await restart.finish(false);
  await page.waitFor(`document.activeElement === ${q(selector)}`,
    'replacement read revalidates and focuses the same appended work');
  await toggle(control, base, otherHandle, false);
  await page.waitFor(`${ROWS}===12 && ${q('#feed-load-more')}`, 'completed restore leaves a normal fresh head after Follow changes');
  await page.click('#feed-load-more');
  await page.waitFor(q(selector), 'reopen the target after the regular append');
  await enter();
  withdraw(anchor);
  await page.eval('history.back()');
  await page.waitFor(`${ROWS}===12 && !${q(selector)} && document.activeElement === ${q(HEADING)}`,
    'Back to withdrawn anchor focuses the Feed heading after a successful live read');
  await focusEvidence(page, HEADING, check, 'feed-355-back-withdrawn');
  await page.eval("document.documentElement.style.fontSize = ''");
  await page.viewport(1280, 900);
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

  // The last eligible work has a long poem. Give its author a long pen name and
  // require the actual link to be in view above the bottom bar at large mobile text.
  const deep = bWorks[0];
  const deepTitle = rowLink(deep);
  const deepByline = `#work-${deep} .vk-ui-byline__identity`;
  authorFixtureSql(`UPDATE principals SET display_name='Nguyễn Thị Phương Vy Bên Hiên Nhà Mùa Mưa'
    WHERE id=${literal(b.principal_id)};`);
  await page.viewport(320, 720);
  await page.eval("document.documentElement.style.fontSize = '200%'; document.fonts.ready");
  await page.click('#feed-load-more');
  await page.waitFor(q(deepByline), 'the tall poem and byline are on the appended page');
  check(await page.eval(`(() => {
    const byline = ${q(deepByline)};
    const row = byline.closest('li');
    const bar = document.getElementById('bottom-navigation');
    const start = parseFloat(getComputedStyle(row).scrollMarginBlockStart);
    return byline.querySelector('.vk-ui-byline__name').textContent.includes('Bên Hiên Nhà Mùa Mưa') &&
      byline.getBoundingClientRect().bottom - row.getBoundingClientRect().top + start >
      bar.getBoundingClientRect().top;
  })()`), 'the tall fixture would put the byline below the bar if Back scrolled only to the row');
  await page.eval(`${q(deepByline)}.focus()`);
  await page.key('Enter');
  await page.waitFor(`location.pathname === '/@${b.handle}' && ${q('#author-profile-heading')}`,
    'keyboard enters the tall row author profile');
  await page.eval('history.back()');
  await page.waitFor(`document.activeElement === ${q(deepByline)}`, 'Back focuses the exact tall-row byline');
  await focusEvidence(page, deepByline, check, 'feed-355-back-byline-320-200');

  // Real document traversal matching #293: a second Back before the intermediate
  // Profile document hydrates must not start the router against the wrong SSR DOM.
  const originalDocument = await page.eval('performance.timeOrigin');
  await page.click(deepByline);
  await page.waitFor(`location.pathname === '/@${b.handle}' && ${q('#author-profile-heading')}`, 'profile ready before new document');
  await page.eval(`window.addEventListener('pageshow', event => {
    if (event.isTrusted && event.persisted) sessionStorage.setItem('fixture-feed-bfcache', 'hit');
  });`);
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
  await page.waitFor(`performance.timeOrigin !== ${originalDocument} &&
    document.documentElement.dataset.hydrated==='true' && !${q('#author-profile-heading')} &&
    document.activeElement === ${q(deepByline)}`,
    'new Feed document revalidates and focuses the exact byline from the appended page');

  console.log('  #355 natural persisted pageshow:', await page.eval("sessionStorage.getItem('fixture-feed-bfcache') === 'hit'"));

  await page.click(deepTitle);
  await page.waitFor(`location.pathname === '/tac-pham/${deep}' && ${q('#reader-paper')}`, 'open Reader from the deep Feed entry');
  const readerHistory = await page.send('Page.getNavigationHistory');
  const deepFeedEntry = readerHistory.entries[readerHistory.currentIndex - 1];
  check(new URL(deepFeedEntry.url).hash === '#work-' + deep, 'the original Feed entry owns its work hint');
  const historyDocument = await page.eval('performance.timeOrigin');
  await page.click('#bottom-navigation a[href="/"]');
  await page.waitFor(`location.pathname === '/' && !location.search && ${q('#explore-mode-following')}`,
    'Khám phá opens from Reader in the shared shell');
  await page.click('#explore-mode-following');
  await page.waitFor(`${ROWS}===12 && ${q(workLink)}`, 'an independent Feed entry starts at the first page');
  const newest = await page.eval("document.querySelector('#following-cards > li').dataset.workId");
  await page.click(workLink);
  await page.waitFor(`location.pathname === '/tac-pham/${newest}' && ${q('#reader-paper')}`,
    'a different Feed entry opens another Reader');
  await page.send('Page.navigateToHistoryEntry', {entryId: deepFeedEntry.id});
  await page.waitFor(`document.activeElement === ${q(deepTitle)}`,
    'traversal to the original entry restores its own appended title, not the newer entry’s work');
  check(await page.eval(`performance.timeOrigin === ${historyDocument}`),
    'both independent Feed entries and Readers live in the same hydrated document');

  await page.click(deepTitle);
  await page.waitFor(q('#reader-paper'), 'a currently public work stays open during Unfollow');
  await toggle(control, base, b.handle, false);
  await page.send('Page.bringToFront');
  await page.eval('history.back()');
  await page.waitFor(`${state('no-follows')} && ${ROWS}===0 && document.activeElement === ${q(EMPTY_HEADING)}`,
    'Back after Unfollow focuses the visible empty heading even though Reader is still public');
  await focusEvidence(page, EMPTY_HEADING, check, 'feed-355-r1-back-after-unfollow');
  await page.key('Tab');
  await focusEvidence(page, DISCOVER_AUTHORS, check, 'feed-355-r1-tab-after-unfollow', {capture: false});
  await toggle(control, base, b.handle, true);
  await page.waitFor(`${ROWS}===12 && ${q('#feed-load-more')}`, 'Follow restores eligible works after return');

  const publicReads = captureRequest(page, 'GET', '/api/v1/works');
  const append = await holdAnswer(page, 'GET', API);
  await page.click('#feed-load-more');
  check(await append.held() === 200, 'hold real append before the Graph ACK');
  await toggle(control, base, b.handle, false);
  await page.waitFor(`${state('no-follows')} && ${ROWS}===0`, 'confirmed Unfollow fences the open chain');
  const modeFocusId = 'explore-mode-works';
  await page.eval(`document.getElementById(${JSON.stringify(modeFocusId)}).focus()`);
  await append.finish(false, true);
  check(await page.eval(`${ROWS}===0 && document.activeElement?.id===${JSON.stringify(modeFocusId)}`),
    'old append cannot restore rows or move focus after Unfollow');

  await toggle(control, base, b.handle, true);
  await page.waitFor(`${ROWS}===12 && ${q('#feed-load-more')}`, 'new Follow restarts from the current head');
  // One real cookie transition invalidates both pending reads. A separate Feed tab
  // owns the append while the original tab is traversing Back to its history entry.
  const appendTab = await openPage();
  await appendTab.goto(base + FEED);
  await appendTab.waitFor(`${ROWS}===12 && ${q('#feed-load-more')}`, 'second tab has A’s first page');
  const oldSession = await holdAnswer(appendTab, 'GET', API);
  await appendTab.click('#feed-load-more');
  check(await oldSession.held() === 200, 'A append is held before the native session changes');

  await page.send('Page.bringToFront');
  await page.click('#feed-load-more');
  await page.waitFor(q(deepTitle), 'append the old work before opening Reader');
  await page.click(deepTitle);
  await page.waitFor(q('#reader-paper'), 'open the appended work before changing authority');
  const previousRestore = await holdAnswer(page, 'GET', API);
  await page.eval('history.back()');
  check(await previousRestore.held() === 200, 'old A around response is held during Back');
  await switchActor('b', control);
  await Promise.all([page, appendTab].map(tab => tab.waitFor(`${state('no-follows')} && ${ROWS}===0`,
    'B never renders A’s anchor or page chain')));
  await switchActor('a', control);
  await Promise.all([page, appendTab].map(tab => tab.waitFor(`${ROWS}===12 && !${q(deepTitle)}`,
    'new A starts at the first page without the old session’s around position')));
  await appendTab.send('Page.bringToFront');
  await oldSession.finish(false, true);
  check(await appendTab.eval(`${ROWS}===12 && location.search==='?mode=following'`),
    'stale A completion cannot append in the new A session');
  await page.send('Page.bringToFront');
  await previousRestore.finish(false, true);
  check(await page.eval(`!${q(deepTitle)} && document.activeElement !== ${q(deepTitle)}`),
    'late focus and rows from the old A restore stay fenced after A → B → A');
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
  const errors = [...page.errors, ...appendTab.errors, ...guest.errors];
  check(errors.length === 0, `Feed tabs have no console or hydration errors: ${errors.join(' | ')}`);
  await control.cdp.send('Target.disposeBrowserContext', {browserContextId});
  await control.send('Page.bringToFront');
  await control.goto(base + '/toi');
}
