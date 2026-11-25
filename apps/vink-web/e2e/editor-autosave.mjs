// Layout/keyboard matrices inspect transient dirty states for minutes. Hold the debounce
// clock in that document only. The lifecycle scenario below uses the real browser timer.
export async function holdEditorDebounce(page) {
  await page.eval(`(() => {
    if (window.__restoreEditorClock) return;
    const clock = window.setTimeout;
    window.setTimeout = (cb, delay, ...args) => clock.call(window, cb,
      delay === 1500 ? 2147483647 : delay, ...args);
    window.__restoreEditorClock = () => {
      window.setTimeout = clock;
      delete window.__restoreEditorClock;
    };
  })()`);
}

export async function editorAutosave(page, { base, check, openPage, SaveGate, sleep }) {
  const ready = "document.querySelector('.cm-content[contenteditable=true]')";
  const label = "document.querySelector('[data-testid=save-status]').textContent";
  await page.goto(base + '/ban-viet');
  await page.waitFor("document.querySelector('#create-draft')", 'desk for autosave');
  await page.click('#create-draft');
  await page.waitFor(ready, 'private autosave fixture');
  const path = await page.eval('location.pathname');
  const id = path.split('/').pop();
  await page.typeAtEnd('#draft-title', 'Thử lưu tự động');
  await sleep(1800);
  check((await page.draft(id)).version === 1 && await page.status() === 'unsaved', 'first Gửi waits for the author');
  await page.click('#save'); await page.waitStatus('saved');
  await page.click('#back-to-desk'); await page.waitFor("location.pathname === '/ban-viet'", 'clean exit');
  await page.goto(base + path); await page.waitFor(ready, 'clean reopen');
  await sleep(1800);
  check((await page.draft(id)).version === 2 && await page.status() === 'saved', 'mount and reopen do not write');

  const gate = new SaveGate(page);
  await gate.enable();
  await page.typeInBody('Mưa\n  qua hiên.');
  const first = await gate.next();
  await page.typeInBody('\n\nĐêm về.');
  await gate.release(first); await page.waitStatus('unsaved');
  check((await page.draft(id)).document.source === 'Mưa\n  qua hiên.', 'first ACK confirms only its own body');
  await gate.release(await gate.next()); await page.waitStatus('saved');
  const stored = await page.draft(id);
  check(stored.version === 4 && stored.document.source === 'Mưa\n  qua hiên.\n\nĐêm về.', 'newer edit autosaves with the acknowledged version and exact stanzas');
  await gate.disable();

  await page.eval("document.querySelector('#draft-title').focus()");
  await page.send('Input.imeSetComposition', {text:'ư', selectionStart:1, selectionEnd:1});
  await sleep(1800);
  check((await page.draft(id)).version === 4, 'open title composition cannot autosave');
  await page.send('Input.insertText', {text:'ưa'});
  await page.waitStatus('saved');
  check((await page.draft(id)).title === await page.eval("document.querySelector('#draft-title').value.trim().normalize('NFC')"), 'committed title autosaves without a second input');

  // A response-stage pause allows the PostgreSQL commit through first.
  const response = new SaveGate(page, 'Response');
  await response.enable();
  await page.typeInBody(' Đã ghi.');
  const committed = await response.next();
  const savedBeforeLoss = await page.draft(id);
  check(savedBeforeLoss.document.source.endsWith(' Đã ghi.'), 'server committed while autosave still awaits ACK');
  await response.fail(committed); await response.disable();
  await page.waitStatus('failed');
  check(await page.eval(label) === 'Chưa lưu được', 'missing ACK on an online browser remains uncertain');
  await page.click('#save'); await page.waitStatus('conflict');
  check((await page.draft(id)).version === savedBeforeLoss.version, 'readback after lost ACK avoids a second PUT');
  await page.click('#save-recovery-jump'); await page.click('#keep-mine');
  await page.waitStatus('saved');

  await gate.enable();
  await page.typeInBody(' Hẹn mai.');
  const leaving = await gate.next();
  await page.click('#back-to-desk');
  await page.waitFor("document.querySelector('#leave-editor').open", 'autosave pending on exit');
  await gate.release(leaving); await page.waitStatus('saved');
  const beforeExit = await page.draft(id);
  await page.click('#leave-save');
  await page.waitFor("location.pathname === '/ban-viet'", 'ACK before leave choice permits exit');
  check((await page.draft(id)).version === beforeExit.version && !gate.held.length, 'confirmed exit issues no extra PUT');
  await gate.disable();
  await page.goto(base + path); await page.waitFor(ready, 'reopen after acknowledged exit');

  const other = await openPage();
  // Open a deep link before the tab has received its first private account scope.
  let holdAccount;
  const accountPaused = new Promise(resolve => { holdAccount = resolve; });
  const interceptAccount = event => {
    if (event.sessionId === other.sessionId && event.method === 'Fetch.requestPaused') {
      holdAccount(event.params.requestId);
    }
  };
  other.cdp.listeners.push(interceptAccount);
  await other.send('Fetch.enable', {patterns: [{urlPattern: '*/api/v1/auth/session/account'}]});
  try {
    await other.goto(base + path);
    const accountRequest = await Promise.race([
      accountPaused, sleep(10000).then(() => { throw Error('account read did not start'); }),
    ]);
    check(await other.eval('!(' + ready + ')'), 'deep link waits for its first account before accepting edits');
    await other.send('Fetch.continueRequest', { requestId: accountRequest });
  } finally {
    await other.send('Fetch.disable');
    other.cdp.listeners = other.cdp.listeners.filter(event => event !== interceptAccount);
  }
  await other.waitFor(ready, 'second browser tab');
  await other.typeInBody(' Tab hai.'); await other.waitStatus('saved');
  await page.send('Page.bringToFront');
  await page.typeInBody(' Tab một.'); await page.waitStatus('conflict');
  check(await page.eval(label) === 'Xung đột' && (await page.body()).endsWith(' Tab một.') &&
    (await page.draft(id)).document.source.endsWith(' Tab hai.'), 'stale autosave from a second live editor preserves both buffers');
  await page.click('#back-to-desk');
  await page.waitFor("document.querySelector('#leave-editor').open", 'conflict blocks departure');
}
