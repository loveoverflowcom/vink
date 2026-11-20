// HANDLE-03/04. One real account, two owner tabs and an isolated guest in Chrome.
import { authorFixtureSql } from './author-fixtures.mjs';
import { fill, openAccount } from './auth-smoke.mjs';

const field = '#author-handle',
  submit = '#dia-chi-tac-gia form button[type=submit]';
const edit = '#dia-chi-tac-gia .vk-handle-text',
  approve = '.vk-handle-dialog .vk-handle-btn';
const saved = '#handle-saved';
const has = (s) => 'document.querySelector(' + JSON.stringify(s) + ')';
const own = (page) => page.eval('fetch("/api/v1/me/handle").then(r=>r.json())');

async function matrix(page, check, textContrast) {
  for (const width of [320, 390, 834, 1024, 1440])
    for (const theme of ['light', 'dark'])
      for (const scale of [100, 200]) {
        await page.viewport(width);
        const facts = await page.eval(`(async ()=>{
      const root=document.documentElement;
      root.dataset.theme='${theme}';root.style.fontSize='${scale}%';await document.fonts.ready;
      await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
      const targets=[...document.querySelectorAll(".vk-handle-columns input,.vk-handle-actions a,.vk-handle-actions button,.vk-author-plate")];
      return {width:root.clientWidth,scroll:root.scrollWidth,inside:targets.every(el=>{const r=el.getBoundingClientRect();return r.left>=0 && r.right<=root.clientWidth+1}),
        named:document.querySelector("#author-handle").labels.length===1,
        primaryHeight:document.querySelector(".vk-handle-actions button").getBoundingClientRect().height};
    })()`);
        check(
          facts.inside &&
            facts.scroll <= facts.width + 1 &&
            facts.named &&
            facts.primaryHeight >= 44,
          `handle reflow at ${width}px, ${theme}, root text ${scale}% ${JSON.stringify(facts)}`,
        );
        if (width === 1440 && scale === 100)
          await textContrast(page, theme, 'handle', [
            '#author-handle',
            '#handle-help',
            '.vk-handle-actions button',
            '.vk-author-info p',
          ]);
        if (
          (width === 1440 && theme === 'light' && scale === 100) ||
          (width === 320 && theme === 'dark' && scale === 200)
        )
          await page.capture(`handle-setup-${width}-${theme}-${scale}`, {
            scenario: 'handle-setup',
            textScale: scale,
            boundary: 'Chrome + native account + scratch PG',
          });
      }
  await page.eval(
    "document.documentElement.style.fontSize='';delete document.documentElement.dataset.theme",
  );
  await page.viewport(1280);
}

