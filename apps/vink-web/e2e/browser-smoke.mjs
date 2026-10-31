#!/usr/bin/env node
// Browser smoke for Vink SSR, hydration and the desk, driven over the Chrome DevTools Protocol.
// No npm dependencies: Node.js 22+ (global WebSocket/fetch) and a Chrome/Chromium binary.
// Run through `cargo xtask verify web`, which prepares a scratch database and the server.
//
//   node browser-smoke.mjs --base URL --chrome BIN --state FILE --phase main|after-restart
//
// Evidence limits: text is entered with Input.insertText / Input.imeSetComposition, which is
// programmatic input, not a real OS IME. One desktop-size headless Chrome is used.

import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const SAMPLE_WORK = '33333333-3333-4333-8333-333333333333';
const PUBLISHED_LINE = 'Phố cũ mưa giăng sợi mỏng,';
const INDENTED_LINE = '    đã theo mưa về rất lâu.';
const DRAFT_ONLY_LINE = 'Khổ đang viết dở — chưa công khai.';

const TITLE = 'Thử nghiệm Bàn viết';
const BODY = 'Sáng nay trời đổ mưa rào,\n  ngõ nhỏ ướt mềm từng viên gạch cũ.\n\nNgười về có nhớ hay sao —\nỹ ỷ ữ ợ Đ đ.';
const TAIL = ' (sửa tiếp)';

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

  status() {
    return this.eval(`document.querySelector('[data-testid=save-status]')?.dataset.status ?? null`);
  }
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

  console.log('client navigation and draft creation:');
  await page.eval(`window.__vinkNavigation = 'kept'`);
  await page.click('.vk-nav a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('[data-testid=create-draft]')`, 'desk list');
  check(await page.eval(`window.__vinkNavigation === 'kept'`), 'navigation stayed in the hydrated app (no full reload)');
  await page.click('[data-testid=create-draft]');
  await page.waitFor(`/^\\/ban-viet\\/[0-9a-f-]{36}$/.test(location.pathname) && document.querySelector('textarea')`, 'new draft editor');
  const workId = await page.eval(`location.pathname.split('/').pop()`);
  check((await page.status()) === 'saved', 'new draft starts saved at version 1');

  console.log('typing and saving Vietnamese text:');
  await page.typeAtEnd('input[name=title]', TITLE);
  await page.typeAtEnd('textarea', BODY.slice(0, -1));
  let composed = 'not supported';
  try {
    await page.send('Input.imeSetComposition', { text: 'đ', selectionStart: 1, selectionEnd: 1 });
    await page.send('Input.insertText', { text: '.' });
    composed = 'simulated';
  } catch {
    await page.send('Input.insertText', { text: '.' });
  }
  console.log(`  ..  IME composition events: ${composed} (not a real OS IME)`);
  const typed = await page.eval(`document.querySelector('textarea').value`);
  check(typed === BODY, 'textarea holds exactly the typed text');
  check((await page.status()) === 'unsaved', 'status shows unsaved after typing');
  await page.click('[data-testid=save]');
  await page.waitFor(`document.querySelector('[data-testid=save-status]').dataset.status === 'saved'`, 'save confirmation');
  check((await page.eval(`document.querySelector('[data-testid=save-status]').textContent`)).includes('phiên bản 2'), 'saved only after server confirmed version 2');

  const stored = await page.eval(`fetch('/api/v1/drafts/${workId}').then((r) => r.json())`);
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
    `window.__vinkBeforeReload === undefined && document.readyState === 'complete' && document.querySelector('textarea') && document.querySelector('input[name=title]') && document.documentElement.dataset.hydrated === 'true'`,
    'new editor after reload',
  );
  check((await page.eval(`document.querySelector('textarea').value`)) === BODY, 'text restored after reload');
  check((await page.eval(`document.querySelector('input[name=title]').value`)) === TITLE, 'title restored after reload');

  console.log('version conflict with another tab:');
  const other = await page.eval(`fetch('/api/v1/drafts/${workId}', {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ expected_version: 2, title: 'Tab khác', document: { schema_version: 1, blocks: [{ lines: ['Bản của tab khác.'] }] } }),
  }).then((r) => r.status)`);
  check(other === 200, 'another tab saved version 3 first');
  await page.typeAtEnd('textarea', TAIL);
  await page.click('[data-testid=save]');
  await page.waitFor(`document.querySelector('[data-testid=save-status]').dataset.status === 'conflict'`, 'conflict state');
  check((await page.eval(`document.querySelector('textarea').value`)) === BODY + TAIL, 'conflict keeps the text being written');
  const serverAfterConflict = await page.eval(`fetch('/api/v1/drafts/${workId}').then((r) => r.json())`);
  check(serverAfterConflict.title === 'Tab khác' && serverAfterConflict.version === 3, 'stale save wrote nothing');
  await page.click('[data-testid=keep-mine]');
  await page.waitFor(`document.querySelector('[data-testid=save-status]').dataset.status === 'saved'`, 'explicit overwrite');
  const final = await page.eval(`fetch('/api/v1/drafts/${workId}').then((r) => r.json())`);
  check(final.version === 4 && final.title === TITLE, 'author chose to keep local text: version 4');

  writeFileSync(args.state, JSON.stringify({ workId, title: TITLE, text: BODY + TAIL, version: 4 }));
}

async function afterRestartPhase(page) {
  const state = JSON.parse(readFileSync(args.state, 'utf8'));
  console.log('after server restart:');
  await page.goto(`${args.base}/ban-viet/${state.workId}`);
  await page.waitFor(`document.querySelector('textarea') && document.documentElement.dataset.hydrated === 'true'`, 'editor');
  check((await page.eval(`document.querySelector('textarea').value`)) === state.text, 'confirmed text survived the restart');
  check((await page.eval(`document.querySelector('input[name=title]').value`)) === state.title, 'title survived the restart');
  check((await page.eval(`document.querySelector('[data-testid=save-status]').textContent`)).includes(`phiên bản ${state.version}`), 'version survived the restart');
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
