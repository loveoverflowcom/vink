// #388 Reader “Thấm · Lưu”: real Chrome, real App API, native Identity and scratch PostgreSQL.
// Faults hold or drop a real answer; no success is fabricated. Geometry rows that need a long
// count replace only the count text in the DOM (`dom-override-geometry-only`).
import { HYDRATED, fill, q } from './auth-smoke.mjs';
import { captureRequest, holdAnswer } from './auth-browser-network.mjs';

const WORK = '33333333-3333-4333-8333-333333333333';
const HERE = `/tac-pham/${WORK}`;
const STATE = `/api/v1/me/saved-works/${WORK}`;
const SAVE = '#save-work';
const DIALOG = '#reader-auth';
const frames = 'new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))';
// A theme switch transitions colours; captures wait until every running transition has ended.
const settledPaint = `${frames}.then(() => Promise.all(document.getAnimations().map(a => a.finished.catch(() => null))))`;
const active = (id) => `document.activeElement?.id === ${JSON.stringify(id)}`;
const pressed = (value) => `${q(SAVE)}?.getAttribute('aria-pressed') === ${JSON.stringify(value)}`;
const settled = `${q(SAVE)} && ${q(SAVE)}.getAttribute('aria-busy') !== 'true'`;
const feedback = `${q(SAVE)}?.closest('.vk-ui-toggle')?.querySelector('.vk-ui-toggle__feedback')`;
const says = (text) => `(${feedback}?.textContent ?? '').includes(${JSON.stringify(text)})`;
const standing = (state) => `${q(DIALOG)}?.dataset.standing === ${JSON.stringify(state)}`;
const isOpen = `${q(DIALOG)}?.open === true`;

const readState = (page) => page.eval(`fetch('${STATE}', { cache: 'no-store' }).then(async r => ({
  status: r.status, cache: r.headers.get('cache-control'), body: r.ok ? await r.json() : null
}))`);

/** Watches PUT/DELETE of this pair and the expected generation each one carried. */
function writes(page) {
  const seen = [];
  const listener = (event) => {
    if (event.sessionId !== page.sessionId || event.method !== 'Network.requestWillBeSent') return;
    const { request } = event.params;
    const url = new URL(request.url);
    if (url.pathname === STATE && ['PUT', 'DELETE'].includes(request.method)) {
      seen.push({ method: request.method, generation: Number(url.searchParams.get('expected_generation')),
        principal: request.headers['X-Vink-Expected-Principal'] ?? request.headers['x-vink-expected-principal'] });
    }
  };
  page.cdp.listeners.push(listener);
  return { seen, close: () => { page.cdp.listeners = page.cdp.listeners.filter((item) => item !== listener); } };
}

async function openReader(page, base, state) {
  await page.goto(base + HERE);
  await page.waitFor(`document.querySelector('.vk-document') && ${settled} && ${state}`, `Reader save control: ${state}`);
}

/** Pair geometry and overflow at the current viewport/theme/text size. */
async function pairFacts(page) {
  return page.eval(`(() => {
    const pair = ${q('#sv-pair')}, host = pair.parentElement;
    const [a, b] = [${q('#endorse-work')}, ${q(SAVE)}].map(el => el.getBoundingClientRect());
    const style = getComputedStyle(${q(SAVE)});
    const label = ${q('#endorse-work .vk-ui-toggle__label')}.getBoundingClientRect();
    const count = ${q('#endorse-work .vk-ui-toggle__count')}?.getBoundingClientRect();
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
    // The pair and everything in it stay inside the viewport and inside their host.
    const hostBox = host.getBoundingClientRect();
    const inside = [...pair.querySelectorAll('*')].filter(n => n.getClientRects().length && !n.closest('.vk-ui-visually-hidden'))
      .every(n => { const r = n.getBoundingClientRect(); return r.left >= -0.5 && r.right <= Math.min(innerWidth, hostBox.right) + 0.5; });
    return {
      stacked: Math.abs(a.left - b.left) < 1, hostRem: host.getBoundingClientRect().width / rem,
      heights: [a.height, b.height], widths: [a.width, b.width], gap: Math.abs(a.left - b.left) < 1 ? b.top - a.bottom : b.left - a.right,
      pairWidth: pair.getBoundingClientRect().width, rem,
      oneLine: !count || Math.abs(label.top - count.top) < 2,
      overflow: !inside || host.scrollWidth > host.clientWidth + 0.5,
      pageOverflow: document.scrollingElement.scrollWidth > innerWidth + 0.5,
      transition: style.transitionDuration, radius: style.borderRadius,
      roles: [pair.getAttribute('role'), pair.getAttribute('aria-label')],
    };
  })()`);
}