export async function authorHandles(page, { base, openPage, check, holdAnswer, textContrast }) {
  await page.waitFor(has('a[href="/toi/dia-chi"]'), 'owner entry');
  await page.click('a[href="/toi/dia-chi"]');
  await page.waitFor(has(field), 'suggested handle');
  const initial = (await own(page)).handle;
  await matrix(page, check, textContrast);
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  check(
    nodes.some((n) => n.role?.value === 'textbox' && n.name?.value === 'Tên trong liên kết'),
    'Chrome accessibility tree exposes a named handle field',
  );
  await page.eval(`(() => {
    const input=${has(field)}; input.focus();
    input.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}));
    input.form.requestSubmit();
  })()`);
  check(
    (await page.eval(`!${has(field)}.readOnly`)) && !(await own(page)).setup_complete,
    'an uncommitted IME composition cannot dispatch or confirm setup',
  );
  await page.eval(
    `${has(field)}.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}))`,
  );
  const fieldNode = await page.eval(`(window.handleFirst=${has(field)}, true)`);
  await fill(page, field, 'Chữ');
  await page.key('Tab');
  check(
    await page.eval(`document.activeElement===${has(submit)}`),
    'Tab goes to the primary setup action',
  );
  await page.key('Enter');
  await page.waitFor(
    `document.activeElement===window.handleFirst && window.handleFirst.getAttribute('aria-invalid')==='true'`,
    'field error restores focus',
  );
  check(
    fieldNode && (await page.eval('window.handleFirst.value==="Chữ"')),
    'Vietnamese input survives validation',
  );
  await fill(page, field, initial);
  await page.click(submit);
  await page.waitFor(has(saved), 'setup confirmed');
  check(
    (await own(page)).remaining_changes === 2,
    'keeping suggestion completes setup with both changes',
  );
  const tab = await openPage();
  await tab.goto(base + '/toi/dia-chi');
  await tab.waitFor(has(edit), 'second owner tab');
  await tab.click(edit);
  await fill(tab, field, 'mua-ngo');
  await page.click(edit);
  await fill(page, field, 'dem-mua');
  await page.click(submit);
  await page.waitFor(has('dialog.vk-handle-dialog[open]'), 'confirmation');
  await page.key('Escape');
  await page.waitFor(`document.activeElement===${has(submit)}`, 'Escape restores trigger');
  await page.click(submit);
  const held = await holdAnswer(page, 'POST', '/api/v1/me/handle');
  await page.click(approve);
  check((await held.held()) === 200, 'rename committed before lost ACK');
  await held.finish(true);
  await page.waitFor(
    `${has('#dia-chi-tac-gia [role=status]')}?.textContent.includes('Chưa nhận')`,
    'uncertain response',
  );
  await page.reload();
  await page.waitFor(has('#dia-chi-tac-gia input[readonly]'), 'pending command survives reload');
  await page.click('#dia-chi-tac-gia form button');
  await page.waitFor(has(saved), 'read receipt and current state');
  check((await own(page)).remaining_changes === 1, 'readback spends exactly one change');
  await tab.click(submit);
  await tab.click(approve);
  await tab.waitFor(
    `${has('#handle-error')}?.textContent.includes('phiên khác')`,
    'stale tab reconciles current version and keeps input',
  );
  check(
    await tab.eval(`${has(field)}.value==='mua-ngo' && document.activeElement===${has(field)}`),
    'stale tab preserves input and focus',
  );
  const context = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
  const guest = await openPage(context.browserContextId);
  await openAccount(guest, base, 'anonymous');
  check(
    !(await guest.eval(`Boolean(${has('a[href="/toi/dia-chi"]')})`)),
    'guest has no handle command',
  );
  async function visit(alias, target) {
    await guest.send('Page.navigate', { url: base + '/@' + alias + '?next=elsewhere#doc' });
    await guest.waitFor(
      `location.pathname==='/@${target}' && location.search==='' && location.hash==='#doc' && ${has('.vk-author-public')}`,
      'direct canonical navigation, fragment preserved',
    );
  }
  await visit(initial, 'dem-mua');
  check(
    await guest.eval(
      `!${has(field)} && ${has('.vk-author-public')}?.textContent.includes('Chưa có tác phẩm công khai')`,
    ),
    'public zero-work profile without owner controls',
  );
  await tab.viewport(390);
  await tab.click(submit);
  await tab.waitFor(has('dialog.vk-handle-dialog[open]'), 'last rename confirmation');
  await tab.capture('handle-final-confirm-390', { scenario: 'handle-last-rename', textScale: 100 });
  await tab.click(approve);
  await tab.waitFor(has(saved), 'final rename');
  check(
    (await own(tab)).remaining_changes === 0 && !(await tab.eval(`Boolean(${has(edit)})`)),
    'exhaustion removes rename action',
  );
  await visit(initial, 'mua-ngo');
  await visit('dem-mua', 'mua-ngo');
  for (const method of ['GET', 'HEAD']) {
    for (const alias of [initial, 'dem-mua']) {
      const result = await fetch(`${base}/@${alias}?next=elsewhere`, {method, redirect: 'manual'});
      check(result.status === 301 && result.headers.get('location') === '/@mua-ngo' &&
        result.headers.get('cache-control')?.includes('no-store'), `${method} on ${alias} redirects directly to the canonical profile`);
    }
  }
  // A saved author link can have been built before the rename; activate it through the app router.
  await guest.goto(base + '/');
  await guest.waitFor("document.documentElement.dataset.hydrated === 'true'", 'alias source has hydrated');
  await guest.eval(`(() => {
    const saved = document.createElement('a');
    saved.id = 'scenario-saved-author';
    saved.setAttribute('data-profile-return', 'true');
    saved.href = '/@${initial}';
    saved.textContent = 'Địa chỉ tác giả đã lưu';
    document.querySelector('main').prepend(saved);
  })()`);
  await guest.click('#scenario-saved-author');
  await guest.waitFor(`location.pathname === '/@mua-ngo' &&
    document.activeElement?.id === 'author-profile-heading'`, 'in-app alias activation hands focus to canonical H1');
  const formerName = await guest.eval(`${has('#author-profile-heading')}.textContent`);
  await guest.capture('handle-public-guest', { scenario: 'handle-public', textScale: 100 });
  await guest.goto(base + '/toi');
  // Account orchestration belongs to #297. Fence the profile in this exact disposable
  // database only, then revisit the URLs in the SAME guest browser with its cache enabled.
  const retired = authorFixtureSql(
    "UPDATE author_profiles SET current_handle=NULL,version=version+1 WHERE current_handle='mua-ngo' RETURNING 1;",
  );
  check(retired === '1', 'retired exactly the scenario profile');
  await guest.eval('history.back()');
  await guest.waitFor(
    "document.querySelector('main h1')?.textContent==='Hồ sơ không còn trên Vink'",
    'Back after retirement rechecks the profile',
  );
  for (const name of [initial, 'dem-mua', 'mua-ngo']) {
    await guest.goto(base + '/@' + name);
    await guest.waitFor(
      "document.querySelector('main h1')?.textContent==='Hồ sơ không còn trên Vink'",
      'previously visited URL now gone',
    );
  }
  for (const method of ['GET', 'HEAD']) {
    const result = await fetch(`${base}/@${initial}`, {method, redirect:'manual'});
    check(result.status === 410 && !result.headers.has('location') &&
      result.headers.get('cache-control')?.includes('no-store'), `${method} on a retired alias returns 410 with no redirect`);
    const html = await result.text();
    check(!html.includes('mua-ngo') && !html.includes('dem-mua') && !html.includes(initial) && !html.includes(formerName) &&
      (method !== 'GET' || (html.includes('Hồ sơ không còn trên Vink') && !html.includes('rel="canonical"'))),
      `${method} on a retired alias omits old byline links, canonical and handles`);
  }
  await guest.send('Emulation.setScriptExecutionDisabled', {value:true});
  try {
    await guest.goto(`${base}/@${initial}`);
    check(await guest.eval(`${has('#profile-unavailable-explore')}?.getAttribute('href') === '/' &&
      !document.documentElement.dataset.hydrated`), '410 offers a real Khám phá link with JavaScript disabled');
    await guest.viewport(390);
    await guest.capture('profile-gone-390-light-100', {scenario: 'profile-gone', boundary: 'real SSR; scratch profile retired; JS disabled'});
  } finally {
    await guest.send('Emulation.setScriptExecutionDisabled', {value:false});
  }
  // The guest checks foregrounded another tab. Restore the owner before password captures.
  await page.send('Page.bringToFront');
  await openAccount(page, base, 'active');
  check(await page.eval(`document.visibilityState === 'visible'`), 'owner tab is visible after handle checks');
}
