// #292/#293. Real Chrome, password sessions, App and scratch PostgreSQL.
// The works/comments and retirement are controlled fixtures, not account deletion.
import { createHash, randomUUID } from 'node:crypto';
import { authorFixtureSql } from './author-fixtures.mjs';
import { fill, q } from './auth-smoke.mjs';

const API = '/api/v1/me/profile';
const EDIT = '/toi/chinh-ho-so';
const NAME = '#profile-pen-name';
const BIO = '#profile-bio';
const SAVE = '#profile-save';
const SAME_NAME = 'Một bút danh tiếng Việt rất dài dùng chung trên tác phẩm và nhận xét';
const own = (page) => page.eval(`fetch('${API}').then(r => r.json())`);
const message = (s) => `${q('#profile-message')}?.textContent.includes(${JSON.stringify(s)})`;
const selector = (id) => '#' + id;

async function edit(page, base) {
  await page.goto(base + EDIT);
  await page.waitFor(q(NAME), 'profile fields for the confirmed account');
}

async function save(page) {
  await page.click(SAVE);
  await page.waitFor(message('Đã lưu hồ sơ'), 'profile save acknowledged');
}

function literal(text) {
  return "'" + text.replaceAll("'", "''") + "'";
}

function fixture(a, b) {
  const ids = {
    a: randomUUID(), b: randomUUID(), root: randomUUID(),
    reply: randomUUID(), tail: randomUUID(), thread: randomUUID(), tag: randomUUID()
  };
  const works = [a, b].map((actor, i) => {
    const id = i ? ids.b : ids.a;
    const rev = randomUUID();
    const snapshot = randomUUID();
    const title = i ? 'Tác phẩm thứ hai của riêng người viết B' : 'Tác phẩm thứ nhất của riêng người viết A';
    const source = 'Mưa bên bậc cửa rồi lặng.';
    const digest = createHash('sha256').update('vink-document-content');
    for (const text of ['default', title, source]) {
      const bytes = Buffer.from(text);
      const length = Buffer.alloc(8);
      length.writeBigUInt64BE(BigInt(bytes.length));
      digest.update(length).update(bytes);
    }
    return `
      INSERT INTO works(id,owner_id) VALUES (${literal(id)}, ${literal(actor)});
      INSERT INTO revisions(id,work_id,title,document,content_hash,created_by)
      VALUES (${literal(rev)},${literal(id)},${literal(title)},
        ${literal(JSON.stringify({format:'default', source}))},${literal(digest.digest('hex'))},${literal(actor)});
      INSERT INTO work_metadata_snapshots(id, work_id, metadata_version, digest)
      VALUES (${literal(snapshot)},${literal(id)},1,work_metadata_digest(${literal(ids.tag)}, ARRAY[]::uuid[]));
      INSERT INTO work_metadata_snapshot_tags(snapshot_id, slot, tag_id, facet)
      VALUES (${literal(snapshot)},0,${literal(ids.tag)},'form');
      INSERT INTO publications(work_id,revision_id,generation,published_at,metadata_snapshot_id)
      VALUES (${literal(id)},${literal(rev)},1,clock_timestamp(),${literal(snapshot)});`;
  });
  authorFixtureSql(`BEGIN;
    INSERT INTO tags(id,facet,label,slug,status) VALUES (${literal(ids.tag)},'form','Tản văn','tản-văn','active');
    INSERT INTO tag_names(slug,tag_id,kind) VALUES ('tản-văn',${literal(ids.tag)},'canonical');
    ${works.join('\n')}
    INSERT INTO discussion_threads(id,work_id) VALUES (${literal(ids.thread)},${literal(ids.a)});
    INSERT INTO comments(id,thread_id,kind,root_id,root_kind,reply_to_id,replied_to_reply,author_id,body,review,moderation_version,admission_policy,admission_reason,created_at)
    VALUES
      (${literal(ids.root)},${literal(ids.thread)},'root',NULL,NULL,NULL,false,${literal(a)},'Nhận xét tại gốc','pending',1,'premoderation.v1','policy_premoderation',now()),
      (${literal(ids.reply)},${literal(ids.thread)},'reply',${literal(ids.root)},'root',NULL,false,${literal(b)},'Lời đáp thứ nhất','pending',1,'premoderation.v1','policy_premoderation',now() + interval '1 second'),
      (${literal(ids.tail)},${literal(ids.thread)},'reply',${literal(ids.root)},'root',${literal(ids.reply)},true,${literal(a)},'Gửi lại người vừa trả lời','pending',1,'premoderation.v1','policy_premoderation',now() + interval '2 seconds');
    UPDATE comments SET review='approved', moderation_version=2 WHERE thread_id=${literal(ids.thread)};
    COMMIT;`);
  return ids;
}

