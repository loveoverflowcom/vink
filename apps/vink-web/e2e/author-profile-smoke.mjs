// #292/#293. Real Chrome, password sessions, App and scratch PostgreSQL.
// The works/comments and retirement are controlled fixtures, not account deletion.
import { createHash, randomUUID } from 'node:crypto';
import { authorFixtureSql } from './author-fixtures.mjs';
import { fill, q } from './auth-smoke.mjs';

const API = '/api/v1/me/profile';
const EDIT = '/toi/chinh-ho-so';
const NAME = '#profile-pen-name';
const BIO = '#profile-bio';

const BACK = '#profile-back';
const BIO_SCOPE = '#profile-bio-policy';
const SAME_NAME = 'Một bút danh tiếng Việt rất dài dùng chung trên tác phẩm và nhận xét';
const own = (page) => page.eval(`fetch('${API}').then(r => r.json())`);

const selector = (id) => '#' + id;

async function edit(page, base) {
  await page.goto(base + EDIT);
  await page.waitFor(q(NAME), 'profile fields for the confirmed account');
}

async function commitName(page) {
  await page.key('Enter');
  await page.waitFor(`${q(NAME + '-feedback')}?.textContent.includes('Đã lưu')`, 'name acknowledged');
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

// Preserve the caller's real appearance settings across each trip to the profile.
async function accountR5Layouts(page, check) {
  const choose = async (group, value) => {
    const selected = `${q('#page-' + group + ' input:checked')}?.value === ${JSON.stringify(value)}`;
    if (!await page.eval(selected)) await page.click(`#page-${group} label:has(input[value="${value}"])`);
    await page.waitFor(selected, 'account appearance choice applied');
  };
  const previous = await page.eval("['theme','text'].map(group => document.querySelector('#page-' + group + ' input:checked').value)");
  for (const width of [390, 1440]) {
    await page.viewport(width);
    for (const theme of ['light', 'dark']) {
      await choose('theme', theme);
      for (const scale of ['100', '150']) {
        await choose('text', scale);
        await page.waitFor(`document.documentElement.dataset.theme === '${theme}' &&
          (document.documentElement.dataset.text || '100') === '${scale}'`,
          'actual account appearance reached its selected theme and scale');
        const header = await page.eval(`(async () => {
          const hero = ${q('.vk-account-status')}, title = ${q('#me-title')};
          hero.scrollIntoView({block:'start'});
          await document.fonts.ready;
          const h = hero.getBoundingClientRect(), t = title.getBoundingClientRect();
          const a = hero.querySelector('.vk-account-avatar').getBoundingClientRect();
          return { parent:h.width, title:t.width, below:t.top>=a.bottom-1,
            inside:t.left>=-1 && t.right<=innerWidth+1 && document.documentElement.scrollWidth<=innerWidth+1 };
        })()`);
        check(header.inside && (width > 390 || header.below && Math.abs(header.parent - header.title) <= 1),
          `account identity reflows at ${width}/${theme}/${scale}`);
        await page.capture(`r5-account-${width}-${theme}-${scale}`, {
          textScale:Number(scale), textScaleLane:'app-appearance-control', requiredTargets:['#me-title'],
        });
        await page.click('#edit-profile-link');
        await page.waitFor(q(NAME), 'R5 opens from the account');
        const facts = await page.eval(`(() => {
          const back = ${q(BACK)}, r = back.getBoundingClientRect();
          back.scrollIntoView({block:'start'});
          return { hit:r.width>=48 && r.height>=48, inline:!document.querySelector('#profile-save, .vk-profile-form [type=submit]'),
            name:back.getAttribute('aria-label'), href:back.getAttribute('href'),
            inside:document.documentElement.scrollWidth<=innerWidth+1 };
        })()`);
        check(facts.hit && facts.inline && facts.inside && facts.name === 'Về Tài khoản' &&
          facts.href === '/toi#edit-profile-link', `profile Back, form and viewport at ${width}/${theme}/${scale}`);
        await page.capture(`r5-profile-${width}-${theme}-${scale}`, {
          textScale:Number(scale), textScaleLane:'app-appearance-control',
          requiredTargets:[BACK, '#edit-profile-heading', NAME],
        });
        await page.click(BACK);
        await page.waitFor("document.activeElement?.id === 'edit-profile-link'", 'R5 Back returns to its account entry');
      }
    }
  }
  await page.viewport(1280);
  await choose('theme', previous[0]);
  await choose('text', previous[1]);
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
  // The preceding multi-account scenario last opened a different tab.
  await page.send('Page.bringToFront');
  await page.goto(base + '/toi');
  await page.waitFor(q('#edit-profile-link'), 'Me shows the profile entry');
  await accountR5Layouts(page, check);
  await page.eval(`${q('#edit-profile-link')}.focus()`);
  await page.key('Enter');
  await page.waitFor(q(NAME), 'keyboard opens edit from Me');
  const a = await own(page);
  const bio = 'Viết bằng mực tím.\n\nMột đoạn nữa.';
  const writes = [];
  const watch = event => {
    if (event.sessionId !== page.sessionId || event.method !== 'Network.requestWillBeSent') return;
    const {request} = event.params;
    if (request.method === 'PUT' && new URL(request.url).pathname === API) writes.push(JSON.parse(request.postData));
  };
  page.cdp.listeners.push(watch);
  await fill(page, NAME, '   ');
  await page.key('Enter');
  await page.waitFor(`${q(NAME)}.getAttribute('aria-invalid')==='true' && document.activeElement===${q(NAME)}`, 'invalid name retains focus');
  check(await page.eval(`${q(NAME)}.value==='   '`) && writes.length===0, 'invalid name stays local');
  await page.key('Escape');
  await page.waitFor(`${q(NAME)}.value===${JSON.stringify(a.pen_name)}`, 'Escape restores the acknowledged name');
  await fill(page, NAME, '  ' + a.pen_name + '  ');
  await page.key('Enter');
  check(writes.length===0,'normalized identical name sends no request');
  // The real fields can hold separate drafts (e.g. user re-entry). Seed the second
  // native input without moving focus, then use keyboard commits on the active field.
  await page.eval(`(() => {
    const el=${q(BIO)};el.value='Đang viết dở';el.dispatchEvent(new InputEvent('input',{bubbles:true}));
  })()`);
  await fill(page, NAME, 'Thử tên');
  await page.eval(`${q(NAME)}.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}))`);
  await page.key('Enter');
  check(writes.length===0,'IME Enter cannot commit a partial name');
  await page.eval(`${q(NAME)}.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}))`);
  const lost = await holdAnswer(page, 'PUT', API);
  await page.key('Enter');
  check(await lost.held()===200 && writes.length===1 && writes[0].bio===a.bio &&
    writes[0].expected_version===a.profile_version && writes[0].pen_name==='Thử tên',
    'first full PUT excludes the unfinished bio');
  await fill(page, NAME, 'Bản giữa');
  await page.key('Enter');
  await fill(page, NAME, SAME_NAME.normalize('NFD'));
  await page.key('Enter');
  check(writes.length===1 && await page.eval(`!${q(BIO)}.readOnly && !${q(NAME)}.readOnly`),
    'later name commits coalesce and fields stay editable while the real ACK is held');
  await lost.finish(true);
  await page.waitFor(`${q(NAME)}.value===${JSON.stringify(SAME_NAME)} && ${q(NAME+'-feedback')}?.textContent.includes('Đã lưu')`,
    'read-back reconciles lost ACK before sending the latest normalized intent');
  check(writes.length===2 && writes[1].expected_version===a.profile_version+1 && writes[1].bio===a.bio &&
    (await own(page)).pen_name===SAME_NAME && await page.eval(`${q(BIO)}.value==='Đang viết dở'`),
    'one successor write preserves the newer intent, draft and durable version');
  await fill(page, BIO, 'Dòng đầu');
  await page.key('Enter');
  check(writes.length===2,'plain Enter in multiline bio does not save');
  await fill(page, BIO, bio);
  await page.key('Enter',2);
  await page.waitFor(`${q(BIO+'-feedback')}?.textContent.includes('Đã lưu')`,'Ctrl+Enter saves bio');
  check((await own(page)).bio===bio && writes.length===3, 'bio read back from the real server');
  page.cdp.listeners=page.cdp.listeners.filter(l=>l!==watch);
  // Drop the next PUT before delivery and its first read-back. Other routes,
  // including session checks, stay connected to their real owners.
  const failed = [];
  const offline = event => {
    if (event.sessionId!==page.sessionId || event.method!=='Fetch.requestPaused') return;
    const {requestId,request}=event.params;
    if (new URL(request.url).pathname===API && !failed.includes(request.method)) {
      failed.push(request.method);
      page.send('Fetch.failRequest',{requestId,errorReason:'InternetDisconnected'}).catch(()=>{});
    } else page.send('Fetch.continueRequest',{requestId}).catch(()=>{});
  };
  page.cdp.listeners.push(offline);
  await page.send('Fetch.enable',{patterns:[{urlPattern:'*'+API,requestStage:'Request'}]});
  await fill(page,BIO,'Chờ đối chiếu');
  await page.key('Enter',2);
  await page.waitFor(`${q(BIO+'-feedback button')}?.textContent.includes('Đối chiếu lại')`,'unknown result offers read-back');
  page.cdp.listeners=page.cdp.listeners.filter(l=>l!==offline);
  await page.send('Fetch.disable');
  check(failed.join(',')==='PUT,GET' && (await own(page)).bio===bio,'unreachable write leaves durable bio alone');
  await page.click(BACK);
  await page.waitFor(q('#profile-leave[open]'),'unknown result protected on Back');
  await page.click('#profile-leave-keep');
  await page.waitFor(q('#edit-profile-link'),'return to Account during unknown result');
  await page.click('#edit-profile-link');
  await page.waitFor(`${q(BIO)}?.value==='Chờ đối chiếu' && ${q(BIO+'-feedback button')}?.textContent.includes('Đối chiếu lại')`,
    'route re-entry keeps the uncertain command');
  await page.click(BIO+'-feedback button');
  await page.waitFor(`${q(BIO+'-feedback button')}?.textContent.includes('Thử lại')`,'unchanged read-back requires explicit retry');
  check((await own(page)).bio===bio,'read-back does not replay an unseen command');
  await page.click(BIO+'-feedback button');
  await page.waitFor(`${q(BIO+'-feedback')}?.textContent.includes('Đã lưu')`,'explicit retry stored the requested bio');
  await fill(page,BIO,'');
  await page.eval(`${q(BIO)}.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));${q(NAME)}.focus()`);
  check((await own(page)).bio==='Chờ đối chiếu','blur during composition defers clearing the stored bio');
  await page.eval(`${q(BIO)}.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}))`);
  await page.waitFor(`${q(BIO+'-feedback')}?.textContent.includes('Đã lưu')`,'composition end settles the deferred blur');
  check((await own(page)).bio===null,'clearing bio is persisted as null');
  await fill(page,BIO,bio);
  await page.eval(`${q(NAME)}.focus()`);
  await page.waitFor(`${q(BIO+'-feedback')}?.textContent.includes('Đã lưu')`,'ordinary text blur commits bio');

  await page.click(BACK);
  await page.waitFor(`${q('#me-title')}?.textContent.trim() === ${JSON.stringify(SAME_NAME)}`, 'stored long name returns on Account');
  await page.viewport(320, 800);
  const stress = await page.eval(`(async () => {
    const root = document.documentElement;
    root.style.fontSize = '200%';
    const title = ${q('#me-title')};
    title.closest('.vk-account-status').scrollIntoView({block:'start'});
    await document.fonts.ready;
    const node = [...title.childNodes].find(node => node.nodeType === Node.TEXT_NODE && node.textContent.trim());
    const wholeWords = node && [...node.textContent.matchAll(/\\S+/gu)].every(word => {
      const range = document.createRange();
      range.setStart(node, word.index);
      range.setEnd(node, word.index + word[0].length);
      return new Set([...range.getClientRects()].map(rect => Math.round(rect.top))).size === 1;
    });
    const avatar = title.closest('.vk-account-status').querySelector('.vk-account-avatar');
    const identity = title.getBoundingClientRect();
    const bottom = avatar.getBoundingClientRect().bottom;
    return {wholeWords, stacked: identity.top >= bottom-1, fits: root.scrollWidth<=root.clientWidth+1,
      scale: getComputedStyle(root).fontSize};
  })()`);
  check(stress.wholeWords && stress.stacked && stress.fits && stress.scale === '32px',
    `320px Account root 200% keeps the long name readable and clear of avatar (${JSON.stringify(stress)})`);
  await page.capture('r5-account-320-stress-200', {
    textScale:200, textScaleLane:'dom-override-geometry-only', requiredTargets:['#me-title'],
  });
  await page.eval("document.documentElement.style.fontSize=''");
  await page.viewport(1280);
  await page.click('#edit-profile-link');
  await page.reload();
  await page.waitFor(`${q(BIO)}?.value === ${JSON.stringify(bio)}`, 'Bio survives reload');
  check(await page.eval(`!!document.querySelector('a[aria-current="page"][href="/toi"]')`),
    'Tôi is active in the profile editor');
  await captureLayouts(page, check, 'edit', '#profile-pen-name, #profile-bio, .vk-profile-form select');
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme='${theme}'`);
    await page.waitFor(`!document.getAnimations().some(a => a.playState==='running' &&
      a.effect.target?.checkVisibility() && Number.isFinite(a.effect.getComputedTiming().endTime))`,
      'profile colors settled');
    await textContrast(page, theme, 'profile', ['#profile-pen-name', '#profile-bio', '#profile-pen-name-help']);
  }
  await page.eval("delete document.documentElement.dataset.theme");
  await page.viewport(320, 580);
  await page.eval("document.documentElement.style.fontSize='200%'");
  await page.eval(`${q(BIO_SCOPE)}.focus(); ${q(BIO_SCOPE)}.scrollIntoView({block:'center'})`);
  await page.key('Space');
  await page.key('ArrowDown');
  await page.key('Enter');
  await page.waitFor(`document.activeElement===${q(BIO_SCOPE+'-cancel')}`, 'confirmation starts at Cancel');
  await page.key('Tab');
  await page.waitFor(`(() => {
    const el=${q(BIO_SCOPE+'-confirm')}, r=el.getBoundingClientRect();
    return document.activeElement===el && r.bottom<=${q('.vk-ui-bottom-nav')}.getBoundingClientRect().top+1 &&
      el.contains(document.elementFromPoint(r.x+r.width/2,r.y+r.height/2));
  })()`, 'Tab reaches the unobscured confirmation at 320px/200%');
  await page.capture('author-edit-short-viewport', {scenario:'privacy confirmation focused',textScale:200});
  await page.key('Escape');
  await page.waitFor(`document.activeElement===${q(BIO_SCOPE)} && ${q(BIO_SCOPE)}.value==='everyone'`, 'Escape cancels narrowing');
  await page.viewport(1280);
  await page.eval("document.documentElement.style.fontSize=''");
  await fill(page, BIO, 'Nháp trước khi về Tôi');
  await page.click(BACK);
  await page.waitFor(q('#profile-leave[open]'), 'leaving with an unfinished bio asks first');
  await page.click('#profile-leave-keep');
  check((await own(page)).bio===bio,'pointer navigation did not publish an unfinished bio');
  await page.waitFor("document.activeElement?.id==='edit-profile-link'", 'cancel returns focus to Me entry');
  await page.key('Enter');
  await page.waitFor(`${q(BIO)}?.value==='Nháp trước khi về Tôi'`, 'cancel preserves draft in the same session');
  await page.eval(`${q(BIO)}.focus()`);
  await page.key('Escape');

  await edit(other, base);
  const b = await own(other);
  await fill(other, NAME, SAME_NAME);
  await commitName(other);
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
  // #587: one editorial row per work. The title link is the only target, its overlay covers the
  // row, and only the public facts the fixture has appear: the form tag, the exact first public
  // time that the pointer insert recorded (read back from the public API) and the excerpt.
  const row = await guest.eval(`(async () => {
    const link = ${q(selector('profile-work-' + ids.a))}, item = link?.closest('li');
    if (!item) return {};
    const api = await fetch(${JSON.stringify(profileApi)}).then(r => r.json());
    const first = api.publications?.works?.find(w => w.work_id === ${JSON.stringify(ids.a)})?.first_published_at ?? null;
    item.scrollIntoView({block:'center'});
    const meta = item.querySelector('.vk-profile-work__meta'), excerpt = item.querySelector('.vk-profile-work__excerpt');
    const title = item.querySelector('h3'), r = excerpt?.getBoundingClientRect();
    const hit = r && document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
    const time = item.querySelector('time');
    const label = time ? new Intl.DateTimeFormat('vi-VN', {}).format(new Date(time.dateTime)) : null;
    return {
      row: item.classList.contains('vk-profile-work'), heading: title?.contains(link),
      name: link.textContent, href: link.getAttribute('href'), back: link.dataset.profileReturn,
      targets: item.querySelectorAll('a, button').length, meta: meta?.textContent ?? null, first, label,
      date: !!first && item.querySelectorAll('time').length === 1 && time.getAttribute('datetime') === first
        && time.getAttribute('aria-description')?.startsWith('Công khai lần đầu: ') && !!meta?.contains(time) && !time.closest('a') && time.textContent === label,
      excerpt: excerpt?.textContent ?? null,
      above: !!meta && meta.getBoundingClientRect().bottom <= title.getBoundingClientRect().top + 1,
      whole: hit === link, chevron: !!document.querySelector('.vk-author-works .vk-ui-link'),
      noByline: !item.textContent.includes(${JSON.stringify(SAME_NAME)}),
      noCounters: !item.textContent.includes('Thấm') && !item.textContent.includes('nhận xét'),
    };
  })()`);
  check(row.row && row.heading && row.name === 'Tác phẩm thứ nhất của riêng người viết A' &&
    row.href === readerPath && row.back === 'true' && row.targets === 1 &&
    row.date && row.meta === 'Tản văn · ' + row.label && row.excerpt === 'Mưa bên bậc cửa rồi lặng.' &&
    row.above && row.whole && !row.chevron && row.noByline && row.noCounters,
    `#587 editorial row keeps one link and shows only public facts ${JSON.stringify(row)}`);
  await guest.capture('author-public-desktop', {scenario:'guest with Bio and works', textScale:100});
  await captureLayouts(guest, check, 'public', '#author-profile-heading, .vk-author-bio, #author-works-heading, .vk-author-works a, .vk-profile-work__excerpt');
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
  const control = q(BIO_SCOPE);
  await page.send('Page.bringToFront');
  await page.eval(`${control}.scrollIntoView({block:"center"}); ${control}.focus()`);
  await page.key('Space');
  await page.waitFor(`${control}.matches(":open")`, 'open the real icon picker with the keyboard');
  check(await page.eval(`Boolean(
    ${control}.querySelector('option[value="everyone"] [data-icon="public"]') &&
    ${control}.querySelector('option[value="self"] [data-icon="lock"]') &&
    ${control}.parentElement.querySelector('.vk-ui-select__value-icon [data-icon="public"]')
  )`), 'current value and both options show their catalog icons');
  await page.capture('r5-profile-privacy-picker', {
    scenario:'Chrome icon picker open by keyboard', requiredTargets:[BIO_SCOPE],
  });
  await page.key('ArrowDown');
  await page.key('Escape');
  await page.waitFor(`!${control}.matches(":open") && document.activeElement === ${control} &&
    ${control}.value === "everyone"`, 'Escape cancels and restores focus without changing the policy');
  await page.key('Space');
  await page.waitFor(`${control}.matches(":open")`, 'reopen the icon picker');
  await page.click('#profile-privacy-heading');
  await page.waitFor(`!${control}.matches(":open") && ${control}.value === "everyone"`, 'outside click dismisses without changing the policy');
  await page.eval(`${control}.focus()`);
  await page.key('Space');
  await page.waitFor(`${control}.matches(":open")`, 'reopen the icon picker after outside click');
  await page.key('ArrowDown');
  await page.key('Enter');
  await page.waitFor(`${control}.value === "self" && !${control}.matches(":open") &&
    ${q(BIO_SCOPE+'-confirm')}?.checkVisibility()`, 'keyboard commits only the draft and shows the narrowing warning');
  check((await own(page)).policies.bio === 'everyone', 'choosing a value sends no durable update before Save');
  // Delay the other tab's broadcast refresh so that its first PUT really
  // reaches the server with a stale version. Its conflict GET passes the gate.
  const remoteRefresh=await holdAnswer(staleTab,'GET',API);
  const staleWrites=[];
  const watchPut=event=>{
    if (event.sessionId===staleTab.sessionId && event.method==='Network.requestWillBeSent' &&
        event.params.request.method==='PUT' && new URL(event.params.request.url).pathname===API) {
      staleWrites.push(JSON.parse(event.params.request.postData));
    }
  };
  staleTab.cdp.listeners.push(watchPut);
  await page.click(BIO_SCOPE+'-confirm');
  await page.waitFor(q(BIO_SCOPE + ':not(select) [data-icon="lock"]'), 'stored private policy becomes a non-interactive value');
  const narrowed = await own(page);
  const widen = {principal_id:narrowed.principal_id, expected_version:narrowed.profile_version,
    pen_name:narrowed.pen_name, bio:narrowed.bio, policies:{...narrowed.policies, bio:'everyone'}};
  const refused = await page.eval(`fetch('${API}', {method:'PUT', headers:{'content-type':'application/json'},
    body:${JSON.stringify(JSON.stringify(widen))}}).then(async r => [r.status, (await r.json()).error?.field])`);
  check(refused[0] === 422 && refused[1] === 'policies' && (await own(page)).policies.bio === 'self',
    'the real API independently rejects widening the stored locked policy');
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
  check(await remoteRefresh.held()===200,'a real own-profile refresh is held after privacy changed');
  await staleTab.eval(`${q(BIO)}.focus()`);
  await staleTab.key('Enter',2);
  await staleTab.waitFor(`${q(BIO+'-feedback')}?.textContent.includes('Đã lưu')`, 'bio commits independently of a remotely narrowed policy');
  check(staleWrites.length===2 && staleWrites[1].expected_version===staleWrites[0].expected_version+1 &&
    staleWrites[0].policies.bio==='everyone' && staleWrites[1].policies.bio==='self' &&
    (await own(staleTab)).policies.bio==='self', 'real stale PUT rebases once, retaining the remotely locked key');
  await remoteRefresh.finish(false,true);
  staleTab.cdp.listeners=staleTab.cdp.listeners.filter(l=>l!==watchPut);
  await fill(staleTab,BIO,'Tranh chấp cục bộ');
  await fill(page,BIO,bio);
  await page.key('Enter',2);
  await page.waitFor(`${q(BIO+'-feedback')}?.textContent.includes('Đã lưu')`, 'remote tab commits a divergent bio');
  await staleTab.send('Page.bringToFront');
  await staleTab.eval(`${q(BIO)}.focus()`);
  await staleTab.key('Enter',2);
  await staleTab.waitFor(`${q(BIO+'-feedback')}?.textContent.includes('ở nơi khác')`, 'only divergent bio needs a choice');
  check(await staleTab.eval(`${q(BIO)}.value==='Tranh chấp cục bộ' && !${q(NAME)}.readOnly`),'conflict retains text and independent fields');
  await staleTab.capture('author-profile-conflict',{scenario:'divergent bio', textScale:100});
  await staleTab.click(BIO+'-feedback button');
  await staleTab.waitFor(`document.activeElement===${q(BIO)} && ${q(BIO)}.value===${JSON.stringify(bio)}`, 'accept saved bio and return focus');
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

  // A write has reached App, but its response and the next committed intent
  // still belong to the old session generation.
  const lateWrite = await holdAnswer(staleTab,'PUT',API);
  await staleTab.key('Enter',2);
  check(await lateWrite.held()===200,'A write reached durable commit before sign-out');
  await fill(staleTab,BIO,'Lệnh xếp hàng của phiên cũ');
  await staleTab.key('Enter',2);
  await switchActor('b');
  await staleTab.waitFor(`${q(BIO)}?.value===''`,'B never adopts A’s draft or pending status');
  await switchActor('a');
  await staleTab.waitFor(`${q(BIO)}?.value==='Nháp của phiên mới'`,'new A observes only the persisted value');
  await fill(staleTab,BIO,'Chữ mới chưa chốt');
  await lateWrite.finish(false,true);
  check(await staleTab.eval(`${q(BIO)}.value==='Chữ mới chưa chốt' && !${q(BIO+'-feedback')}`) &&
    (await own(staleTab)).bio==='Nháp của phiên mới','late ACK cannot erase new text, claim Saved or send the old queue');
  await fill(staleTab,BIO,bio);
  await staleTab.key('Enter',4);
  await staleTab.waitFor(`${q(BIO+'-feedback')}?.textContent.includes('Đã lưu')`,'Cmd+Enter commits in the new A session');

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
  // Join the real byline navigation with a member's Follow and the profile's per-field policy.
  await page.send('Page.bringToFront');
  await page.goto(base + readerPath);
  await page.waitFor(`${q(bylines[2][0])} && document.documentElement.dataset.hydrated === 'true'`,
    'member can open the other author from a verified reply byline');
  await page.eval(`${q(bylines[2][0])}.focus()`);
  await page.key('Enter');
  await page.waitFor(`location.pathname === ${JSON.stringify(bPath)} &&
    ${q('#follow-action')}?.getAttribute('aria-pressed') === 'false'`, 'reply author has a real member Follow action');
  await page.click('#follow-action');
  await page.waitFor(`${q('#follow-action')}?.getAttribute('aria-pressed') === 'true'`,
    'profile reached from reply confirms the Follow');
  await page.click('#profile-followers-link');
  await page.waitFor(`${q('#relationship-title')} && ${q('#rel-person-' + a.handle)}`,
    'public follower list includes the member who followed from the reply');
  await page.eval('history.back()');
  await page.waitFor(`${q('#author-profile-heading')} && ${q('#follow-action')}?.getAttribute('aria-pressed') === 'true'`,
    'Back returns to the same author and confirmed state');
  check(await page.eval(`!${q('#profile-following-link')}`), 'the other author’s private Following list remains hidden');
  await page.click('#follow-action');
  await page.click('#follow-confirm-unfollow');
  await page.waitFor(`${q('#follow-action')}?.getAttribute('aria-pressed') === 'false'`, 'clean up the confirmed fixture edge');
  await guest.send('Page.bringToFront');

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
  await commitName(page);
  await page.send('Page.bringToFront');
  await page.goto(base + '/toi');
}
