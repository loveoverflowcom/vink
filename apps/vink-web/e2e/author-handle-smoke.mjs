// HANDLE-03/04. One real account, two owner tabs and an isolated guest in Chrome.
import { authorFixtureSql } from './author-fixtures.mjs';
import { fabricate, fill, openAccount } from './auth-smoke.mjs';

const field = '#author-handle';
const submit = '#author-handle-submit';
const edit = '#author-handle-open';
const dialog = '#author-handle-dialog';
const saved = '#handle-saved[data-setup="true"]';
const canonical = '/toi/chinh-ho-so#dia-chi-tac-gia';
const has = (s) => 'document.querySelector(' + JSON.stringify(s) + ')';
const enabled = (s) => has(s + ':not([aria-disabled="true"])');
const own = (page) => page.eval('fetch("/api/v1/me/handle").then(r=>r.json())');

async function matrix(page, check, textContrast) {
  for (const width of [320, 390, 834, 1024, 1440])
    for (const theme of ['light', 'dark'])
      for (const scale of [100, 150, 200]) {
        await page.viewport(width);
        const facts = await page.eval(`(async ()=>{
      const root=document.documentElement;
      root.dataset.theme='${theme}';root.style.fontSize='${scale}%';await document.fonts.ready;
      await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
      const targets=[...document.querySelectorAll("#author-handle-dialog input,#author-handle-dialog button,#author-handle-proposed")];
      return {width:root.clientWidth,scroll:root.scrollWidth,inside:targets.every(el=>{const r=el.getBoundingClientRect();return r.left>=0 && r.right<=root.clientWidth+1}),
        named:document.querySelector("#author-handle").labels.length===1,
        primaryHeight:document.querySelector("#author-handle-submit").getBoundingClientRect().height};
    })()`);
        check(
          facts.inside &&
            facts.scroll <= facts.width + 1 &&
            facts.named &&
            facts.primaryHeight >= 44,
          `handle reflow at ${width}px, ${theme}, root text ${scale}% ${JSON.stringify(facts)}`,
        );
        if (width === 1440 && scale === 100) {
          await page.waitFor(`!document.getAnimations().some(a => a.playState === 'running' &&
            a.effect.target?.checkVisibility() && Number.isFinite(a.effect.getComputedTiming().endTime))`,
            'Handle colors settled after changing theme');
          await textContrast(page, theme, 'handle', [
            '#author-handle',
            '#author-handle-help',
            '#author-handle-submit',
            '#author-handle-proposed',
          ]);
        }
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
  await page.waitFor(has('#edit-profile-link'), 'owner profile entry');
  check(await page.eval("!document.querySelector('a[href=\"/toi/dia-chi\"]')"), 'Tôi has one entry to edit identity');
  const unavailable = await holdAnswer(page, 'GET', '/api/v1/me/handle');
  await page.click('#edit-profile-link');
  check(await unavailable.held() === 200, 'the initial Handle read reached the real app');
  await unavailable.finish(true);
  await page.waitFor(has('#author-handle-refresh'), 'Handle read failure offers a local retry');
  const beforeProfile = await page.eval("fetch('/api/v1/me/profile').then(r => r.json())");
  const bio = 'Bản nháp Bio trong khi đổi địa chỉ';
  await fill(page, '#profile-bio', bio);
  await page.click('#author-handle-refresh');
  await page.waitFor(enabled(edit), 'retry loads the Handle row');
  await page.click(edit);
  await page.waitFor(has(dialog + '[open] ' + field), 'setup dialog with the suggested handle');
  const initial = (await own(page)).handle;
  await matrix(page, check, textContrast);
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  check(nodes.some(n => n.role?.value === 'textbox' && n.name?.value === 'Tên trong liên kết'),
    'Chrome accessibility tree exposes the named Handle field');
  await page.eval(`(() => {
    const input = ${has(field)};
    input.focus();
    input.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    input.form.requestSubmit();
  })()`);
  check(!(await own(page)).setup_complete, 'Enter during composition cannot confirm setup');
  await page.eval(`${has(field)}.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }))`);
  await page.eval(`window.handleFirst = ${has(field)}`);
  await fill(page, field, 'Chữ');
  await page.key('Enter');
  await page.waitFor(
    "document.activeElement === window.handleFirst && window.handleFirst.getAttribute('aria-invalid') === 'true' && window.handleFirst.value === 'Chữ'",
    'validation keeps input, focus and the same field',
  );
  await page.key('Escape');
  await page.waitFor(`!${has(dialog + '[open]')} && document.activeElement === ${has(edit)}`, 'Escape restores the row action');
  check(!(await own(page)).setup_complete &&
    await page.eval(`document.querySelector('#profile-bio').value === ${JSON.stringify(bio)}`), 'cancel leaves both drafts and quota alone');
  await page.click(edit);
  await page.waitFor(has(dialog + '[open] ' + field), 'reopened setup');
  await page.eval('history.back()');
  await page.waitFor(`location.pathname === '/toi/chinh-ho-so' && !${has(dialog + '[open]')}
    && document.activeElement === ${has(edit)}`, 'Back closes the dialog without leaving the form');
  await page.click(edit);
  await page.waitFor(has(dialog + '[open] ' + field), 'setup opens again after Back');
  await fill(page, field, initial);
  await page.click(submit);
  await page.waitFor(`${has(saved)} && !${has(dialog + '[open]')}`, 'suggestion accepted');
  const afterSetup = await own(page);
  await page.click('#own-handle-public-link');
  await page.waitFor(`location.pathname === ${JSON.stringify('/@' + initial)} && ${has('#author-profile-heading')}`,
    'open the public profile from the Handle row');
  await page.eval('history.back()');
  await page.waitFor(`location.pathname === '/toi/chinh-ho-so' && ${enabled(edit)}`,
    'return to the hydrated profile form');
  const unchangedProfile = await page.eval("fetch('/api/v1/me/profile').then(r => r.json())");
  check(afterSetup.remaining_changes === 2 && unchangedProfile.profile_version === beforeProfile.profile_version
    && await page.eval(`document.querySelector('#profile-bio').value === ${JSON.stringify(bio)}`),
    'setup and a visit to the public profile leave the draft, version and both turns intact');
  await page.click('#profile-save');
  await page.waitFor("document.querySelector('#profile-message')?.textContent === 'Đã lưu hồ sơ.'", 'separate profile save');
  check(await page.eval(`${has(saved)}?.textContent === ${JSON.stringify('@' + initial)}`) &&
    (await own(page)).version === afterSetup.version, 'profile save runs independently and keeps the confirmed address');

  await page.waitFor(enabled(edit), 'profile save revalidation settles');
  await page.click(edit);
  await page.waitFor(has(dialog + '[open]'), 'open an edit before the address becomes unavailable');
  await fill(page, field, 'can-doi-chieu');
  // Inject one definitive refusal. The real tombstone is terminal and covered at the end.
  const declined = await fabricate(page, 'POST', '/api/v1/me/handle', 404, { error: { code: 'not_found' } });
  await page.click(submit);
  await declined();
  await page.waitFor(`${has('#author-handle-dialog-status')}?.textContent.includes('Không tìm thấy')
    && !${has('#author-handle-receipt')}`, 'a missing address declines the write without entering receipt recovery');
  check(await page.eval(`!sessionStorage.getItem(${JSON.stringify('vink.handle.pending.' + afterSetup.principal_id)})`),
    'a definite 404 clears the stored command');
  await page.key('Escape');
  await page.waitFor(has('#author-handle-refresh'), 'the unavailable row stays recoverable');
  await page.click('#author-handle-refresh');
  await page.waitFor(enabled(edit), 'reload after a definitive refusal');

  const legacy = await fetch(base + '/toi/dia-chi', { redirect: 'manual' });
  check(legacy.status === 308 && legacy.headers.get('location') === canonical
    && legacy.headers.get('cache-control')?.includes('no-store'), 'old private URL redirects to the canonical row');
  const tab = await openPage();
  // goto waits for the exact source URL. Here the destination must change.
  await tab.send('Page.navigate', { url: base + '/toi/dia-chi' });
  await tab.waitFor(`location.pathname + location.hash === ${JSON.stringify(canonical)}
    && document.activeElement?.id === 'dia-chi-tac-gia' && ${enabled(edit)}`, 'legacy link focuses the owner row');
  await tab.click(edit);
  await tab.waitFor(has(dialog + '[open]'), 'second owner dialog');
  const refresh = await holdAnswer(tab, 'GET', '/api/v1/me/handle');
  await page.eval(`(() => {
    const channel = new BroadcastChannel('vink-profile-changes');
    channel.postMessage(null);
    channel.close();
  })()`);
  check(await refresh.held() === 200, 'a profile change starts a Handle refresh in the open dialog');
  await tab.waitFor(has(submit + '[aria-disabled="true"]'), 'submission waits for the Handle refresh');
  await fill(tab, field, 'mua-ngo');
  check(await tab.eval(`!${has(field)}.readOnly && ${has(field)}.value === 'mua-ngo'`),
    'the background read leaves the field editable');
  await refresh.finish(false, true);
  await page.click(edit);
  await page.waitFor(has(dialog + '[open]'), 'rename dialog');
  await fill(page, field, initial);
  await page.click(submit);
  await page.waitFor(`${has('#author-handle-error')}?.textContent.includes('địa chỉ hiện tại')`, 'no-op does not start a rename');
  check((await own(page)).remaining_changes === 2, 'no-op keeps both turns');
  await fill(page, field, 'admin');
  await page.click(submit);
  await page.waitFor(`${has('#author-handle-error')}?.textContent.includes('dành riêng') && ${enabled(submit)}`,
    'reserved name is rejected with a useful explanation');
  check((await own(page)).remaining_changes === 2, 'reserved name keeps both turns');
  await fill(page, field, 'dem-mua');
  const held = await holdAnswer(page, 'POST', '/api/v1/me/handle');
  await page.click(submit);
  check((await held.held()) === 200, 'rename committed before lost ACK');
  await held.finish(true);
  await page.waitFor(`${has('#author-handle-dialog-status')}?.textContent.includes('Chưa nhận')`, 'uncertain response');
  await page.reload();
  await page.waitFor(`${enabled(edit)}?.textContent.includes('Kiểm tra kết quả')`,
    'pending command survives reload');
  await page.click(edit);
  await page.waitFor(has(dialog + '[open] input[readonly]'), 'pending command keeps its original payload');
  await page.click('#author-handle-receipt');
  await page.waitFor(`${has(saved)}?.textContent === '@dem-mua' && !${has(dialog + '[open]')}`, 'receipt and current state settle');
  check((await own(page)).remaining_changes === 1, 'receipt recovery spends exactly one change');
  await tab.waitFor(enabled(submit), 'other tab can attempt the stale command');
  await tab.click(submit);
  await tab.waitFor(`${has('#author-handle-error')}?.textContent.includes('phiên khác')`,
    'stale tab reconciles the current version');
  check(await tab.eval(`${has(field)}.value === 'mua-ngo' && document.activeElement === ${has(field)}`),
    'stale tab preserves input and focus');
  const context = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
  const guest = await openPage(context.browserContextId);
  await openAccount(guest, base, 'anonymous');
  await guest.send('Page.navigate', { url: base + '/toi/dia-chi' });
  await guest.waitFor(`location.pathname + location.hash === ${JSON.stringify(canonical)}
    && ${has('.vk-profile-edit a[href="/toi"]')}?.textContent.includes('Mở Tài khoản') && !${has(edit)} && !${has(field)}`,
    'guest redirect exposes no own Handle controls');
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
  await fill(tab, field, initial);
  await tab.click(submit);
  await tab.waitFor(`${has('#author-handle-error')}?.textContent.includes('địa chỉ cũ')`, 'cannot rename back to an own alias');
  check((await own(tab)).remaining_changes === 1, 'alias rejection keeps the last turn');
  await fill(tab, field, 'mua-ngo');
  await tab.viewport(390);
  await tab.waitFor(has(dialog + '[open]'), 'last rename confirmation');
  await tab.capture('handle-final-confirm-390', { scenario: 'handle-last-rename', textScale: 100 });
  await tab.click(submit);
  await tab.waitFor(`${has(saved)}?.textContent === '@mua-ngo' && !${has(dialog + '[open]')}
    && document.activeElement?.id === 'dia-chi-tac-gia'`, 'final rename restores focus to the row');
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
  // A saved author link can have been built before the rename. Hold the real Alias DTO
  // beyond the old 10s deadline, then abandon the same request in a separate navigation.
  async function openAliasSource() {
    await guest.goto(base + '/');
    await guest.waitFor("document.documentElement.dataset.hydrated === 'true'", 'alias source has hydrated');
    await guest.eval(`(() => {
      const saved = document.createElement('a');
      saved.id = 'scenario-saved-author';
      saved.setAttribute('data-profile-return', 'true');
      saved.href = '/@${initial}';
      saved.textContent = 'Địa chỉ tác giả đã lưu';
      const next = document.createElement('a');
      next.id = 'scenario-next-destination';
      next.href = '/toi';
      next.textContent = 'Tôi';
      document.querySelector('main').prepend(saved, next);
    })()`);
  }
  const aliasApi = '/api/v1/authors/' + initial;
  await openAliasSource();
  const slowAlias = await holdAnswer(guest, 'GET', aliasApi);
  await guest.click('#scenario-saved-author');
  check(await slowAlias.held() === 200, 'real Alias DTO held before delivery');
  await new Promise(resolve => setTimeout(resolve, 11000));
  await slowAlias.finish(false, true);
  await guest.waitFor(`location.pathname === '/@mua-ngo' &&
    document.activeElement?.id === 'author-profile-heading'`,
    'slow alias activation hands focus to canonical H1 after 11 seconds');
  console.log('  ok  slow alias preserves canonical H1 focus after an 11 second wait');

  await openAliasSource();
  const abandonedAlias = await holdAnswer(guest, 'GET', aliasApi);
  await guest.click('#scenario-saved-author');
  check(await abandonedAlias.held() === 200, 'second Alias DTO held before choosing another destination');
  await guest.click('#scenario-next-destination');
  await guest.waitFor("location.pathname === '/toi' && document.querySelector('#me-title')", 'new destination commits first');
  await abandonedAlias.finish(false, true);
  check(await guest.eval("location.pathname === '/toi' && !sessionStorage.getItem('vinkProfileAliasFocus')"),
    'abandoned alias cannot replace the newer destination or queue stale focus');
  await guest.goto(base + '/@mua-ngo');
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
    await guest.eval(`${has('#author-profile-heading')}.focus()`);
    await guest.key('Tab', 0, {settle:false});
    await guest.waitFor("document.activeElement?.id === 'profile-unavailable-explore'", '410 Tab reaches the recovery link');
    await guest.capture('profile-gone-390-light-100', {scenario: 'profile-gone', textScale: 100, boundary: 'real SSR; scratch profile retired; JS disabled'});
    await guest.key('Enter', 0, {settle:false});
    await guest.waitFor("location.pathname === '/'", '410 recovery link opens Khám phá by keyboard');
  } finally {
    await guest.send('Emulation.setScriptExecutionDisabled', {value:false});
  }
  // The guest checks foregrounded another tab. Restore the owner before password captures.
  await page.send('Page.bringToFront');
  await openAccount(page, base, 'active');
  check(await page.eval(`document.visibilityState === 'visible'`), 'owner tab is visible after handle checks');
}
