import { STANDING, cookies, fill, q, signOut } from './auth-smoke.mjs';
import { captureRequest, holdAnswer } from './auth-browser-network.mjs';
import { matchesAXButton } from './visual-evidence.mjs';

const WORK = '33333333-3333-4333-8333-333333333333';
const READER = '/tac-pham/' + WORK;
const LIST = '/api/v1/me/saved-works';
const PAIR = LIST + '/' + WORK;
const NOT_FOUND = '[data-panel="error:not-found"]';
const ownLink = '#save-work-list a';
const open = '#saved-open-' + WORK;
const remove = '#saved-remove-' + WORK;

async function read(page, path) {
  return page.eval(`fetch(${JSON.stringify(path)}, {cache:'no-store'}).then(async r => ({
    status:r.status, cache:r.headers.get('cache-control'), body:r.ok ? await r.json() : null
  }))`);
}

async function documentFacts(page, path) {
  return page.eval(`fetch(${JSON.stringify(path)}).then(async r => {
    const html = await r.text();
    const dom = new DOMParser().parseFromString(html, 'text/html');
    return {status:r.status, privateMarkup:!!dom.querySelector('#profile-modes,#saved-list,#saved-privacy-note')
      || /href="[^"]*\\/da-luu"/.test(html), notFound:dom.querySelector('${NOT_FOUND}')?.textContent.trim() ?? null};
  })`);
}

export async function savedProfileJourney(page, { args, check, ssr = false, lost = false }) {
  const { base } = args;
  await page.viewport(1024, 900);
  await page.goto(base + READER);
  await page.waitFor(`${q('#save-work')}?.hasAttribute('aria-pressed') &&
    ${q('#save-work')}?.getAttribute('aria-busy') !== 'true'`, '#389 real Reader membership loaded');
  if (await page.eval(`${q('#save-work')}?.getAttribute('aria-pressed') === 'true'`)) {
    await page.click('#save-work');
    await page.waitFor(`${q('#save-work')}?.getAttribute('aria-pressed') === 'false'`, '#389 prepare a fresh Reader save');
  }
  await page.click('#save-work');
  await page.waitFor(q(ownLink), '#389 Reader ACK opens the private destination');
  const path = await page.eval(`${q(ownLink)}.getAttribute('href')`);
  const publicPath = path.replace(/\/da-luu$/, '');
  const [pub, saved, unknown] = await Promise.all([
    documentFacts(page, publicPath), documentFacts(page, path), documentFacts(page, '/khong/co/trang-nay'),
  ]);
  check(!pub.privateMarkup && !saved.privateMarkup &&
    (!ssr || (saved.status === 404 && unknown.status === 404 && saved.notFound === unknown.notFound)),
    '#389 documents omit private markup; SSR saved route has the ordinary 404 status and presentation');
  if (ssr) {
    const guest = await fetch(base + path);
    const html = await guest.text();
    check(guest.status === 404 && !html.includes('id="saved-list"') && html.includes('Không tìm thấy trang này.'),
      '#389 SSR direct request without a session renders the same generic page');
  }
  await page.click(ownLink);
  await page.waitFor(q(open), '#389 real Reader → Mở Đã lưu → matching row');
  const title = await page.eval(`${q(open)}.textContent.trim()`);
  check(await matchesAXButton(page, remove, {name:'Bỏ lưu: ' + title}),
    '#389 Chrome AX identifies a separate unsave button with the readable title');
  const list = await read(page, LIST);
  check(list.status === 200 && list.cache === 'private, no-store' && list.body.items.some(row => row.work_id === WORK)
    && !('total' in list.body) && await page.eval(`${q('#profile-modes [aria-current=page]')}?.textContent.trim() === 'Đã lưu'`),
    '#389 owner list response is no-store and has no total, and navigation marks the current page');

  async function chooseMode(mode) {
    const wide = '#profile-mode-' + mode;
    if (await page.eval(`${q(wide)}?.checkVisibility()`)) {
      await page.click(wide);
    } else {
      await page.click('#profile-modes summary');
      await page.click(wide + '-compact');
    }
  }
  await chooseMode('works');
  await page.waitFor(`${q('#profile-modes')} && location.pathname === ${JSON.stringify(publicPath)}`, '#389 owner can move to Tác phẩm');
  await chooseMode('saved');
  await page.waitFor(q(open), '#389 owner can open Đã lưu from the public profile');
  if (!ssr) await geometry(page, check);

  const beforeTop = await page.eval(`${q(open)}.focus(); ${q(open)}.getBoundingClientRect().top`);
  await page.key('Enter');
  await page.waitFor(`location.pathname === '${READER}' && ${q('#reader-paper')}`, '#389 real keyboard opening to Reader');
  await page.eval('history.back()');
  await page.waitFor(`location.pathname === ${JSON.stringify(path)} && document.activeElement?.id === 'saved-open-${WORK}'`,
    '#389 Back verifies the list and restores focus by WorkId');
  check(Math.abs(await page.eval(`${q(open)}.getBoundingClientRect().top`) - beforeTop) <= 8,
    '#389 Back preserves the row viewport position where the page allows it');
  await page.key('Tab');
  await page.waitFor(`document.activeElement?.id === 'saved-remove-${WORK}'`, '#389 Tab moves from the Reader link to Bỏ lưu');
  const deletes = captureRequest(page, 'DELETE', PAIR);
  const hold = await holdAnswer(page, 'DELETE', PAIR);
  await page.key('Space');
  check(await hold.held() === 200 && await page.eval(`${q(remove)}?.getAttribute('aria-busy') === 'true' &&
    document.activeElement?.id === 'saved-remove-${WORK}' && ${q(open)}`),
    '#389 keeps the row and focus until the real unsave ACK');
  await page.key('Enter');
  await hold.finish(lost, true);
  if (lost) {
    await page.waitFor(`${q(open)} && ${q('#saved-list')}?.textContent.includes('Chưa xác nhận được.')`, '#389 lost ACK keeps the confirmed row');
    await page.click('#saved-list li .vk-saved-row__error button');
  }
  await page.waitFor(`!${q(open)} && ${q('#saved-snack')}?.textContent.includes('Đã bỏ lưu')`, '#389 unsave confirmed and announced');
  check(deletes.count() === 1 && (await read(page, PAIR)).body?.saved === false,
    '#389 exactly one DELETE, and pair readback confirms NotSaved');
  deletes.close();
  await page.waitFor(`${q('#saved-empty-heading')} && document.activeElement === ${q('#saved-empty-heading')}`,
    '#389 when the final focused row disappears, focus goes to the empty heading');
  return { path, publicPath };
}

