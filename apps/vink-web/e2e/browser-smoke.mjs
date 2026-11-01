#!/usr/bin/env node
// Browser smoke for Vink SSR, hydration and the desk, driven over the Chrome DevTools Protocol.
// No npm dependencies: Node.js 22+ (global WebSocket/fetch) and a Chrome/Chromium binary.
// Run through `cargo xtask verify web`, which prepares a scratch database and the server.
//
//   node browser-smoke.mjs --base URL --chrome BIN --state FILE --phase main|after-restart
//
// Evidence limits: text is entered with Input.insertText / Input.imeSetComposition and keys with
// Input.dispatchKeyEvent, which is programmatic input, not a real OS IME or keyboard layout. One
// desktop-size headless Chrome is used. Save responses are held or failed with the Fetch domain.

import { spawn } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SAMPLE_WORK = '33333333-3333-4333-8333-333333333333';
const PUBLISHED_LINE = 'Phố cũ mưa giăng sợi mỏng,';
const INDENTED_LINE = '    đã theo mưa về rất lâu.';
const DRAFT_ONLY_LINE = 'Khổ đang viết dở — chưa công khai.';

const TITLE = 'Thử nghiệm Bàn viết';
const BODY = 'Sáng nay trời đổ mưa rào,\n  ngõ nhỏ ướt mềm từng viên gạch cũ.\n\nNgười về có nhớ hay sao —\nỹ ỷ ữ ợ Đ đ.';
const TAIL = ' (sửa tiếp)';
const STANZA = '\n\nKhổ mới chưa lưu,\n  dòng thụt vào,\nlời cuối ở đây.';
const LAST_LINE = 'lời cuối ở đây.';

// CodeMirror renders one `.cm-line` per line of the document (all of it, for texts this short).
const BODY_TEXT = `[...document.querySelectorAll('[data-testid=editor-body] .cm-line')].map((l) => l.textContent).join('\\n')`;
const EDITOR_READY = `document.querySelector('[data-testid=editor-body] .cm-content[contenteditable=true]')`;
const CTRL = 2;
const SHIFT = 8;
const KEYS = {
  Escape: { code: 'Escape', keyCode: 27 },
  End: { code: 'End', keyCode: 35 },
  Home: { code: 'Home', keyCode: 36 },
  s: { code: 'KeyS', keyCode: 83 },
  z: { code: 'KeyZ', keyCode: 90 },
  Z: { code: 'KeyZ', keyCode: 90 },
};

const args = Object.fromEntries(
  process.argv.slice(2).reduce((pairs, value, i, all) => {
    if (value.startsWith('--')) pairs.push([value.slice(2), all[i + 1]]);
    return pairs;
  }, []),
);
for (const key of ['base', 'chrome', 'state', 'phase']) {
  if (!args[key]) fail(`missing --${key}`);
}

function fail(message) {
  console.error(`browser-smoke: FAIL ${message}`);
  process.exit(1);
}

function check(condition, message) {
  if (!condition) throw new Error(message);
  console.log(`  ok  ${message}`);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

class Cdp {
  constructor(url) {
    this.ws = new WebSocket(url);
    this.nextId = 0;
    this.pending = new Map();
    this.listeners = [];
  }

  async open() {
    await new Promise((resolve, reject) => {
      this.ws.onopen = resolve;
      this.ws.onerror = () => reject(new Error('cannot open DevTools WebSocket'));
    });
    this.ws.onmessage = (event) => {
      const message = JSON.parse(event.data);
      const waiter = message.id && this.pending.get(message.id);
      if (waiter) {
        this.pending.delete(message.id);
        if (message.error) waiter.reject(new Error(`${waiter.method}: ${message.error.message}`));
        else waiter.resolve(message.result);
      } else if (message.method) {
        for (const listener of this.listeners) listener(message);
      }
    };
  }

  send(method, params = {}, sessionId) {
    const id = ++this.nextId;
    this.ws.send(JSON.stringify({ id, method, params, sessionId }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject, method }));
  }
}