async function geometryMatrix(page, check, base) {
  const rows = [];
  const fail = [];
  const pageOverflow = [];
  for (const [width, height] of [[320, 800], [390, 844], [1024, 900]]) {
    for (const theme of ['light', 'dark']) {
      for (const scale of ['100', '150', '200', '300']) {
        await page.viewport(width, height);
        await page.eval(`document.documentElement.dataset.theme = ${JSON.stringify(theme)};
          document.documentElement.style.fontSize = ${JSON.stringify(scale === '100' ? '' : scale + '%')};`);
        await page.eval(`${q('#sv-pair')}.scrollIntoView({ block: 'center', behavior: 'instant' })`);
        await page.eval(frames);
        const f = await pairFacts(page);
        const id = `${width}-${theme}-${scale}`;
        rows.push(id);
        if (f.pageOverflow) pageOverflow.push(id);
        // D5 keeps “Thấm + số” on one line up to 200%; at 150% app × 200% browser it may wrap.
        const extreme = scale === '300';
        const equal = (extreme || Math.abs(f.heights[0] - f.heights[1]) <= 0.5) && Math.abs(f.widths[0] - f.widths[1]) <= 0.5;
        const base100 = scale !== '100' || (Math.abs(f.heights[0] - 56) <= 0.5 && f.pairWidth <= 400.5 && Math.abs(f.gap - 2) <= 0.5);
        const stackRule = f.stacked === (f.hostRem < 20);
        if (!(equal && base100 && stackRule && !f.overflow && (extreme || f.oneLine))) fail.push(`${id}: ${JSON.stringify(f)}`);
        if (['390-light-100', '390-dark-100', '320-dark-200', '1024-light-100', '320-light-300'].includes(id)) {
          await page.eval(settledPaint);
          await page.capture(`reader-save-${id}`, { boundary: 'real-reader-save-pair', requiredTargets: ['#sv-pair'] });
        }
      }
    }
  }
  if (pageOverflow.length) console.log(`  ..  page-level horizontal overflow outside the pair (not this AC): ${pageOverflow.join(', ')}`);
  check(fail.length === 0, `#388 pair: equal halves, 56px/≤400px/2px gap at 100%, stacks exactly when its host is below 20rem, no horizontal overflow, Thấm + count on one line across ${rows.length} viewport/theme/text rows${fail.length ? ' — ' + fail.join(' | ') : ''}`);
  // Long counts: only the count text changes, to measure the reflow of the real component.
  const longFail = [];
  for (const digits of ['12.480', '1.234.567']) {
    for (const [width, scale] of [[320, '100'], [390, '100'], [1024, '100'], [320, '200'], [390, '200']]) {
      await page.viewport(width, 844);
      await page.eval(`document.documentElement.dataset.theme = 'light';
        document.documentElement.style.fontSize = ${JSON.stringify(scale === '100' ? '' : scale + '%')};
        { const shown = ${q('#endorse-work .vk-ui-toggle__count')}?.firstElementChild; if (shown) shown.textContent = ${JSON.stringify(digits)}; }`);
      if (!await page.eval(`${q('#endorse-work .vk-ui-toggle__count')}?.firstElementChild?.textContent === ${JSON.stringify(digits)}`)) {
        longFail.push(`${digits}: no visible Thấm count to measure`);
        continue;
      }
      await page.eval(frames);
      const f = await pairFacts(page);
      const allowedWrap = digits.length > 7 && scale === '200';
      if (f.overflow || (!f.oneLine && !allowedWrap)) longFail.push(`${digits}@${width}/${scale}: ${JSON.stringify(f)}`);
    }
  }
  check(longFail.length === 0, `#388 counts of 4 and 7 digits fit without overflow; one line except 7 digits at 200% (dom-override-geometry-only)${longFail.length ? ' — ' + longFail.join(' | ') : ''}`);
  await page.eval(`document.documentElement.style.fontSize = ''; document.documentElement.dataset.theme = 'light';`);
  await page.viewport(1280, 900);
  // Reload restores the real count after the DOM override.
  await openReader(page, base, `${q(SAVE)}?.hasAttribute('aria-pressed')`);
}