async function captureLayouts(page, check, name, target) {
  for (const width of [320, 390, 834, 1024, 1440]) {
    for (const theme of ['light', 'dark']) {
      for (const scale of [100, 200]) {
        await page.viewport(width);
        const facts = await page.eval(`(async () => {
          const root = document.documentElement;
          root.dataset.theme = '${theme}';
          root.style.fontSize = '${scale}%';
          await document.fonts.ready;
          await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
          const targets = [...document.querySelectorAll(${JSON.stringify(target)})];
          return {
            width: root.clientWidth, scroll: root.scrollWidth, targets: targets.length,
            inside: targets.every(el => {
              const rect = el.getBoundingClientRect();
              return rect.width > 0 && rect.left >= -1 && rect.right <= root.clientWidth+1;
            })
          };
        })()`);
        check(facts.targets > 0 && facts.inside && facts.scroll <= facts.width + 1,
          `#292/#293 ${name} ${width}px ${theme} ${scale}% ${JSON.stringify(facts)}`);
        if (width === 320 && theme === 'dark' && scale === 200 ||
            width === 1440 && theme === 'light' && scale === 100) {
          await page.eval(`${q(target)}.scrollIntoView({block:'start'})`);
          await page.capture(`author-${name}-${width}-${theme}-${scale}`,
            {boundary:'Chrome + App + native Identity + scratch PG', scenario:name, textScale:scale});
        }
      }
    }
  }
  await page.eval("document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme");
  await page.viewport(1280);
}

// A synthetic persisted event exercises the live-document branch even on a Chrome
// configuration that chooses a fresh document for the real Back traversal below.
async function restoredAttribution(page, check, holdAnswer, { url, endpoint, links, labels, content }) {
  await page.goto(url);
  await page.send('Page.bringToFront');
  const targets = Object.entries(links);
  const linked = targets.map(([selector, href]) =>
    `${q(selector)}?.getAttribute('href')===${JSON.stringify(href)}`).join(' && ');
  await page.waitFor(`document.documentElement.dataset.hydrated==='true' &&
    document.visibilityState==='visible' && ${linked}`, 'fresh linked document before history event');
  await page.eval(`(async () => {
    window.dispatchEvent(new PageTransitionEvent('pageshow'));
    await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
  })()`);
  check(await page.eval(linked), 'ordinary pageshow does not suppress fresh attribution');

  const response = await holdAnswer(page, 'GET', endpoint);
  await page.eval("window.dispatchEvent(new PageTransitionEvent('pageshow', {persisted:true}))");
  check(await response.held() === 200, `persisted pageshow immediately reads ${endpoint}`);
  try {
    const fenced = targets.map(([selector]) => `!${q(selector + '[href]')}`);
    const safe = labels.map(selector =>
      `${q(selector)}?.textContent.includes('Chưa xác nhận được tác giả') &&
        !${q(selector)}.textContent.includes(${JSON.stringify(SAME_NAME)})`);
    await page.waitFor(`${q(content)} && ${[...fenced, ...safe].join(' && ')}`,
      'stale names and links are absent while the current projection is pending');
  } finally {
    await response.finish();
  }
  await page.waitFor(linked, 'the current projection restores the correct links');
}