async function geometry(page, check) {
  const bad = [];
  for (const width of [320, 390, 1024]) {
    for (const theme of ['light', 'dark']) {
      for (const scale of [100, 150, 200]) {
        await page.viewport(width, 900);
        const facts = await page.eval(`(async () => {
          document.documentElement.dataset.theme = '${theme}';
          document.documentElement.style.fontSize = '${scale}%';
          await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
          const a = ${q(open)}.getBoundingClientRect(), b = ${q(remove)}.getBoundingClientRect();
          return {fits:document.scrollingElement.scrollWidth <= innerWidth + 1 && b.right <= innerWidth + 1,
            sizing:a.height >= 43.5 && Math.abs(b.height - 48) < 0.5 && Math.abs(b.width - 48) < 0.5,
            avatar:document.querySelector('.vk-saved-byline .vk-ui-avatar').getBoundingClientRect().width <= 36};
        })()`);
        if (!(facts.fits && facts.sizing && facts.avatar)) bad.push([width, theme, scale, facts]);
        if ((width === 390 && scale === 100) || (width === 320 && theme === 'dark' && scale === 200) ||
            (width === 1024 && theme === 'light' && scale === 100)) {
          if (width === 320) {
            await page.eval("document.querySelector('#saved-list').scrollIntoView({block:'center',behavior:'instant'})");
          }
          await page.capture(`saved-profile-${width}-${theme}-${scale}`, {requiredTargets:['#saved-list']});
        }
      }
    }
  }
  check(bad.length === 0, '#389 responsive geometry across 18 viewport, theme and text-size combinations ' + JSON.stringify(bad));
  await page.eval("document.documentElement.style.fontSize = ''; document.documentElement.dataset.theme = 'light'");
  await page.viewport(1024, 900);
}