/**
 * CSR standalone journey. Starts as a guest; `reader` and `other` are two registered accounts
 * of the scenario (username/password), neither the author of the sample work.
 */
export async function readerSave(page, { args, check, openPage, reader, other }) {
  console.log('Reader save (#388): guest sheet, ACK-only save/unsave, lost ACK, two tabs, session change, geometry.');
  const base = args.base;
  const sent = writes(page);
  const errorsBefore = page.errors.length;
  try {
    await page.viewport(1280, 900);
    await openReader(page, base, `${standing('anonymous')} && ${pressed('false')}`);
    const facts = await page.eval(`({
      saves: document.querySelectorAll('[id="save-work"], [data-kind="save"] button').length,
      groups: document.querySelectorAll('#work-endorsement [role=group]').length,
      order: [...${q('#sv-pair')}.querySelectorAll('button.vk-ui-toggle__control')].map(b => b.id),
      name: ${q(SAVE)}.getAttribute('aria-label'), icon: ${q(SAVE)}.querySelector('[data-icon]')?.dataset.icon,
      heart: !!${q('#sv-pair')}.querySelector('[data-icon^=heart]'),
    })`);
    check(facts.saves === 1 && facts.groups === 1 && facts.order.join() === 'endorse-work,save-work' && facts.name === 'Lưu' &&
      facts.icon === 'bookmark-border' && !facts.heart,
      `#388 D1/SAV-04: exactly one Thấm · Lưu group at the end of the Reader, outline bookmark, no heart (${JSON.stringify(facts)})`);

    // D2: the guest sheet, keyboard only, never a save on the guest's behalf.
    await page.eval(`${q(SAVE)}.focus()`);
    await page.key('Enter');
    await page.waitFor(`${isOpen} && ${active('reader-auth-close')}`, 'guest Lưu opens the Reader sheet at Close');
    const sheet = await page.eval(`(() => {
      const d = ${q(DIALOG)};
      return { modal: d.matches(':modal'), heading: document.getElementById(d.getAttribute('aria-labelledby'))?.textContent.trim(),
        lead: ${q('#reader-auth-lead')}?.textContent.trim(), described: d.getAttribute('aria-describedby'),
        alternatives: [...d.querySelectorAll('#use-registration, #use-email-code')].map(b => {
          const s = getComputedStyle(b); return [b.textContent.trim(), s.borderTopWidth, Math.round(b.getBoundingClientRect().width)];
        }) };
    })()`);
    check(sheet.modal && sheet.heading === 'Đăng nhập để lưu bài' &&
      sheet.lead === 'Bài sẽ nằm trong danh sách Đã lưu riêng trên hồ sơ của bạn.' && sheet.described === 'reader-auth-lead' &&
      sheet.alternatives.length === 2 && sheet.alternatives.every(([, border]) => border === '2px') &&
      Math.abs(sheet.alternatives[0][2] - sheet.alternatives[1][2]) <= 1,
      `#388 D2: “Đăng nhập để lưu bài” sheet with its lead and two equal 2px outline alternatives (${JSON.stringify(sheet)})`);
    await page.capture('reader-save-guest-sheet-1280-light', { boundary: 'real-reader-save-guest-sheet' });
    await page.key('Escape');
    await page.waitFor(`!${isOpen} && ${active('save-work')}`, 'Escape returns focus to Lưu');
    await page.key('Space');
    await page.waitFor(`${isOpen} && ${active('reader-auth-close')}`, 'Space on Lưu opens the sheet again');
    await fill(page, '#password-login-username', reader.username);
    await fill(page, '#password-login-password', reader.password);
    await page.click('#password-login-submit');
    await page.waitFor(`!${isOpen} && ${active('save-work')} && ${settled} && ${pressed('false')} &&
      ${says('Bạn đã đăng nhập. Bấm Lưu để lưu bài này.')}`, 'signed in from the sheet: back on Lưu, not saved, guidance line');
    let server = await readState(page);
    check(sent.seen.length === 0 && server.status === 200 && server.body.saved === false && server.cache === 'private, no-store',
      `#388 D2: signing in from the sheet sends no save; the private pair stays unsaved (${JSON.stringify(server)})`);

    // D11: pending holds the confirmed look until the real ACK.
    const put = await holdAnswer(page, 'PUT', STATE);
    await page.eval(`${q(SAVE)}.focus()`);
    await page.key('Enter');
    check(await put.held() === 200, '#388 holds the real successful PUT before delivery');
    const pending = await page.eval(`(() => {
      const b = ${q(SAVE)};
      return { pressed: b.getAttribute('aria-pressed'), busy: b.getAttribute('aria-busy'), focus: document.activeElement === b,
        icon: b.querySelector('.vk-ui-toggle__glyph [data-icon]')?.dataset.icon, arc: !!b.querySelector('.vk-ui-arc'),
        note: ${feedback}.textContent.trim() };
    })()`);
    check(pending.pressed === 'false' && pending.busy === 'true' && pending.focus && pending.icon === 'bookmark-border' &&
      pending.arc && pending.note.includes('Đang lưu…'),
      `#388 D11: pending keeps aria-pressed=false, the outline glyph and focus, shows the arc and “Đang lưu…” (${JSON.stringify(pending)})`);
    await page.key('Enter');
    await put.finish(false, true);
    await page.waitFor(`${pressed('true')} && ${says('Đã lưu vào hồ sơ của bạn.')} && ${feedback}.querySelector('a') &&
      getComputedStyle(${q(SAVE)}).borderTopLeftRadius === '28px'`, 'ACK confirms the save; the pill finishes its shape change');
    const confirmed = await page.eval(`(() => {
      const b = ${q(SAVE)}, s = getComputedStyle(b);
      const link = ${feedback}.querySelector('a');
      return { icon: b.querySelector('.vk-ui-toggle__glyph [data-icon]')?.dataset.icon, check: !!b.querySelector('.vk-ui-toggle__mark [data-icon=check]'),
        name: b.getAttribute('aria-label'), radius: [s.borderTopLeftRadius, s.borderTopRightRadius, s.borderBottomRightRadius, s.borderBottomLeftRadius],
        link: link && [link.textContent.trim(), link.getAttribute('href'), Math.round(link.getBoundingClientRect().height)],
        live: ${feedback}.getAttribute('aria-live'), focus: document.activeElement === b };
    })()`);
    check(sent.seen.length === 1 && sent.seen[0].method === 'PUT' && sent.seen[0].generation === server.body.generation &&
      /^[0-9a-f-]{36}$/.test(sent.seen[0].principal ?? ''),
      `#388 one PUT with the confirmed generation and the acting principal, even after a second Enter while pending (${JSON.stringify(sent.seen)})`);
    check(confirmed.icon === 'bookmark' && confirmed.check && confirmed.name === 'Lưu' && confirmed.radius.every(r => r === '28px') &&
      confirmed.live === 'polite' && confirmed.focus && confirmed.link?.[0] === 'Mở Đã lưu' &&
      /^\/@[^/]+\/da-luu$/.test(confirmed.link?.[1] ?? '') && confirmed.link?.[2] >= 44,
      `#388 D3: filled bookmark, check, full pill, stable name “Lưu”, polite note with “Mở Đã lưu” ≥44px (${JSON.stringify(confirmed)})`);

    await page.reload();
    await page.waitFor(`${settled} && ${pressed('true')}`, 'reload keeps the saved state');
    server = await readState(page);
    check(server.body?.saved === true, '#388 saved state survives a full reload (server and Reader agree)');
    // SAV-INV-02: nothing private in the public reads, the document or browser storage.
    const leak = await page.eval(`(async () => {
      const own = await fetch('/api/v1/works/${WORK}/reader').then(r => r.text());
      const doc = await fetch('${HERE}', { headers: { accept: 'text/html' } }).then(r => r.text());
      const stored = JSON.stringify([Object.entries(localStorage), Object.entries(sessionStorage), history.state]);
      const cached = [];
      if ('caches' in window) {
        for (const name of await caches.keys()) {
          for (const request of await (await caches.open(name)).keys()) cached.push(request.url);
        }
      }
      const dbs = indexedDB.databases ? (await indexedDB.databases()).map(d => d.name) : [];
      return { own, doc, stored, cached, dbs };
    })()`);
    const guestReader = await fetch(`${base}/api/v1/works/${WORK}/reader`).then(r => r.text());
    const guestDoc = await fetch(`${base}${HERE}`, { headers: { accept: 'text/html' } }).then(r => r.text());
    check(leak.own === guestReader && leak.doc === guestDoc && !/saved|da-luu/i.test(leak.own) &&
      !/saved-works|"saved"|da-luu/.test(leak.stored) && !leak.cached.some(url => url.includes('saved-works')),
      `#388 SAV-INV-02: the saver's public Reader DTO and document equal the guest's; no saved state in storage or caches (dbs: ${JSON.stringify(leak.dbs)}, cached: ${leak.cached.length})`);

    // Unsave, then reload.
    await page.click(SAVE);
    await page.waitFor(`${pressed('false')} && ${says('Đã bỏ lưu.')}`, 'unsave ACK');
    await page.reload();
    await page.waitFor(`${settled} && ${pressed('false')}`, 'reload keeps the unsaved state');
    check((await readState(page)).body?.saved === false, '#388 unsave survives a full reload');

    // Lost ACK: a real committed PUT whose answer never reaches the page.
    const lost = await holdAnswer(page, 'PUT', STATE);
    await page.click(SAVE);
    check(await lost.held() === 200, '#388 the lost-ACK PUT was committed by the server');
    await lost.finish(true, true);
    await page.waitFor(`${says('Chưa xác nhận được lượt lưu.')} && ${pressed('false')} && ${q(SAVE)}.getAttribute('aria-disabled') === 'true'`,
      'lost ACK keeps the known state and blocks presses');
    const reconcile = `[...${feedback}.querySelectorAll('button')].find(b => b.textContent.trim() === 'Kiểm tra lại')`;
    const beforeRecover = sent.seen.length;
    await page.click(SAVE); // ignored while uncertain
    check(sent.seen.length === beforeRecover && await page.eval(`!!${reconcile}`), '#388 uncertain: no blind replay, “Kiểm tra lại” offered');
    await page.eval(`${reconcile}.click()`);
    await page.waitFor(`${pressed('true')} && ${says('Đã lưu vào hồ sơ của bạn.')} && ${active('save-work')}`,
      '“Kiểm tra lại” reads the committed state and returns focus to Lưu');
    check(sent.seen.length === beforeRecover, '#388 reconcile is a read, not a second PUT');

    // Two tabs of one account: a stale tab cannot undo a newer state. A fresh mount keeps
    // tab 1 inside its 15 s wake window, so only the press can learn the newer state.
    await page.reload();
    await page.waitFor(`${settled} && ${pressed('true')}`, 'tab 1 shows Saved before tab 2 opens');
    const second = await openPage();
    try {
      await openReader(second, base, pressed('true'));
      await second.click(SAVE);
      await second.waitFor(`${pressed('false')} && ${says('Đã bỏ lưu.')}`, 'tab 2 unsaves');
      await second.click(SAVE);
      await second.waitFor(`${pressed('true')} && ${says('Đã lưu vào hồ sơ của bạn.')}`, 'tab 2 saves again');
      const newest = (await readState(second)).body;
      const staleFrom = sent.seen.length;
      // Tab 1 still shows the older Saved; it never got the focus that would refresh it.
      await page.eval(`${q(SAVE)}.click()`);
      await page.waitFor(`${settled} && ${pressed('true')} && ${says('Chưa bỏ lưu được. Bạn có thể thử lại.')}`, 'stale tab refused, shows the newer state');
      const stale = sent.seen[staleFrom];
      const after = (await readState(page)).body;
      check(stale?.method === 'DELETE' && stale.generation < newest.generation && after.saved === true && after.generation === newest.generation,
        `#388 SAV-INV-04: the stale tab's DELETE(${stale?.generation}) cannot reverse tab 2's newer save (${JSON.stringify({ newest, after })})`);
    } finally {
      await second.send('Page.close').catch(() => {});
    }

    // A → sign-out: a held answer of A never shows to the guest.
    const held = await holdAnswer(page, 'GET', STATE);
    await page.reload();
    await held.held();
    const revoked = await page.eval(`fetch('/api/v1/auth/session', { method: 'DELETE' }).then(r => r.status)`);
    await page.eval(`(() => { const c = new BroadcastChannel('vink-session'); c.postMessage('changed'); c.close(); })()`);
    await page.waitFor(standing('anonymous'), 'session ended in this tab');
    await held.finish(false, true);
    await page.eval(frames);
    const guest = await page.eval(`({ pressed: ${q(SAVE)}.getAttribute('aria-pressed'), note: ${feedback}.textContent.trim(), state: ${q(SAVE)}.dataset.state })`);
    check(revoked === 204 && guest.pressed === 'false' && guest.state === 'enabled' && !guest.note.includes('Đã lưu'),
      `#388 A → sign-out: A's late saved state is dropped; the guest sees an unsaved Lưu (${JSON.stringify(guest)})`);

    // → B: another account signs in from the same sheet and sees only its own pair.
    await page.eval(`${q(SAVE)}.focus()`);
    await page.key('Enter');
    await page.waitFor(`${isOpen} && ${active('reader-auth-close')}`, 'sheet for account B');
    await fill(page, '#password-login-username', other.username);
    await fill(page, '#password-login-password', other.password);
    await page.click('#password-login-submit');
    await page.waitFor(`!${isOpen} && ${active('save-work')} && ${settled} && ${pressed('false')}`, 'B returns to Lưu with B\'s own unsaved pair');
    const b = await readState(page);
    check(b.body?.saved === false && !await page.eval(says('Đã lưu vào hồ sơ')),
      `#388 A → B: B never sees A's saved state (${JSON.stringify(b)})`);

    // Thấm keeps its own state and feedback beside Lưu (no regression of the shared control).
    const endorse = await page.eval(`(() => {
      const t = ${q('#endorse-work')};
      return { name: t.getAttribute('aria-label'), pressedAttr: t.hasAttribute('aria-pressed'), described: t.getAttribute('aria-describedby') };
    })()`);
    check(['Thấm', 'Bỏ Thấm'].includes(endorse.name) && !endorse.pressedAttr,
      `#388 Thấm keeps its action-named contract in the pair (${JSON.stringify(endorse)})`);

    // Reduced motion: the shape change is immediate.
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    // Tokens drop to 0ms; the Web's global fallback pins every transition to 0.01ms.
    const seconds = (d) => d.trim().endsWith('ms') ? parseFloat(d) / 1000 : parseFloat(d);
    const durations = (await pairFacts(page)).transition;
    const reduced = durations.split(',').every(d => seconds(d) <= 0.00001 + 1e-9);
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
    check(reduced, `#388 reduced motion: the pill changes without a visible transition (${durations})`);
    await page.click(SAVE);
    await page.waitFor(`${pressed('true')}`, 'B saves for the dark capture');
    await page.viewport(390, 844);
    await page.eval(`document.documentElement.dataset.theme = 'dark'; ${q('#sv-pair')}.scrollIntoView({ block: 'center', behavior: 'instant' })`);
    await page.eval(settledPaint);
    await page.capture('reader-save-390-dark-saved', { boundary: 'real-reader-save-saved-dark', requiredTargets: ['#sv-pair'] });
    await geometryMatrix(page, check, base);
    const errors = page.errors.slice(errorsBefore);
    check(errors.length === 0, `#388 no console exceptions during the save journey (${errors.slice(0, 3).join(' | ')})`);
  } finally {
    sent.close();
  }
}