export async function authorProfiles(page, other, {base, openPage, check, holdAnswer, switchActor, textContrast}) {
  console.log('Author Profile and bylines (#292/#293):');
  await page.goto(base + '/toi');
  await page.waitFor(q('#edit-profile-link'), 'Me shows the profile entry');
  await page.eval(`${q('#edit-profile-link')}.focus()`);
  await page.key('Enter');
  await page.waitFor(q(NAME), 'keyboard opens edit from Me');
  const a = await own(page);
  const bio = 'Viết bằng mực tím.\n\nMột đoạn nữa.';
  await fill(page, NAME, '   ');
  await page.click(SAVE);
  await page.waitFor(`${q(NAME)}.getAttribute('aria-invalid')==='true' &&
    document.activeElement===${q(NAME)}`, 'invalid pen name returns focus to its field');
  check(await page.eval(`${q(NAME)}.value==='   '`) && (await own(page)).profile_version === a.profile_version,
    'validation preserves the typed name and leaves durable state unchanged');
  await fill(page, NAME, SAME_NAME);
  await fill(page, BIO, bio);
  await page.eval(`(() => {
    const input = ${q(BIO)};
    input.focus();
    input.dispatchEvent(new CompositionEvent('compositionstart', {bubbles:true}));
    input.form.requestSubmit();
  })()`);
  check((await own(page)).profile_version === a.profile_version, 'composition event prevents a partial profile save');
  await page.eval(`${q(BIO)}.dispatchEvent(new CompositionEvent('compositionend', {bubbles:true}))`);
  const lost = await holdAnswer(page, 'PUT', API);
  await page.click(SAVE);
  check(await lost.held() === 200, 'profile save reached durable commit before the lost answer');
  await lost.finish(true);
  await page.waitFor(message('khớp với thay đổi'), 'lost profile ACK reconciled from persisted state');
  await page.reload();
  await page.waitFor(`${q(BIO)}?.value === ${JSON.stringify(bio)}`, 'Bio survives reload');
  check(await page.eval(`!!document.querySelector('a[aria-current="page"][href="/toi"]')`),
    'Tôi is active in the profile editor');
  await captureLayouts(page, check, 'edit', '#profile-pen-name, #profile-bio, #profile-save, .vk-profile-form select');
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme='${theme}'`);
    await page.waitFor(`!document.getAnimations().some(a => a.playState==='running' &&
      a.effect.target?.checkVisibility() && Number.isFinite(a.effect.getComputedTiming().endTime))`,
      'profile colors settled');
    await textContrast(page, theme, 'profile', ['#profile-save', '#profile-bio', '#profile-pen-name-help']);
  }
  await page.eval("delete document.documentElement.dataset.theme");
  await page.viewport(320, 580);
  await page.eval("document.documentElement.style.fontSize='200%'");
  await page.eval(`${q('.vk-profile-form a[href="/toi/dia-chi"]')}.focus()`);
  await page.key('Tab');
  await page.waitFor(`(() => {
    const save=${q(SAVE)}, nav=${q('.vk-ui-bottom-nav')};
    const r=save.getBoundingClientRect();
    return document.activeElement === save && r.bottom <= nav.getBoundingClientRect().top+1 &&
      save.contains(document.elementFromPoint(r.x + r.width/2, r.y + r.height/2));
  })()`, 'Tab reaches unobscured Save at 320px/200% with a short viewport');
  await page.capture('author-edit-short-viewport', {scenario:'Save focused', textScale:200});
  await page.viewport(1280);
  await page.eval("document.documentElement.style.fontSize=''");
  await fill(page, BIO, 'Nháp trước khi về Tôi');
  await page.click('#cancel-profile-edit');
  await page.waitFor("document.activeElement?.id==='edit-profile-link'", 'cancel returns focus to Me entry');
  await page.key('Enter');
  await page.waitFor(`${q(BIO)}?.value==='Nháp trước khi về Tôi'`, 'cancel preserves draft in the same session');
  await fill(page, BIO, bio);

  await edit(other, base);
  const b = await own(other);
  await fill(other, NAME, SAME_NAME);
  await save(other);
  const ids = fixture(a.principal_id, b.principal_id);
  const aPath = '/@' + a.handle;
  const bPath = '/@' + b.handle;
  const profileApi = '/api/v1/authors/' + a.handle;
  const readerPath = '/tac-pham/' + ids.a;
  const context = await page.cdp.send('Target.createBrowserContext', {disposeOnDetach:false});
  const guest = await openPage(context.browserContextId);
  await guest.goto(base + aPath);
  await guest.waitFor(q('.vk-author-bio'), 'guest receives the public Bio');
  check(await guest.eval(`${q('.vk-author-bio')}.textContent===${JSON.stringify(bio)}
    && ${q(selector('profile-work-' + ids.a))} && !${q(selector('profile-work-' + ids.b))}
    && !${q('#own-profile-edit-link')}`), 'public profile lists the correct principal and no owner action');
  await guest.waitFor("document.documentElement.dataset.hydrated==='true'", 'public profile hydrated');
  await guest.capture('author-public-desktop', {scenario:'guest with Bio and works', textScale:100});
  await captureLayouts(guest, check, 'public', '#author-profile-heading, .vk-author-bio, #author-works-heading, .vk-author-works a');
  await guest.eval(`window.addEventListener('pageshow', event => {
    if (event.persisted) sessionStorage.setItem('fixture-public-bfcache', 'restored');
  });`);
  await guest.goto(base + '/toi');
  const staleTab = await openPage();
  await edit(staleTab, base);
  await fill(staleTab, BIO, 'Bản nháp còn ở tab kia');
  const viewing = await openPage();
  await viewing.goto(base + aPath);
  await viewing.send('Page.bringToFront');
  await viewing.waitFor(`document.documentElement.dataset.hydrated==='true' &&
    document.visibilityState==='visible' && ${q('.vk-author-bio')} && ${q('#own-profile-edit-link')}`,
    'hydrated public profile and own entry in a foreground tab');
  // Hold complete bytes of the old public response. PostgreSQL/HTTP checks the 304 path.
  await viewing.send('Network.setCacheDisabled', {cacheDisabled:true});
  const delayed = await holdAnswer(viewing, 'GET', profileApi);
  await viewing.eval("window.dispatchEvent(new FocusEvent('focus'))");
  check(await delayed.held() === 200, 'hold a real public response before policy changes');
  await page.eval(`(() => {
    const select = ${q('#profile-bio-policy')};
    select.value = 'self'; select.dispatchEvent(new Event('change', {bubbles:true}));
  })()`);
  await save(page);
  await viewing.waitFor(`${q('#author-profile-heading')} && !${q('.vk-author-bio')}`, 'broadcast invalidates the previous public view');
  await viewing.eval(`window.revokedBioShown = false;
    new MutationObserver(records => {
      if (records.some(record => [...record.addedNodes].some(node => node.nodeType===1 &&
          (node.matches('.vk-author-bio') || node.querySelector('.vk-author-bio'))))) window.revokedBioShown = true;
    }).observe(document.getElementById('noi-dung'), {childList:true, subtree:true});
  `);
  const publicDisposition = await delayed.finish(false, true);
  check(await viewing.eval(`${q('#author-profile-heading')} && !${q('.vk-author-bio')} && !window.revokedBioShown`),
    `late public answer cannot restore a revoked Bio (${publicDisposition})`);
  await guest.send('Page.bringToFront');
  await guest.eval('history.back()');
  await guest.waitFor(`location.pathname===${JSON.stringify(aPath)} && ${q('#author-profile-heading')} &&
    !${q('.vk-author-bio')} && !${q('#noi-dung')}.inert`, 'native Back does not restore the revoked Bio');
  console.log('  profile revoke Back used BFCache:', await guest.eval("sessionStorage.getItem('fixture-public-bfcache')==='restored'"));
  await guest.eval("window.dispatchEvent(new FocusEvent('focus'))");
  await guest.waitFor(`${q('#author-profile-heading')} && !${q('.vk-author-bio')}`, 'isolated guest revalidates upon focus');
  check(authorFixtureSql('UPDATE author_handle_rollout SET ready=false WHERE singleton AND ready RETURNING 1;') === '1',
    'closed the profile read gate in this scratch database only');
  try {
    await guest.reload();
    await guest.waitFor(`${q('#author-profile-heading')}?.textContent==='Chưa tải được hồ sơ' &&
      ${q('.vk-author-public button')}`, 'unavailable public profile offers retry');
    await guest.capture('author-public-retry', {scenario:'real read gate unavailable', textScale:100});
  } finally {
    check(authorFixtureSql('UPDATE author_handle_rollout SET ready=true WHERE singleton AND NOT ready RETURNING 1;') === '1',
      'reopened the scratch profile read gate');
  }
  await guest.click('.vk-author-public button');
  await guest.waitFor(`${q('#author-profile-heading')}?.textContent===${JSON.stringify(SAME_NAME)} &&
    ${q(selector('profile-work-' + ids.a))} && !${q('.vk-author-bio')}`, 'retry loads the current profile and works');
  await staleTab.click(SAVE);
  await staleTab.waitFor(q('#profile-conflict-heading'), 'stale form displays the current saved version');
  check(await staleTab.eval(`${q(BIO)}.value==='Bản nháp còn ở tab kia'`), 'CAS conflict preserves typed draft');
  check(await staleTab.eval(`${q(SAVE)}.dataset.state==='disabled' &&
    ${q(SAVE)}.getAttribute('aria-disabled')==='true'`), 'Save is visibly and semantically disabled during conflict');
  await staleTab.eval(`${q('.vk-profile-conflict')}.scrollIntoView({block:'center'})`);
  await staleTab.capture('author-profile-conflict', {scenario:'real CAS conflict', textScale:100});
  await staleTab.eval("document.querySelector('.vk-profile-conflict button').click()");
  await staleTab.waitFor(`document.activeElement?.id==='profile-pen-name' && ${q(BIO)}.value===${JSON.stringify(bio)}`,
    'choosing the saved version restores focus and value');

  // A held private GET can arrive after logout, login as B, and a new login as A.
  const lateOwn = await holdAnswer(staleTab, 'GET', API);
  await staleTab.goto(base + EDIT);
  check(await lateOwn.held() === 200, 'hold A’s real own profile');
  await switchActor('b');
  await staleTab.waitFor(`${q(BIO)} && ${q(BIO)}.value===''`, 'B has its own profile state');
  await switchActor('a');
  await staleTab.waitFor(`${q(BIO)}?.value===${JSON.stringify(bio)}`, 'A returns with a new generation');
  await fill(staleTab, BIO, 'Nháp của phiên mới');
  const ownDisposition = await lateOwn.finish(false, true);
  check(await staleTab.eval(`${q(BIO)}.value==='Nháp của phiên mới'`),
    `old A response cannot overwrite the new A form (${ownDisposition})`);

  const discovery = '/?q=rieng&tag=' + encodeURIComponent('tản-văn');
  await guest.goto(base + discovery);
  const cardA = selector('work-' + ids.a + '-author');
  const cardB = selector('work-' + ids.b + '-author');
  const rowA = selector('work-' + ids.a);
  const titleA = rowA + ' .vk-public-row__link';
  await guest.waitFor(`document.documentElement.dataset.hydrated==='true' && ${q(cardA)}`, 'linked bylines in hydrated discovery');
  check(await guest.eval(`!document.querySelector('a a') &&
    ${q(cardA)}.getAttribute('href')===${JSON.stringify(aPath)} && ${q(cardB)}.getAttribute('href')===${JSON.stringify(bPath)}`),
    'two cards with one pen name have distinct sibling destinations');
  await captureLayouts(guest, check, 'byline', rowA + ' a');
  await guest.viewport(320);
  await guest.eval("document.documentElement.style.fontSize='200%'");
  await guest.eval(`${q(titleA)}.focus()`);
  await guest.key('Tab');
  check(await guest.eval(`document.activeElement===${q(cardA)}`), 'Tab goes from the work to its author');
  await guest.key('Tab', 8); // CDP Shift modifier.
  check(await guest.eval(`document.activeElement===${q(titleA)}`), 'Shift+Tab returns to the same work title');
  await guest.key('Enter');
  await guest.waitFor(`location.pathname===${JSON.stringify(readerPath)} && ${q('#reader-paper')}`, 'the sibling work link opens the Reader');
  await guest.eval('history.back()');
  await guest.waitFor(`document.activeElement===${q(titleA)}`, 'Reader Back restores the same work title');
  await guest.key('Tab');
  await guest.key('Enter');
  await guest.waitFor(`location.pathname===${JSON.stringify(aPath)} && ${q('#author-profile-heading')}`, 'Enter opens the right profile');
  await guest.eval('history.back()');
  await guest.waitFor(`document.activeElement===${q(cardA)}`, 'Back restores the validated card link');
  check(await guest.eval(`(() => {
    const query = new URLSearchParams(location.search);
    return location.pathname==='/' && query.get('q')==='rieng' && query.getAll('tag').join()==='tản-văn';
  })()`), 'Back preserves query and tag scope');
  // #352: avatar, pen name and @handle are one shared AuthorByline target.
  for (const part of ['vk-ui-avatar', 'vk-ui-byline__name', 'vk-ui-byline__handle']) {
    await guest.click(cardA + ' .' + part);
    await guest.waitFor(`location.pathname===${JSON.stringify(aPath)} && ${q('#author-profile-heading')}`,
      'clicking the ' + part + ' opens the same author profile');
    await guest.eval('history.back()');
    await guest.waitFor(`document.activeElement===${q(cardA)}`, 'Back from the ' + part + ' restores the byline');
  }
  await guest.send('Emulation.setTouchEmulationEnabled', {enabled:true, maxTouchPoints:1});
  const point = await guest.eval(`(() => {
    const el=${q(cardB + ' .vk-ui-byline__handle')};
    el.scrollIntoView({block:'center'});
    const rect=el.getBoundingClientRect();
    return {x:rect.x+rect.width/2, y:rect.y+rect.height/2};
  })()`);
  await guest.send('Input.dispatchTouchEvent', {type:'touchStart', touchPoints:[point]});
  await guest.send('Input.dispatchTouchEvent', {type:'touchEnd', touchPoints:[]});
  await guest.waitFor(`location.pathname===${JSON.stringify(bPath)} && ${q('#author-profile-heading')}`, 'touch opens the other author');
  await guest.send('Emulation.setTouchEmulationEnabled', {enabled:false});
  await guest.viewport(1280);

  await guest.goto(base + readerPath);
  await guest.waitFor("document.documentElement.dataset.hydrated==='true'", 'Reader navigation listeners are hydrated');
  const bylines = [
    ['#reader-author', aPath],
    [selector('comment-' + ids.root + '-author'), aPath],
    [selector('comment-' + ids.reply + '-author'), bPath],
    [selector('comment-' + ids.tail + '-context-author'), bPath],
  ];
  for (const [link, target] of bylines) {
    await guest.waitFor(`${q(link)}?.getAttribute('href')===${JSON.stringify(target)}`, 'verified byline link');
    await guest.click(link);
    await guest.waitFor(`location.pathname===${JSON.stringify(target)} && ${q('#author-profile-heading')}`, 'byline opens correct public profile');
    await guest.eval('history.back()');
    try {
      await guest.waitFor(`${q(link)} && document.activeElement===${q(link)}`, `Back restores ${link}`);
    } catch (error) {
      console.error('Byline Back state:', await guest.eval(`({
        path: location.pathname, hash: location.hash, state: history.state,
        active: document.activeElement?.id || document.activeElement?.tagName,
        href: ${q(link)}?.getAttribute('href')
      })`), guest.errors);
      throw error;
    }
    console.log('  ok  Back restores', link);
  }
  const commentHash = '#comment-' + ids.reply;
  const jump = '#comment-' + ids.tail + ' a[href="' + commentHash + '"]';
  check(await guest.eval(`!!${q(jump)}`), 'reply context still has a separate comment jump');
  await guest.eval(`${q('#comment-' + ids.tail)}.scrollIntoView({block:'center'})`);
  await guest.capture('author-reply-context', {scenario:'reader discussion bylines', textScale:100});
  await guest.click(jump);
  await guest.waitFor(`location.hash===${JSON.stringify(commentHash)}`, 'reply context jumps to the replied-to comment');
  const workApi = '/api/v1/works/' + ids.a;
  await guest.send('Network.setCacheDisabled', {cacheDisabled:true});
  await restoredAttribution(guest, check, holdAnswer, {
    url:base + discovery, endpoint:'/api/v1/works', links:{[cardA]:aPath, [cardB]:bPath},
    labels:[rowA + ' .vk-ui-byline__name', '#work-' + ids.b + ' .vk-ui-byline__name'], content:titleA,
  });
  await restoredAttribution(guest, check, holdAnswer, {
    url:base + readerPath, endpoint:workApi + '/reader', links:{'#reader-author':aPath},
    labels:['.vk-work-byline'], content:'#reader-paper',
  });
  await restoredAttribution(guest, check, holdAnswer, {
    url:base + readerPath, endpoint:workApi + '/discussion/public',
    links:Object.fromEntries(bylines.slice(1)),
    labels:['#comment-' + ids.root + ' .vk-discussion-byline .vk-ui-byline__name', '#comment-' + ids.reply + ' .vk-discussion-byline .vk-ui-byline__name',
      '#comment-' + ids.tail + ' .vk-discussion-context'], content:'#comment-' + ids.root,
  });
  await guest.send('Network.setCacheDisabled', {cacheDisabled:false});
  console.log('  ok  persisted pageshow revalidates Discovery, Reader and reply context before restoring links');
  // Freeze a document carrying B's byline before retiring B in this disposable fixture.
  await guest.goto(base + readerPath);
  await guest.waitFor(`document.documentElement.dataset.hydrated==='true' && ${q(bylines[2][0])}`,
    'linked byline is hydrated before controlled retirement');
  const readerHistory = await guest.send('Page.getNavigationHistory');
  const readerEntry = readerHistory.entries[readerHistory.currentIndex].id;
  await guest.eval('window.fixtureProfileDocument = true');
  await guest.eval(`window.addEventListener('pageshow', event => {
    if (event.persisted) sessionStorage.setItem('fixture-profile-bfcache', 'restored');
  });`);
  // Create a newer SPA entry in the same document, then traverse straight past it.
  await guest.click('#reader-author');
  await guest.waitFor(`location.pathname===${JSON.stringify(aPath)} && ${q('#author-profile-heading')}`, 'same-document Profile entry before freeze');
  check(await guest.eval('window.fixtureProfileDocument === true'), 'Reader and linked profile share the same document before freeze');
  await guest.goto(base + bPath);
  check(authorFixtureSql(`UPDATE author_profiles SET current_handle=NULL, version=version+1
    WHERE principal_id=${literal(b.principal_id)} AND current_handle=${literal(b.handle)} RETURNING 1;`) === '1',
    'retired exactly the B fixture');
  await guest.reload();
  await guest.waitFor(`${q('#author-profile-heading')}?.textContent==='Hồ sơ không còn trên Vink'`, 'B profile is gone');
  const history = await guest.send('Page.getNavigationHistory');
  check(history.entries[history.currentIndex - 2]?.id === readerEntry,
    'two-step Back targets the earlier Reader entry in the frozen document');
  const missed = [];
  const onMiss = (message) => {
    if (message.sessionId === guest.sessionId && message.method === 'Page.backForwardCacheNotUsed') {
      missed.push(...(message.params.notRestoredExplanations || []).map(item => item.reason));
    }
  };
  guest.cdp.listeners.push(onMiss);
  await guest.eval('history.go(-2)');
  try {
    await guest.waitFor(`location.pathname===${JSON.stringify(readerPath)} &&
      ${q('#comment-' + ids.reply)}?.textContent.includes('Tài khoản đã xóa') &&
      !${q('a[href="' + bPath + '"]')}`, 'Back does not restore retired attribution');
  } catch (error) {
    console.error('Controlled Back state:', await guest.eval(`({
      path: location.pathname,
      hydrated: document.documentElement.dataset.hydrated,
      visibility: ${q('#noi-dung')} && getComputedStyle(${q('#noi-dung')}).visibility,
      frozenDocument: window.fixtureProfileDocument === true,
      restored: sessionStorage.getItem('fixture-profile-bfcache'),
      author: ${q('#comment-' + ids.reply)}?.textContent,
      retiredLink: !!${q('a[href="' + bPath + '"]')}
    })`), guest.errors);
    throw error;
  }
  console.log('  controlled Back used BFCache:', await guest.eval("sessionStorage.getItem('fixture-profile-bfcache')==='restored'"),
    'reported misses:', missed);
  for (const tab of [guest, viewing, staleTab]) {
    check(tab.errors.length === 0, `profile/byline tab console and hydration: ${tab.errors.join(' | ')}`);
  }
  await page.cdp.send('Target.disposeBrowserContext', {browserContextId:context.browserContextId});

  // Keep the original password/Handle scenario identities and restore its foreground page.
  check(authorFixtureSql(`DELETE FROM publications
    WHERE work_id IN (${literal(ids.a)}, ${literal(ids.b)}) RETURNING 1;`) === '1\n1',
    'removed both profile fixtures from the public corpus before the zero-work Handle checks');
  await edit(page, base);
  await fill(page, NAME, a.pen_name);
  await save(page);
  await page.send('Page.bringToFront');
  await page.goto(base + '/toi');
}