class Page {
  constructor(cdp, sessionId) {
    this.cdp = cdp;
    this.sessionId = sessionId;
    this.errors = [];
    cdp.listeners.push((message) => {
      if (message.sessionId !== sessionId) return;
      if (message.method === 'Runtime.exceptionThrown') {
        const details = message.params.exceptionDetails;
        this.errors.push(details.exception?.description ?? details.text);
      } else if (message.method === 'Runtime.consoleAPICalled' && message.params.type === 'error') {
        this.errors.push(message.params.args.map((a) => a.value ?? a.description).join(' '));
      }
    });
  }

  send(method, params) {
    return this.cdp.send(method, params, this.sessionId);
  }

  async eval(expression) {
    const result = await this.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (result.exceptionDetails) {
      throw new Error(`evaluate failed: ${result.exceptionDetails.exception?.description ?? expression}`);
    }
    return result.result.value;
  }

  async waitFor(expression, what, timeout = 20000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      try {
        if (await this.eval(`Boolean(${expression})`)) return;
      } catch {
        // The document may be navigating; retry.
      }
      await sleep(100);
    }
    throw new Error(`timed out waiting for ${what}`);
  }

  async goto(url) {
    await this.send('Page.navigate', { url });
    await this.waitFor(`location.href === ${JSON.stringify(url)} && document.readyState === 'complete'`, `load ${url}`);
  }

  async click(selector) {
    const point = await this.eval(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      if (!el) return null;
      el.scrollIntoView({ block: 'center' });
      const r = el.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    })()`);
    if (!point) throw new Error(`no element ${selector}`);
    for (const type of ['mousePressed', 'mouseReleased']) {
      await this.send('Input.dispatchMouseEvent', { type, x: point.x, y: point.y, button: 'left', clickCount: 1 });
    }
  }

  /** Focuses a field, puts the caret at the end and inserts text as committed input. */
  async typeAtEnd(selector, text) {
    await this.eval(`(() => {
      const el = document.querySelector(${JSON.stringify(selector)});
      el.focus();
      el.setSelectionRange(el.value.length, el.value.length);
    })()`);
    await this.send('Input.insertText', { text });
  }

  /** A key press as the browser receives it from the keyboard. */
  async key(name, modifiers = 0) {
    const { code, keyCode } = KEYS[name];
    for (const type of ['rawKeyDown', 'keyUp']) {
      await this.send('Input.dispatchKeyEvent', { type, key: name, code, modifiers, windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode });
    }
  }

  /** Focuses the CodeMirror body, moves to the end with Ctrl+End and inserts committed text. */
  async typeInBody(text) {
    await this.eval(`document.querySelector('[data-testid=editor-body] .cm-content').focus()`);
    await this.key('End', CTRL);
    if (text) await this.send('Input.insertText', { text });
  }

  body() {
    return this.eval(BODY_TEXT);
  }

  status() {
    return this.eval(`document.querySelector('[data-testid=save-status]')?.dataset.status ?? null`);
  }

  async waitStatus(status, what = status) {
    await this.waitFor(`document.querySelector('[data-testid=save-status]')?.dataset.status === ${JSON.stringify(status)}`, what);
  }

  dialogOpen(testid) {
    return this.eval(`document.querySelector('[data-testid=${testid}]')?.open === true`);
  }

  async draft(workId) {
    return this.eval(`fetch('/api/v1/drafts/${workId}').then((r) => r.json())`);
  }
}

/** Holds draft saves (PUT) until released or failed; every other request continues. */
class SaveGate {
  constructor(page) {
    this.page = page;
    this.held = [];
    page.cdp.listeners.push((message) => {
      if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
      const { requestId, request } = message.params;
      if (request.method === 'PUT') this.held.push(requestId);
      else page.send('Fetch.continueRequest', { requestId }).catch(() => {});
    });
  }

  enable() {
    return this.page.send('Fetch.enable', { patterns: [{ urlPattern: '*/api/v1/drafts/*', requestStage: 'Request' }] });
  }

  disable() {
    return this.page.send('Fetch.disable');
  }

  async next() {
    const deadline = Date.now() + 10000;
    while (!this.held.length) {
      if (Date.now() > deadline) throw new Error('timed out waiting for a save request');
      await sleep(50);
    }
    return this.held.shift();
  }

  release(requestId) {
    return this.page.send('Fetch.continueRequest', { requestId });
  }

  fail(requestId) {
    return this.page.send('Fetch.failRequest', { requestId, errorReason: 'ConnectionRefused' });
  }
}

/** Waits for a finished download in `dir` and returns its name and UTF-8 text. Chrome reserves
 *  the final name with an empty file while `.crdownload` is still being written. */
async function downloaded(dir, before = []) {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    const all = readdirSync(dir);
    const names = all.filter((n) => !before.includes(n));
    if (names.length === 1 && !all.some((n) => n.endsWith('.crdownload'))) {
      return { name: names[0], text: readFileSync(join(dir, names[0]), 'utf8') };
    }
    await sleep(100);
  }
  throw new Error('no finished download');
}

async function launchChrome() {
  const profile = mkdtempSync(join(tmpdir(), 'vink-chrome-'));
  const flags = [
    '--headless=new',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-extensions',
    '--disable-gpu',
    '--window-size=1280,900',
    '--lang=vi-VN',
    'about:blank',
  ];
  // CI runners may not allow Chrome's user-namespace sandbox; only our local app is loaded.
  if (process.env.CI || process.env.VINK_CHROME_NO_SANDBOX) flags.push('--no-sandbox');
  const child = spawn(args.chrome, flags, { stdio: ['ignore', 'ignore', 'pipe'] });
  let output = '';
  let exited = null;
  child.stderr.on('data', (chunk) => (output += chunk));
  child.on('exit', (code) => (exited = code));
  // Chrome writes "<port>\n<browser path>" to DevToolsActivePort once the endpoint is open.
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) {
    if (exited !== null) throw new Error(`Chrome exited (${exited}): ${output}`);
    try {
      const [port, path] = readFileSync(join(profile, 'DevToolsActivePort'), 'utf8').split('\n');
      if (port && path) return { child, profile, url: `ws://127.0.0.1:${port.trim()}${path.trim()}` };
    } catch {
      // Not written yet.
    }
    await sleep(100);
  }
  child.kill();
  throw new Error(`Chrome did not open DevTools within 20s: ${output}`);
}

