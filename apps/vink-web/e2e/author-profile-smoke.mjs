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
    INSERT INTO tags(id,facet,label,slug,status) VALUES (${literal(ids.tag)},'form','Tản văn','tan-van','active');
    INSERT INTO tag_names(slug,tag_id,kind) VALUES ('tan-van',${literal(ids.tag)},'canonical');
    ${works.join('\n')}
    INSERT INTO discussion_threads(id,work_id) VALUES (${literal(ids.thread)},${literal(ids.a)});
    INSERT INTO comments(id,thread_id,kind,root_id,root_kind,reply_to_id,replied_to_reply,author_id,body,review,moderation_version,created_at)
    VALUES
      (${literal(ids.root)},${literal(ids.thread)},'root',NULL,NULL,NULL,false,${literal(a)},'Nhận xét tại gốc','approved',1,now()),
      (${literal(ids.reply)},${literal(ids.thread)},'reply',${literal(ids.root)},'root',NULL,false,${literal(b)},'Lời đáp thứ nhất','approved',1,now() + interval '1 second'),
      (${literal(ids.tail)},${literal(ids.thread)},'reply',${literal(ids.root)},'root',${literal(ids.reply)},true,${literal(a)},'Gửi lại người vừa trả lời','approved',1,now() + interval '2 seconds');
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
          await page.capture(`author-${name}-${width}-${theme}-${scale}`,
            {boundary:'Chrome + App + native Identity + scratch PG', scenario:name, textScale:scale});
        }
      }
    }
  }
  await page.eval("document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme");
  await page.viewport(1280);
}

export async function authorProfiles(page, other, {base, openPage, check, holdAnswer, switchActor}) {
  console.log('Author Profile and bylines (#292/#293):');
  await page.goto(base + '/toi');
  await page.waitFor(q('#edit-profile-link'), 'Me shows the profile entry');
  await page.eval(`${q('#edit-profile-link')}.focus()`);
  await page.key('Enter');
  await page.waitFor(q(NAME), 'keyboard opens edit from Me');
  const a = await own(page);
  const bio = 'Viết bằng mực tím.\n\nMột đoạn nữa.';
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
  await guest.capture('author-public-desktop', {scenario:'guest with Bio and works', textScale:100});
  const staleTab = await openPage();
  await edit(staleTab, base);
  await fill(staleTab, BIO, 'Bản nháp còn ở tab kia');
  const viewing = await openPage();
  await viewing.goto(base + aPath);
  await viewing.waitFor(q('.vk-author-bio'), 'public profile in another tab');
  await viewing.send('Page.bringToFront');
  const delayed = await holdAnswer(viewing, 'GET', profileApi);
  await viewing.eval("window.dispatchEvent(new FocusEvent('focus'))");
  check(await delayed.held() === 200, 'hold a real public response before policy changes');
  await page.eval(`(() => {
    const select = ${q('#profile-bio-policy')};
    select.value = 'self'; select.dispatchEvent(new Event('change', {bubbles:true}));
  })()`);
  await save(page);
  await viewing.waitFor(`${q('#author-profile-heading')} && !${q('.vk-author-bio')}`, 'broadcast invalidates the previous public view');
  await delayed.finish();
  check(await viewing.eval(`!${q('.vk-author-bio')}`), 'late public answer cannot restore a revoked Bio');
  await guest.send('Page.bringToFront');
  await guest.eval("window.dispatchEvent(new FocusEvent('focus'))");
  await guest.waitFor(`${q('#author-profile-heading')} && !${q('.vk-author-bio')}`, 'isolated guest revalidates upon focus');
  await staleTab.click(SAVE);
  await staleTab.waitFor(q('#profile-conflict-heading'), 'stale form displays the current saved version');
  check(await staleTab.eval(`${q(BIO)}.value==='Bản nháp còn ở tab kia'`), 'CAS conflict preserves typed draft');
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
  await lateOwn.finish();
  check(await staleTab.eval(`${q(BIO)}.value==='Nháp của phiên mới'`), 'old A response cannot overwrite the new A form');

  const discovery = '/?q=rieng&tag=tan-van';
  await guest.goto(base + discovery);
  const cardA = selector('work-' + ids.a + '-author');
  const cardB = selector('work-' + ids.b + '-author');
  const rowA = selector('work-' + ids.a);
  await guest.waitFor(q(cardA), 'linked bylines in real discovery');
  check(await guest.eval(`!document.querySelector('a a') &&
    ${q(cardA)}.getAttribute('href')===${JSON.stringify(aPath)} && ${q(cardB)}.getAttribute('href')===${JSON.stringify(bPath)}`),
    'two cards with one pen name have distinct sibling destinations');
  await captureLayouts(guest, check, 'byline', rowA + ' a');
  await guest.viewport(320);
  await guest.eval("document.documentElement.style.fontSize='200%'");
  await guest.eval(`${q(rowA + ' .vk-public-row__link')}.focus()`);
  await guest.key('Tab');
  check(await guest.eval(`document.activeElement===${q(cardA)}`), 'Tab goes from the work to its author');
  await guest.key('Enter');
  await guest.waitFor(`location.pathname===${JSON.stringify(aPath)} && ${q('#author-profile-heading')}`, 'Enter opens the right profile');
  await guest.eval('history.back()');
  await guest.waitFor(`document.activeElement===${q(cardA)}`, 'Back restores the validated card link');
  check(await guest.eval(`location.pathname+location.search===${JSON.stringify(discovery)}`), 'Back preserves query and tag scope');
  await guest.send('Emulation.setTouchEmulationEnabled', {enabled:true, maxTouchPoints:1});
  const point = await guest.eval(`(() => {
    const el=${q(cardB)};
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
    await guest.waitFor(`document.activeElement===${q(link)}`, 'Back restores the Reader or discussion byline');
  }
  check(await guest.eval(`!!${q('#comment-' + ids.tail + ' a[href="#comment-' + ids.reply + '"]')}`),
    'reply context still has a separate comment jump');
  await guest.eval(`${q('#feedback-heading')}.scrollIntoView({block:'start'})`);
  await guest.capture('author-reply-context', {scenario:'reader discussion bylines', textScale:100});
  for (const tab of [guest, viewing, staleTab]) {
    check(tab.errors.length === 0, `profile/byline tab console and hydration: ${tab.errors.join(' | ')}`);
  }
  await page.cdp.send('Target.disposeBrowserContext', {browserContextId:context.browserContextId});

  // Keep the original password/Handle scenario identities and restore its foreground page.
  await edit(page, base);
  await fill(page, NAME, a.pen_name);
  await save(page);
  await edit(other, base);
  await fill(other, NAME, b.pen_name);
  await save(other);
  await page.send('Page.bringToFront');
  await page.goto(base + '/toi');
}