export async function savedProfilePrivacy(page, { args, check, openPage, paths, other }) {
  const { base } = args;
  const { path, publicPath } = paths;
  const { browserContextId } = await page.cdp.send('Target.createBrowserContext');
  const outsider = await openPage(browserContextId);
  try {
    const outsiderCalls = captureRequest(outsider, 'GET', LIST);
    await outsider.goto(base + publicPath);
    await outsider.waitFor(q('#author-works-heading'), '#389 guest public profile');
    check(await outsider.eval(`!${q('#profile-modes')}`), '#389 guest public profile has no saved navigation');
    check(await outsider.eval(`${q('#author-works-heading')}.textContent.trim() === 'Tác phẩm'
      && ${q('#author-works-heading')}.closest('section')?.getAttribute('aria-labelledby') === 'author-works-heading'`),
      '#585 public works retain a named section with the shorter heading');
    await outsider.goto(base + path);
    await outsider.waitFor(q(NOT_FOUND), '#389 guest direct destination returns ordinary NotFound');
    check(outsiderCalls.count() === 0, '#389 guest never requests the private list');
    await outsider.goto(base + '/toi');
    await outsider.waitFor(`${STANDING} === 'anonymous'`, '#389 CSR guest account form');
    await fill(outsider, '#password-login-username', other.username);
    await fill(outsider, '#password-login-password', other.password);
    await outsider.click('#password-login-submit');
    await outsider.waitFor(q('#sign-out'), '#389 second real account signed in');
    const outsiderProfile = await read(outsider, '/api/v1/me/profile');
    check(outsiderProfile.status === 200 && '/@' + outsiderProfile.body.handle !== publicPath,
      '#389 the second session is a different verified profile');
    await outsider.goto(base + publicPath);
    await outsider.waitFor(q('#author-works-heading'), '#389 another account can see the public works');
    await outsider.goto(base + path);
    await outsider.waitFor(q(NOT_FOUND), '#389 another account cannot open the private destination');
    check(outsiderCalls.count() === 0 && await outsider.eval(`!${q('#saved-list')} && !${q('#profile-modes')}`),
      '#389 another account has the generic page and no private list request');
    outsiderCalls.close();

    await page.goto(base + path);
    await page.waitFor(q('#saved-empty-heading'), '#389 current owner sees only its own empty state');
    await page.goto(base + READER);
    await page.waitFor(`${q('#save-work')}?.getAttribute('aria-pressed') === 'false'`, '#389 prepare a populated answer for the stale-response check');
    await page.click('#save-work');
    await page.waitFor(q(ownLink), '#389 new confirmed save for the stale-response check');
    await page.goto(base + '/toi');
    await page.waitFor(`${STANDING} === 'active'`, '#389 current owner can sign out in the other tab');
    const watcher = await openPage();
    await watcher.goto(base + '/toi');
    await watcher.waitFor(q('#sign-out'), '#389 same account in second tab');
    const stale = await holdAnswer(watcher, 'GET', LIST);
    await watcher.goto(base + path);
    check(await stale.held() === 200, '#389 real old-owner list response held in a second tab');
    await signOut(page, base);
    await watcher.waitFor(q(NOT_FOUND), '#389 logout in another tab fences the old saved projection');
    const cookie = (await cookies(outsider, base))['__Host-vink_session'];
    await watcher.send('Network.setCookie', {
      name:cookie.name, value:cookie.value, url:base, path:'/', secure:true, httpOnly:true, sameSite:cookie.sameSite,
    });
    await watcher.eval("window.dispatchEvent(new Event('focus'))");
    await stale.finish(false, true);
    check(await watcher.eval(`!!${q(NOT_FOUND)} && !${q('#saved-list')} && !${q('#profile-modes')}`),
      '#389 old response cannot render after logout and replacement with another real session');
  } finally {
    await page.cdp.send('Target.disposeBrowserContext', {browserContextId});
  }
}