async function openPage(cdp) {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const page = new Page(cdp, sessionId);
  await page.send('Page.enable');
  await page.send('Runtime.enable');
  return page;
}

async function serverHtml() {
  console.log('server HTML before any JavaScript:');
  const reader = await fetch(`${args.base}/tac-pham/${SAMPLE_WORK}`);
  const html = await reader.text();
  check(reader.status === 200, 'reader responds 200');
  check(html.includes(PUBLISHED_LINE) && html.includes(INDENTED_LINE), 'reader HTML contains the published text');
  check(/<link[^>]+rel="canonical"/.test(html) && html.includes('<title>[Mẫu] Mưa trên phố cũ — Vink</title>'), 'reader HTML has title and canonical link');
  check(!html.includes(DRAFT_ONLY_LINE), 'reader HTML and hydration payload exclude the private draft');
  const desk = await (await fetch(`${args.base}/ban-viet/${SAMPLE_WORK}`)).text();
  check(!desk.includes(DRAFT_ONLY_LINE), 'desk HTML is a shell without draft text');
  const anonymous = await fetch(`${args.base}/api/v1/drafts`);
  check(anonymous.status === 200, 'loopback identity fixture is active for this local run');
}

async function mainPhase(page) {
  await serverHtml();

  console.log('reader in the browser:');
  await page.goto(`${args.base}/tac-pham/${SAMPLE_WORK}`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'hydration');
  check(true, 'page hydrated');
  const blocks = await page.eval(`[...document.querySelectorAll('.vk-block')].map((b) => b.textContent)`);
  check(blocks.length === 2, 'two stanzas render as two blocks');
  check(blocks[1].split('\n').length === 4 && blocks[1].includes(INDENTED_LINE), 'line breaks and indentation survive rendering');

  console.log('self-hosted fonts:');
  const loaded = await page.eval(`document.fonts.ready.then(() => [...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family.replaceAll('"', '') + ' ' + f.weight))`);
  check(loaded.includes('Be Vietnam Pro 400') && loaded.includes('Literata 200 900'), `reader loaded the UI and reading faces (${loaded.join(', ')})`);
  const served = await page.eval(`fetch('/pkg/fonts/literata-variable.woff2').then((r) => r.status + ' ' + r.headers.get('content-type'))`);
  check(served === '200 font/woff2', 'font files are served as font/woff2');
  const editorOnReader = await page.eval(`performance.getEntriesByType('resource').some((e) => e.name.endsWith('/pkg/vink-editor.js'))`);
  check(!editorOnReader, 'the public reader does not load the editor runtime');

  console.log('client navigation and draft creation:');
  await page.eval(`window.__vinkNavigation = 'kept'`);
  await page.click('.vk-nav a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('[data-testid=create-draft]')`, 'desk list');
  check(await page.eval(`window.__vinkNavigation === 'kept'`), 'navigation stayed in the hydrated app (no full reload)');
  await page.click('[data-testid=create-draft]');
  await page.waitFor(`/^\\/ban-viet\\/[0-9a-f-]{36}$/.test(location.pathname) && ${EDITOR_READY}`, 'new draft editor');
  const workId = await page.eval(`location.pathname.split('/').pop()`);
  check((await page.status()) === 'saved', 'new draft starts saved at version 1');

  console.log('CodeMirror 6 body editor:');
  check(await page.eval(`document.querySelector('textarea') === null && document.querySelectorAll('.cm-editor').length === 1`), 'one CodeMirror view and no textarea');
  const runtime = await page.eval(`performance.getEntriesByType('resource').find((e) => e.name.endsWith('/pkg/vink-editor.js'))?.responseStatus ?? null`);
  const runtimeType = await page.eval(`fetch('/pkg/vink-editor.js').then((r) => r.headers.get('content-type'))`);
  check(runtime === 200 && /javascript/.test(runtimeType), `editor runtime loaded from /pkg/vink-editor.js (${runtimeType})`);
  check(await page.eval(`document.getElementById(document.querySelector('.cm-content').getAttribute('aria-labelledby'))?.textContent === 'Nội dung'`), 'text box is named by the visible “Nội dung” label');
  check(await page.eval(`document.activeElement?.name === 'title'`), 'a new, empty draft starts with focus on its title');
  check(await page.eval(`document.querySelector('input[name=title]').maxLength === 200`), 'title input keeps the API limit of 200');

  console.log('typing and saving Vietnamese text:');
  await page.typeAtEnd('input[name=title]', TITLE);
  await page.typeInBody(BODY.slice(0, -2));
  let composed = 'not supported';
  try {
    await page.send('Input.imeSetComposition', { text: 'd', selectionStart: 1, selectionEnd: 1 });
    await page.send('Input.imeSetComposition', { text: 'đ', selectionStart: 1, selectionEnd: 1 });
    await page.send('Input.insertText', { text: 'đ' });
    composed = 'simulated';
  } catch {
    await page.send('Input.insertText', { text: 'đ' });
  }
  await page.send('Input.insertText', { text: '.' });
  console.log(`  ..  IME composition events: ${composed} (not a real OS IME)`);
  check((await page.body()) === BODY, 'CodeMirror holds exactly the typed text, lines and stanza');
  check((await page.status()) === 'unsaved', 'status shows unsaved after typing');
  await page.click('[data-testid=save]');
  await page.waitStatus('saved', 'save confirmation');
  check((await page.eval(`document.querySelector('[data-testid=save-status]').textContent`)).includes('phiên bản 2'), 'saved only after server confirmed version 2');

  const stored = await page.draft(workId);
  check(stored.title === TITLE, 'server stored the title');
  check(JSON.stringify(stored.document.blocks) === JSON.stringify([
    { lines: ['Sáng nay trời đổ mưa rào,', '  ngõ nhỏ ướt mềm từng viên gạch cũ.'] },
    { lines: ['Người về có nhớ hay sao —', 'ỹ ỷ ữ ợ Đ đ.'] },
  ]), 'server stored stanzas, lines, indentation and diacritics');

  console.log('reload keeps the confirmed draft:');
  // Page.reload can return while the old, hydrated editor is still present.
  // Wait for its window to be replaced before accepting the new editor as ready.
  await page.eval(`window.__vinkBeforeReload = true`);
  await page.send('Page.reload');
  await page.waitFor(
    `window.__vinkBeforeReload === undefined && document.readyState === 'complete' && ${EDITOR_READY} && document.querySelector('input[name=title]') && document.documentElement.dataset.hydrated === 'true'`,
    'new editor after reload',
  );
  check((await page.body()) === BODY, 'text restored after reload');
  check((await page.eval(`document.querySelector('input[name=title]').value`)) === TITLE, 'title restored after reload');

  console.log('version conflict with another tab:');
  const other = await page.eval(`fetch('/api/v1/drafts/${workId}', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ expected_version: 2, title: 'Tab khác', document: { schema_version: 1, blocks: [{ lines: ['Bản của tab khác.'] }] } }),
  }).then((r) => r.status)`);
  check(other === 200, 'another tab saved version 3 first');
  await page.typeInBody(TAIL);
  await page.click('[data-testid=save]');
  await page.waitStatus('conflict', 'conflict state');
  check((await page.body()) === BODY + TAIL, 'conflict keeps the text being written');
  const serverAfterConflict = await page.draft(workId);
  check(serverAfterConflict.title === 'Tab khác' && serverAfterConflict.version === 3, 'stale save wrote nothing');
  await page.click('[data-testid=keep-mine]');
  await page.waitStatus('saved', 'explicit overwrite');
  const final = await page.draft(workId);
  check(final.version === 4 && final.title === TITLE, 'author chose to keep local text: version 4');

  console.log('confirmed text leaves without asking; reopening through the app:');
  const editorPath = `/ban-viet/${workId}`;
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('a[href="${editorPath}"]')`, 'desk list');
  check(!(await page.dialogOpen('leave-editor')), 'no leave dialog for server-confirmed text');
  await page.click(`a[href="${editorPath}"]`);
  await page.waitFor(`location.pathname === '${editorPath}' && ${EDITOR_READY}`, 'editor reopened');

  console.log('one EditorView keeps selection and history across theme, preview and dialogs:');
  await page.eval(`document.querySelector('.cm-editor').__vinkView = 'kept'`);
  const view = `document.querySelector('.cm-editor')?.__vinkView === 'kept' && document.querySelectorAll('.cm-editor').length === 1`;
  await page.typeInBody(STANZA);
  const written = BODY + TAIL + STANZA;
  check((await page.body()) === written, 'new stanza typed with an indented line');
  await page.key('Home', SHIFT);
  const selected = `getSelection().toString()`;
  check((await page.eval(selected)) === LAST_LINE, 'last line selected with Shift+Home');
  const surface = `getComputedStyle(document.querySelector('[data-testid=editor-body]')).backgroundColor`;
  const lightSurface = await page.eval(surface);
  await page.eval(`document.documentElement.dataset.theme = 'dark'`);
  await page.waitFor(`${surface} !== ${JSON.stringify(lightSurface)}`, 'dark theme applied');
  check(await page.eval(view), 'theme change kept the same EditorView');
  check((await page.eval(selected)) === LAST_LINE, 'theme change kept the selection');

  await page.eval(`scrollTo(0, document.documentElement.scrollHeight)`);
  const scrolled = await page.eval(`scrollY`);
  await page.eval(`document.querySelector('[data-testid=preview]').click()`);
  await page.waitFor(`document.querySelector('[data-testid=draft-preview]').open`, 'preview dialog');
  const preview = await page.eval(`[...document.querySelectorAll('[data-testid=draft-preview] .vk-block')].map((b) => b.textContent)`);
  check(preview.length === 3 && preview[2] === 'Khổ mới chưa lưu,\n  dòng thụt vào,\nlời cuối ở đây.', 'preview shows the unsaved stanza with its line breaks');
  check((await page.eval(`document.querySelector('[data-testid=draft-preview] .vk-reader__title').textContent`)) === TITLE, 'preview shows the current title');
  check((await page.status()) === 'unsaved' && (await page.draft(workId)).version === 4, 'preview neither saved nor marked saved');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('[data-testid=draft-preview]').open`, 'preview closed by Escape');
  check(await page.eval(view), 'preview kept the same EditorView');
  check(scrolled > 0 && Math.abs((await page.eval(`scrollY`)) - scrolled) < 2, `closing the preview kept the page scroll (${scrolled}px)`);
  await page.eval(`document.querySelector('[data-testid=editor-body] .cm-content').focus()`);
  check((await page.eval(selected)) === LAST_LINE, 'selection is back when the text regains focus');

  await page.click('.vk-brand');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog from the brand link');
  check((await page.eval(`location.pathname`)) === editorPath, 'brand link held while text is unsaved');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog closed by Escape');
  check((await page.eval(`location.pathname`)) === editorPath, 'Escape stays in the editor');
  check(await page.eval(`document.activeElement?.classList.contains('cm-content')`), 'Escape returns focus to the text');
  check((await page.eval(selected)) === LAST_LINE && (await page.body()) === written, 'Escape keeps the text and selection');
  check(await page.eval(view), 'dialog kept the same EditorView');

  await page.key('End');
  await page.send('Input.insertText', { text: ' Thêm' });
  check((await page.body()) === `${written} Thêm`, 'typed after the dialog');
  await page.key('z', CTRL);
  check((await page.body()) === written, 'Ctrl+Z undoes the last insertion');
  await page.key('Z', CTRL | SHIFT);
  check((await page.body()) === `${written} Thêm`, 'Ctrl+Shift+Z redoes it');
  check(await page.eval(view), 'undo and redo in the same EditorView');
  await page.eval(`delete document.documentElement.dataset.theme`);

  // Chrome asks before a page's second download, which headless Chrome cannot show: this phase
  // downloads once (the recovery export); the toolbar export runs in the after-restart phase.
  const downloads = mkdtempSync(join(tmpdir(), 'vink-downloads-'));
  try {
    await page.cdp.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads });

    console.log('shortcut save, then back/forward with unsaved text:');
    await page.key('s', CTRL);
    await page.waitStatus('saved', 'Ctrl+S save');
    const afterShortcut = await page.draft(workId);
    check(afterShortcut.version === 5, 'Ctrl+S saved version 5');
    await page.typeInBody(' Lui lại.');
    await page.eval(`history.back()`);
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog from history back');
    check((await page.eval(`location.pathname`)) === editorPath, 'back is held and the editor URL restored');
    await page.click('[data-testid=leave-stay]');
    check((await page.eval(`location.pathname`)) === editorPath && (await page.body()).endsWith(' Lui lại.'), '“Ở lại” keeps the route and the text');
    await page.eval(`history.back()`);
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog again');
    await page.click('[data-testid=leave-save]');
    await page.waitFor(`location.pathname === '/ban-viet'`, 'save and leave');
    const afterLeave = await page.draft(workId);
    check(afterLeave.version === 6 && afterLeave.document.blocks.at(-1).lines.at(-1) === `${LAST_LINE} Thêm Lui lại.`, 'left only after the server confirmed the latest text (version 6)');

    console.log('cancelling a save-and-leave in flight:');
    const gate = new SaveGate(page);
    await gate.enable();
    await page.click(`a[href="${editorPath}"]`);
    await page.waitFor(`location.pathname === '${editorPath}' && ${EDITOR_READY}`, 'editor reopened');
    await page.typeInBody(' Hủy.');
    await page.click('.vk-nav a[href="/"]');
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog');
    check(!(await page.eval(`document.querySelector('[data-testid=leave-recovery]')`)), 'no discard offered while saving is still possible');
    await page.click('[data-testid=leave-save]');
    const held = await gate.next();
    await page.waitStatus('saving');
    await page.key('Escape');
    await page.waitFor(`!document.querySelector('[data-testid=leave-editor]').open`, 'stayed while saving');
    await gate.release(held);
    await page.waitStatus('saved', 'late acknowledgement');
    await sleep(700);
    check((await page.eval(`location.pathname`)) === editorPath, 'a late successful save did not navigate');

    console.log('persistent failure: export, explicit discard, late acknowledgement:');
    await page.typeInBody(' Mất kết nối.');
    await page.click('.vk-nav a[href="/"]');
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog');
    await page.click('[data-testid=leave-save]');
    await gate.fail(await gate.next());
    await page.waitStatus('failed', 'failed save');
    check((await page.eval(`document.querySelector('[data-testid=leave-message]').textContent`)).includes('Chưa lưu được'), 'dialog explains the failed save');
    check((await page.eval(`location.pathname`)) === editorPath && (await page.body()).endsWith(' Mất kết nối.'), 'failure keeps the route and the text');
    await page.click('[data-testid=leave-export]');
    const rescue = await downloaded(downloads);
    check(rescue.text.endsWith(' Mất kết nối.\n'), `recovery export holds the unsaved text (${rescue.name})`);
    await page.click('[data-testid=leave-save]');
    const late = await gate.next();
    await page.click('[data-testid=leave-discard]');
    await page.waitFor(`document.querySelector('[data-testid=discard-confirm]')`, 'discard confirmation');
    check(await page.eval(`document.activeElement?.dataset.testid === 'discard-back'`), 'confirmation focuses “Quay lại”, not the loss');
    await page.click('[data-testid=discard-confirm]');
    await page.waitFor(`location.pathname === '/'`, 'left without saving');
    await page.click('.vk-nav a[href="/ban-viet"]');
    await page.waitFor(`document.querySelector('a[href="/ban-viet/${SAMPLE_WORK}"]')`, 'desk list');
    await page.click(`a[href="/ban-viet/${SAMPLE_WORK}"]`);
    await page.waitFor(`location.pathname === '/ban-viet/${SAMPLE_WORK}' && ${EDITOR_READY}`, 'another draft');
    await page.typeInBody(' Bản khác.');
    await gate.release(late);
    await sleep(1000);
    check((await page.eval(`location.pathname`)) === `/ban-viet/${SAMPLE_WORK}`, 'the late acknowledgement did not navigate again');
    check((await page.status()) === 'unsaved', 'nor marked the other draft saved');
    await gate.disable();
  } finally {
    rmSync(downloads, { recursive: true, force: true });
  }

  const kept = await page.draft(workId);
  const text = kept.document.blocks.map((block) => block.lines.join('\n')).join('\n\n');
  writeFileSync(args.state, JSON.stringify({ workId, title: kept.title, text, version: kept.version }));
}

