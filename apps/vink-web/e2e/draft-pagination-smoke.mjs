// #201. The corpus and paging checks use real HTTP/PostgreSQL. DraftGate is used only
// for explicit delayed/error transport cases. No private text is persisted by the UI.
export async function draftPagination(page, { args, check, DraftGate, sleep }) {
  console.log('private draft pagination: real corpus, bounded pages, search and delayed intents:');
  const corpus = [];
  for (let i = 0; i < 250; i++) {
    const created = await fetch(args.base + '/api/v1/drafts', { method: 'POST', headers: { Origin: args.base } });
    if (created.status !== 201) throw new Error('paging corpus create: ' + created.status);
    const draft = await created.json();
    const title = i === 0 ? '[Paging201] ĐƯỜNG   VỀ xa' : `[Paging201] Trang ${String(i).padStart(3, '0')}`;
    const saved = await fetch(args.base + '/api/v1/drafts/' + draft.work_id, {
      method: 'PUT', headers: { Origin: args.base, 'Content-Type': 'application/json' },
      body: JSON.stringify({ expected_version: draft.version, title, document: { schema_version: 1, blocks: [{ lines: ['PRIVATE-201-BODY'] }] } }),
    });
    if (saved.status !== 200) throw new Error('paging corpus save: ' + saved.status);
    corpus.push({ work_id: draft.work_id, title });
  }
  const legacy = await (await fetch(args.base + '/api/v1/drafts')).json();
  check(legacy.drafts.length === 100 && !legacy.drafts.some(d => d.work_id === corpus[0].work_id), 'real oldest draft lies outside the legacy 100');
  const realQuery = await fetch(args.base + '/api/v1/drafts?q=duong%20ve%20xa&sort=title&limit=24');
  const found = await realQuery.json();
  check(found.drafts.length === 1 && found.drafts[0].work_id === corpus[0].work_id && realQuery.headers.get('cache-control') === 'no-store' && !JSON.stringify(found).includes('PRIVATE-201-BODY'), 'real provider searches the full corpus and returns only private/no-store summaries');

  const rows = () => page.eval(`[...document.querySelectorAll('.vk-draft-row__link')].map(a => a.getAttribute('href').split('/').at(-1))`);
  const settled = () => page.waitFor(`document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy') === 'false'`, 'paging read settles');
  const type = async q => {
    await page.eval(`document.querySelector('#draft-search').focus(); document.querySelector('#draft-search').select()`);
    await page.send('Input.insertText', { text: q });
  };
  const search = async q => { await type(q); await page.key('Enter'); await settled(); };
  await page.goto(args.base + '/ban-viet');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 24`, 'first real page is bounded');
  await settled();
  const resume = await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href')`);
  await search('  Đường  về xa  '.normalize('NFD'));
  check(JSON.stringify(await rows()) === JSON.stringify([corpus[0].work_id]), 'browser finds the old draft without loading intermediate pages');
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resume)}`), 'filtered results do not redefine Viết tiếp');
  await search('[Paging201]');
  for (const sort of ['recent', 'title']) {
    if (sort === 'title') {
      await page.eval(`const s = document.querySelector('#draft-sort'); s.value='title'; s.dispatchEvent(new Event('change',{bubbles:true}))`);
      await settled();
    }
    const seen = new Set();
    for (let i = 1; i <= 10; i++) {
      const ids = await rows();
      check(ids.length === i * 24, `${sort}: append ${i} retains exactly its accepted pages`);
      ids.forEach(id => seen.add(id));
      if (i < 10) { await page.click('#load-more-drafts'); await settled(); }
    }
    check(await page.eval(`document.querySelector('#load-more-drafts').textContent.includes('Xem phần tiếp theo')`), 'ten pages offer an explicit next part');
    await page.click('#load-more-drafts'); await settled();
    const tail = await rows();
    check(tail.length === 10 && tail.every(id => !seen.has(id)), 'next part releases the old 240 summaries and retains only the next ten');
    tail.forEach(id => seen.add(id));
    check(seen.size === 250 && corpus.every(d => seen.has(d.work_id)), `${sort}: static traversal has no missing/duplicate IDs`);
    check(await page.eval(`!!document.querySelector('#draft-end') && !document.querySelector('#load-more-drafts')`), 'exhaustion is distinct from unavailable');
  }
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resume)}`), 'Title ordering retains the owner recent resume');

  await page.click('#refresh-drafts'); await settled();
  const gate = new DraftGate(page); gate.holdWhen = url => new URL(url).searchParams.get('limit') === '24';
  await gate.enable();
  try {
    // Failed append leaves the exact cursor and cards available for explicit retry.
    const initial = await rows(); gate.holdGets = true;
    await page.click('#load-more-drafts');
    const failed = await gate.next('gets'), cursor = new URL(gate.getURLs.get(failed)).searchParams.get('cursor');
    await gate.fulfill(failed, 503, { error: { code: 'dependency_unavailable' } });
    await settled();
    check(JSON.stringify(await rows()) === JSON.stringify(initial) && await page.eval(`!!document.querySelector('#draft-page-error') && !document.querySelector('#draft-end')`), 'append outage retains data and is not end-of-list');
    await page.click('#load-more-drafts');
    const retry = await gate.next('gets');
    check(new URL(gate.getURLs.get(retry)).searchParams.get('cursor') === cursor, 'retry submits the unadvanced cursor');
    gate.holdGets = false; await gate.answerGet(retry); await settled();
    check((await rows()).length === 48, 'retry appends the real next page once');

    // A successful head read discards old tails, even while input has an unsubmitted edit.
    await type('Chữ chưa gửi ỹ ỷ');
    await page.eval(`window.__deskInput=document.querySelector('#draft-search'); window.__deskInput.setSelectionRange(3,7); window.__deskInput.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true})); window.__deskClock=performance.now.bind(performance); performance.now=()=>window.__deskClock()+121000;`);
    const requestsBefore = gate.requests.length;
    await page.eval(`window.dispatchEvent(new Event('online')); window.dispatchEvent(new Event('focus'))`);
    await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length===24`, 'reconnect replaces the whole visible chain'); await settled();
    check(gate.requests.slice(requestsBefore).filter(u => new URL(u).searchParams.get('limit') === '24').length === 1, 'reconnect/focus burst coalesces one head request');
    check(await page.eval(`document.querySelector('#draft-search')===window.__deskInput && window.__deskInput.value==='Chữ chưa gửi ỹ ỷ' && window.__deskInput.selectionStart===3 && window.__deskInput.selectionEnd===7`), 'reconnect preserves input node, unsubmitted text and selection during simulated composition');
    await page.eval(`window.__deskInput.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true})); performance.now=window.__deskClock;`);
    check(gate.postCount === 0, 'read lifecycle never replays a create mutation');

    // Query A -> B -> A: the old A response must not overwrite the newer A generation.
    gate.holdGets = true;
    await type('duong ve xa'); await page.key('Enter');
    const oldA = await gate.next('gets');
    gate.holdGets = false; await search('Trang 249');
    check(JSON.stringify(await rows()) === JSON.stringify([corpus[249].work_id]), 'B settles while A is delayed');
    await search('duong ve xa');
    check(JSON.stringify(await rows()) === JSON.stringify([corpus[0].work_id]), 'new A settles independently');
    await gate.fulfill(oldA, 200, { drafts: [{ work_id: '99999999-9999-4999-8999-999999999999', title: 'OBSOLETE A', version: 1, updated_at: '2026-01-01T00:00:00Z' }] }).catch(() => {});
    await sleep(150);
    check(JSON.stringify(await rows()) === JSON.stringify([corpus[0].work_id]), 'late A cannot overwrite the newer A scope generation');

    await search('[Paging201]');
    // Cursor validation failure is a restart action, without advancing or discarding the data.
    gate.holdGets = true; await page.click('#load-more-drafts');
    const invalid = await gate.next('gets');
    await gate.fulfill(invalid, 422, { error: { code: 'validation', field: 'cursor' } }); await settled();
    check(await page.eval(`!!document.querySelector('#restart-drafts') && document.querySelectorAll('[data-testid=draft-row]').length===24`), 'invalid cursor retains cards and offers restart');
    for (const width of [390,1440]) for (const scale of [100,200]) {
      await page.viewport(width);
      await page.eval(`document.documentElement.style.fontSize='${scale}%'; document.querySelector('#draft-page-error').scrollIntoView({block:'center'})`);
      check(await page.eval(`document.documentElement.scrollWidth<=innerWidth`), `paging recovery fits ${width}px at text ${scale}%`);
      await page.capture(`desk-paging-${width}-${scale}`, { boundary: 'real-corpus-delayed-transport', scenario: '#201-cursor-restart' });
    }
    gate.holdGets = false;
    await page.eval(`document.documentElement.style.fontSize=''; document.querySelector('#restart-drafts').focus()`);
    await page.key('Enter'); await settled();
    check(await page.eval(`!document.querySelector('#draft-page-error') && document.activeElement.id==='draft-search'`), 'keyboard restart recovers and focuses search');
  } finally {
    await gate.disable();
    await page.viewport(1440);
    await page.eval(`document.documentElement.style.fontSize=''`);
  }
}