/**
 * Pre-cutover SSR/hydration: the server HTML of one work is the same for a guest and for a
 * signed-in saver; the saved state appears only after hydration. `page` is signed in.
 */
export async function readerSaveSsr(page, { args, check }) {
  const base = args.base;
  await page.goto(base + HERE);
  await page.waitFor(`${HYDRATED} && ${settled} && ${q(SAVE)}?.hasAttribute('aria-pressed')`, 'hydrated Reader with the private pair read');
  if (await page.eval(pressed('false'))) {
    await page.click(SAVE);
    await page.waitFor(`${pressed('true')} && ${says('Đã lưu vào hồ sơ của bạn.')}`, 'SSR Reader saves after hydration');
  }
  const own = await page.eval(`fetch('${HERE}', { headers: { accept: 'text/html' } }).then(r => r.text())`);
  const guest = await fetch(base + HERE, { headers: { accept: 'text/html' } }).then(r => r.text());
  // The public part is the reading surface and the pair; the shell may know the session.
  const parts = await page.eval(`(() => {
    const pick = (html) => {
      const d = new DOMParser().parseFromString(html, 'text/html');
      return ['#reader-paper', '#work-endorsement', '#reader'].map(s => d.querySelector(s)?.outerHTML ?? null);
    };
    return { own: pick(${JSON.stringify(own)}), guest: pick(${JSON.stringify(guest)}) };
  })()`);
  const at = [...own].findIndex((c, i) => c !== guest[i]);
  if (at >= 0) {
    console.log(`  ..  whole SSR documents differ from offset ${at}; #reader equal: ${parts.own[2] === parts.guest[2]}`);
    console.log(`  ..  saver: ${JSON.stringify(own.slice(Math.max(0, at - 80), at + 80))}`);
    console.log(`  ..  guest: ${JSON.stringify(guest.slice(Math.max(0, at - 80), at + 80))}`);
  }
  const saveSsr = (parts.own[1]?.match(/<button[^>]*id="save-work"[^>]*>/) ?? [''])[0];
  check(parts.own[0] && parts.own[1] && parts.own[0] === parts.guest[0] && parts.own[1] === parts.guest[1] &&
    saveSsr.includes('data-pressed="unknown"') && !saveSsr.includes('aria-pressed') && !/saved-works|da-luu|Đã lưu vào/.test(own),
    `#388 SSR: the saver's server HTML equals the guest's in the Reader text and the Thấm · Lưu pair; Lưu is held without aria-pressed and nothing private is in the document (${saveSsr})`);
  await page.reload();
  await page.waitFor(`${HYDRATED} && ${settled} && ${pressed('true')}`, 'SSR reload shows the saved state after hydration');
  await page.click(SAVE);
  await page.waitFor(`${pressed('false')} && ${says('Đã bỏ lưu.')}`, 'SSR Reader unsaves');
  check((await readState(page)).body?.saved === false, '#388 SSR/hydration: save and unsave reach the private API');
}

export { captureRequest };