async function afterRestartPhase(page) {
  const state = JSON.parse(readFileSync(args.state, 'utf8'));
  console.log('after server restart:');
  await page.goto(`${args.base}/ban-viet/${state.workId}`);
  await page.waitFor(`${EDITOR_READY} && document.documentElement.dataset.hydrated === 'true'`, 'editor');
  check((await page.body()) === state.text, 'confirmed text survived the restart');
  check((await page.eval(`document.querySelector('input[name=title]').value`)) === state.title, 'title survived the restart');
  check((await page.eval(`document.querySelector('[data-testid=save-status]').textContent`)).includes(`phiên bản ${state.version}`), 'version survived the restart');

  console.log('export keeps the text being written:');
  const downloads = mkdtempSync(join(tmpdir(), 'vink-downloads-'));
  try {
    await page.cdp.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads });
    await page.typeInBody(STANZA);
    const current = await page.body();
    check(current === state.text + STANZA, 'unsaved stanza typed after the restart');
    await page.click('[data-testid=export]');
    const file = await downloaded(downloads);
    check(file.name === `${state.title}.txt`, `export file is named after the title (${file.name})`);
    check(file.text === `${state.title}\n\n${current}\n`, 'export holds the title and the unsaved text with lines and stanzas');
    check((await page.status()) === 'unsaved', 'export is not a save');
  } finally {
    rmSync(downloads, { recursive: true, force: true });
  }
}

const chrome = await launchChrome();
let exitCode = 0;
try {
  const cdp = new Cdp(chrome.url);
  await cdp.open();
  const page = await openPage(cdp);
  if (args.phase === 'main') await mainPhase(page);
  else if (args.phase === 'after-restart') await afterRestartPhase(page);
  else throw new Error(`unknown phase ${args.phase}`);
  check(page.errors.length === 0, `no console errors or exceptions${page.errors.length ? `: ${page.errors.join(' | ')}` : ''}`);
  console.log(`browser-smoke: PASS (${args.phase})`);
} catch (error) {
  console.error(`browser-smoke: FAIL ${error.message}`);
  exitCode = 1;
} finally {
  chrome.child.kill();
  await sleep(200);
  rmSync(chrome.profile, { recursive: true, force: true });
}
process.exit(exitCode);
