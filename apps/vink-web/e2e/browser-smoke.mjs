#!/usr/bin/env node
// Browser smoke for Vink SSR, hydration and the desk, driven over the Chrome DevTools Protocol.
// No npm dependencies: Node.js 22+ (global WebSocket/fetch) and a Chrome/Chromium binary.
// Run through `cargo xtask verify web`, which prepares a scratch database and the server.
//
//   node browser-smoke.mjs --base URL --chrome BIN --state FILE --phase main|after-restart
//
// Evidence limits: text is entered with Input.insertText / Input.imeSetComposition and keys with
// Input.dispatchKeyEvent, which is programmatic input, not a real OS IME or keyboard layout. One
// headless Chrome is used at desktop/tablet/mobile CSS viewport sizes. Save responses are held or failed with the Fetch domain.

import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

const SAMPLE_WORK = '33333333-3333-4333-8333-333333333333';
const PUBLISHED_LINE = 'Phố cũ mưa giăng sợi mỏng,';
const INDENTED_LINE = '    đã theo mưa về rất lâu.';
const DRAFT_ONLY_LINE = 'Khổ đang viết dở — chưa công khai.';

const TITLE = 'Thử nghiệm Bàn viết — Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ tiếng Việt và những khổ thơ còn đang viết dở';
// The existing export policy caps file names at 80 characters; the file keeps the full title.
const EXPORT_NAME = 'Thử nghiệm Bàn viết — Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ.txt';
const BODY = 'Sáng nay trời đổ mưa rào,\n  ngõ nhỏ ướt mềm từng viên gạch cũ.\n\nNgười về có nhớ hay sao —\nỹ ỷ ữ ợ Đ đ.';
const TAIL = ' (sửa tiếp)';
const STANZA = '\n\nKhổ mới chưa lưu,\n  dòng thụt vào,\nlời cuối ở đây.';
const LAST_LINE = 'lời cuối ở đây.';

// CodeMirror renders one `.cm-line` per line of the document (all of it, for texts this short).
const BODY_TEXT = `[...document.querySelectorAll('[data-testid=editor-body] .cm-line')].map((l) => l.textContent).join('\\n')`;
const EDITOR_READY = `document.querySelector('[data-testid=editor-body] .cm-content[contenteditable=true]')`;
const CTRL = 2;
const META = 4;
const SHIFT = 8;
const KEYS = {
  Escape: { code: 'Escape', keyCode: 27 },
  Tab: { code: 'Tab', keyCode: 9 },
  End: { code: 'End', keyCode: 35 },
  Home: { code: 'Home', keyCode: 36 },
  ArrowDown: { code: 'ArrowDown', keyCode: 40 },
  ArrowLeft: { code: 'ArrowLeft', keyCode: 37 },
  ArrowRight: { code: 'ArrowRight', keyCode: 39 },
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
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method}: no DevTools response within 30s`));
      }, 30000);
      this.pending.set(id, {
        resolve: result => { clearTimeout(timer); resolve(result); },
        reject: error => { clearTimeout(timer); reject(error); },
        method,
      });
    });
  }
}

class Page {
  constructor(cdp, sessionId) {
    this.cdp = cdp;
    this.sessionId = sessionId;
    this.errors = [];
    this.mac = false;
    this.mod = CTRL;
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

  async reload() {
    await this.eval(`window.__beforeReload = true`);
    await this.send('Page.reload');
    await this.waitFor(`!window.__beforeReload && document.documentElement.dataset.hydrated === 'true'`, 'fresh hydrated document');
  }

  viewport(width, height = 900) {
    return this.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
  }

  async capture(name) {
    await this.eval(`document.fonts.ready`);
    const { data } = await this.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    const dir = join(dirname(args.state), 'screenshots');
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, `${name}.png`), Buffer.from(data, 'base64'));
    const context = await this.eval(`({ route: location.pathname, width: innerWidth, height: innerHeight, dpr: devicePixelRatio, scrollX, scrollY, theme: document.documentElement.dataset.theme || 'system', fontSize: getComputedStyle(document.documentElement).fontSize, userAgent: navigator.userAgent })`);
    const manifest = join(dir, 'contexts.json');
    const contexts = readdirSync(dir).includes('contexts.json') ? JSON.parse(readFileSync(manifest, 'utf8')) : {};
    contexts[name] = context;
    writeFileSync(manifest, JSON.stringify(contexts, null, 2));
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
    // Give native selectionchange and CodeMirror's focus observer a rendering opportunity
    // before the next synthetic input, as between separate human key/pointer actions.
    await this.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
  }

  /** Native document/line movement differs on macOS; don't edit a selection through the DOM. */
  lineStart(modifiers = 0) {
    return this.key(this.mac ? 'ArrowLeft' : 'Home', (this.mac ? META : 0) | modifiers);
  }

  lineEnd() {
    return this.key(this.mac ? 'ArrowRight' : 'End', this.mac ? META : 0);
  }

  /** Focuses CodeMirror, uses the platform document-end shortcut and inserts committed text. */
  async typeInBody(text) {
    await this.eval(`document.querySelector('[data-testid=editor-body] .cm-content').focus()`);
    await this.key(this.mac ? 'ArrowDown' : 'End', this.mod);
    if (text) await this.send('Input.insertText', { text });
  }

  /** Return as a keyboard user does, rather than assigning focus to contenteditable directly
   * (which lets Chrome put its DOM caret at the start before CodeMirror restores selection). */
  async tabToBody() {
    for (let i = 0; i < 16; i++) {
      if (await this.eval(`document.activeElement?.classList.contains('cm-content')`)) return;
      await this.key('Tab');
    }
    throw new Error('keyboard traversal did not reach the body editor');
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

/** Explicit dashboard fixtures and held POSTs. Real mode lets the server commit a POST,
 * then drops its 201 response: reconciliation must find that draft without replaying it. */
class DraftGate {
  constructor(page) {
    this.page = page;
    this.list = null;
    this.holdGets = false;
    this.gets = [];
    this.posts = [];
    this.postCount = 0;
    this.dropAck = false;
    this.droppedStatus = null;
    this.finishedPosts = 0;
    this.awaitingFinish = new Set();
    this.listener = message => {
      if (message.sessionId !== page.sessionId) return;
      if (message.method === 'Network.loadingFinished' && this.awaitingFinish.delete(message.params.requestId)) this.finishedPosts++;
      if (message.method !== 'Fetch.requestPaused') return;
      this.handle(message.params).catch(error => { this.error = error; });
    };
    page.cdp.listeners.push(this.listener);
  }

  async handle(params) {
    const { requestId, request, responseStatusCode, networkId } = params;
    if (responseStatusCode !== undefined) {
      if (request.method === 'POST') {
        if (networkId) this.awaitingFinish.add(networkId);
      }
      if (request.method === 'POST' && this.dropAck) {
        this.dropAck = false;
        this.droppedStatus = responseStatusCode;
        await this.page.send('Fetch.failRequest', { requestId, errorReason: 'ConnectionClosed' });
      } else await this.page.send('Fetch.continueRequest', { requestId });
    } else if (request.method === 'POST') {
      this.postCount++;
      this.posts.push(requestId);
    } else if (this.holdGets) this.gets.push(requestId);
    else await this.answerGet(requestId);
  }

  enable() {
    return this.page.send('Fetch.enable', { patterns: [
      { urlPattern: `${args.base}/api/v1/drafts`, requestStage: 'Request' },
      { urlPattern: `${args.base}/api/v1/drafts`, requestStage: 'Response' },
    ] });
  }

  async disable() {
    await this.page.send('Fetch.disable');
    this.page.cdp.listeners = this.page.cdp.listeners.filter(listener => listener !== this.listener);
    if (this.error) throw this.error;
  }

  async answerGet(requestId) {
    if (!this.list) return this.page.send('Fetch.continueRequest', { requestId });
    return this.fulfill(requestId, this.list.status ?? 200, this.list.body);
  }

  fulfill(requestId, status, body) {
    return this.page.send('Fetch.fulfillRequest', {
      requestId, responseCode: status,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify(body)).toString('base64'),
    });
  }

  async next(queue) {
    const deadline = Date.now() + 10000;
    while (!this[queue].length) {
      if (this.error) throw this.error;
      if (Date.now() > deadline) throw new Error(`timed out waiting for dashboard ${queue}`);
      await sleep(50);
    }
    return this[queue].shift();
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
  throw new Error(`no finished download; directory entries: ${JSON.stringify(readdirSync(dir))}`);
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
  await page.send('Network.enable');
  // Chrome may open its update/help tab on macOS after this target is created. Keep this
  // headless target visible/focused so keyboard events and animation frames can run.
  await page.send('Page.bringToFront');
  await page.send('Emulation.setFocusEmulationEnabled', { enabled: true });
  page.mac = await page.eval(`/Mac/.test(navigator.platform)`);
  page.mod = page.mac ? META : CTRL;
  return page;
}

async function serverHtml() {
  console.log('server HTML before any JavaScript:');
  const home = await fetch(args.base);
  const homeHtml = await home.text();
  check(home.status === 200 && homeHtml.includes('<title>Khám phá — Vink</title>') && homeHtml.includes('Bạn đã xem hết danh sách này.'), 'Explore HTML contains its finite public collection before JavaScript');
  check(homeHtml.includes(`href="/tac-pham/${SAMPLE_WORK}"`) && !homeHtml.includes(DRAFT_ONLY_LINE), 'Explore uses real reader links and excludes private draft text');
  for (const query of ['mua tren pho cu', 'TÁC GIẢ MẪU'.normalize('NFD')]) {
    const response = await fetch(`${args.base}/?q=${encodeURIComponent(query)}`);
    const filtered = await response.text();
    check(response.status === 200 && filtered.includes(`href="/tac-pham/${SAMPLE_WORK}?q=`) && !filtered.includes(DRAFT_ONLY_LINE), 'SSR finds the public title or author before JavaScript without exposing a draft');
    const canonical = filtered.match(/<link\b(?=[^>]*\brel="canonical")[^>]*>/)?.[0];
    check(canonical?.includes(`href="${args.base}/"`), 'Explore canonical excludes the search context');
  }
  const noResults = await fetch(`${args.base}/?q=khong-co-tac-pham-nay`);
  const noResultsHtml = await noResults.text();
  check(noResults.status === 200 && noResultsHtml.includes('data-testid="public-no-results"') && !noResultsHtml.includes('data-testid="public-empty"'), 'SSR distinguishes a filtered miss from an empty public collection');
  const reader = await fetch(`${args.base}/tac-pham/${SAMPLE_WORK}`);
  const html = await reader.text();
  check(reader.status === 200, 'reader responds 200');
  check(html.includes(PUBLISHED_LINE) && html.includes(INDENTED_LINE), 'reader HTML contains the published text');
  const canonicalTag = html.match(/<link\b(?=[^>]*\brel="canonical")[^>]*>/)?.[0];
  check(canonicalTag?.includes(`href="${args.base}/tac-pham/${SAMPLE_WORK}"`) && html.includes('<title>[Mẫu] Mưa trên phố cũ — Vink</title>') && html.includes('name="description" content="Phố cũ mưa giăng sợi mỏng,"'), 'reader HTML has exact title, description and canonical link');
  check(!html.includes(DRAFT_ONLY_LINE), 'reader HTML and hydration payload exclude the private draft');
  const desk = await (await fetch(`${args.base}/ban-viet/${SAMPLE_WORK}`)).text();
  check(!desk.includes(DRAFT_ONLY_LINE), 'desk HTML is a shell without draft text');
  const index = await (await fetch(`${args.base}/ban-viet`)).text();
  const privateList = await (await fetch(`${args.base}/api/v1/drafts`)).json();
  check(!index.includes(DRAFT_ONLY_LINE) && privateList.drafts.every(d => !d.title || !index.includes(d.title)), 'dashboard SSR contains no private title/body or list hydration payload');
  const anonymous = await fetch(`${args.base}/api/v1/drafts`);
  check(anonymous.status === 200, 'loopback identity fixture is active for this local run');

  // The runner creates a scratch database; this writes only that disposable sample draft.
  const draft = await (await fetch(`${args.base}/api/v1/drafts/${SAMPLE_WORK}`)).json();
  const publicBefore = await (await fetch(`${args.base}/api/v1/works/${SAMPLE_WORK}/publication`)).json();
  const privateTitle = 'Tiêu đề riêng sau khi sửa nháp — kiểm tra privacy';
  const updated = await fetch(`${args.base}/api/v1/drafts/${SAMPLE_WORK}`, {
    method: 'PUT', headers: { 'Content-Type': 'application/json', Origin: args.base },
    body: JSON.stringify({ expected_version: draft.version, title: privateTitle, document: { schema_version: 1, blocks: [{ lines: [DRAFT_ONLY_LINE] }] } }),
  });
  check(updated.status === 200, 'a private sample draft is really saved in the scratch PostgreSQL database');
  const publicAfter = await (await fetch(`${args.base}/api/v1/works/${SAMPLE_WORK}/publication`)).json();
  const afterHtml = await (await fetch(`${args.base}/tac-pham/${SAMPLE_WORK}`)).text();
  const afterHome = await (await fetch(args.base)).text();
  check(JSON.stringify(publicAfter) === JSON.stringify(publicBefore) && afterHtml.includes(PUBLISHED_LINE), 'saving the draft leaves the public API snapshot and SSR text unchanged');
  check([JSON.stringify(publicAfter), afterHtml, afterHome].every(text => !text.includes(privateTitle) && !text.includes(DRAFT_ONLY_LINE)), 'private edits stay out of public DTOs, HTML and hydration data');

  const created = await fetch(`${args.base}/api/v1/drafts`, { method: 'POST', headers: { Origin: args.base } });
  const privateWork = (await created.json()).work_id;
  check(created.status === 201, 'an unpublished work exists for the public access check');
  for (const id of [privateWork, '00000000-0000-4000-8000-000000000000', 'not-a-uuid']) {
    for (const path of [`/tac-pham/${id}`, `/api/v1/works/${id}/publication`]) {
      const missing = await fetch(`${args.base}${path}`);
      check(missing.status === 404, `${path} returns 404 for missing/unpublished content`);
    }
  }
}

const RAIL_WIDTH = `Math.round(document.querySelector('.vk-rail').getBoundingClientRect().width)`;
const PROFILE_OPEN = `document.querySelector('[data-testid=profile-menu]').open`;

async function openProfile(page) {
  await page.click('[data-testid=profile-trigger]');
  await page.waitFor(PROFILE_OPEN, 'Tôi overlay');
}

async function chooseTheme(page, value, group = 'overlay-theme') {
  await page.click(`label:has(input[name="${group}"][value="${value}"])`);
  await page.waitFor(value === 'system' ? `!document.documentElement.hasAttribute('data-theme')` : `document.documentElement.dataset.theme === '${value}'`, `${value} appearance`);
  // Token colours switch at once; nav background transitions take 160ms. Contrast samples
  // describe the settled theme, separately from those transient animation frames.
  await page.eval(`Promise.all(document.getAnimations().map(a => a.finished.catch(() => {})))`);
  await assertBrand(page, value);
}

async function assertBrand(page, preference) {
  const theme = preference === 'system'
    ? await page.eval(`matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'`)
    : preference;
  await page.waitFor(`document.querySelector('#vink-favicon').getAttribute('href') === '/pkg/brand/enso-${theme}.svg' && document.querySelector('#vink-manifest').getAttribute('href') === '/pkg/brand/manifest-${theme}.webmanifest'`, `${theme} head icons`);
  check(await page.eval(`[...document.querySelectorAll('.vk-brand__mark')].every(el => getComputedStyle(el).backgroundImage.includes('enso-${theme}-96.avif') && getComputedStyle(el).filter === 'none' && el.textContent === '')`), `${theme} rail/mobile marks use the approved asset without inversion or placeholder text`);
  check(await page.eval(`[...document.querySelectorAll('.vk-brand__mark')].filter(el => el.getClientRects().length).every(el => { const r = el.getBoundingClientRect(); return r.width === r.height && r.width === 2.25 * parseFloat(getComputedStyle(document.documentElement).fontSize); })`), `${theme} brand reserves its square before image decoding`);
}

async function brandAssets(page) {
  console.log('approved Enso asset delivery and decoding:');
  for (const theme of ['light', 'dark']) {
    for (const size of [24, 32, 48, 96, 192, 512]) {
      const path = `/pkg/brand/enso-${theme}-${size}.avif`;
      const response = await fetch(`${args.base}${path}`);
      check(response.ok && response.headers.get('content-type')?.startsWith('image/avif'), `${theme} ${size}px AVIF is served as an image`);
      check(await page.eval(`(async () => { const image = new Image(); image.src = '${path}'; await image.decode(); return image.naturalWidth === ${size} && image.naturalHeight === ${size}; })()`), `${theme} ${size}px AVIF decodes at the declared size`);
    }
    const response = await fetch(`${args.base}/pkg/brand/manifest-${theme}.webmanifest`);
    const manifest = await response.json();
    check(response.ok && manifest.icons.some(icon => icon.sizes === '192x192') && manifest.icons.some(icon => icon.sizes === '512x512'), `${theme} app manifest exposes 192/512 icons`);
    for (const icon of manifest.icons) {
      check((await fetch(`${args.base}${icon.src}`)).ok, `${theme} manifest icon ${icon.src} exists`);
    }
  }
  // Suppress only this app's hydration bundle to inspect the real server document
  // with CSS + appearance bootstrap. Restore it before any interaction checks.
  await page.send('Network.setBlockedURLs', { urls: [`${args.base}/pkg/vink.js*`] });
  for (const theme of ['light', 'dark']) {
    await page.eval(`localStorage.setItem('vink.ui.theme', '${theme}')`);
    await page.goto(`${args.base}/toi`);
    check(await page.eval(`document.documentElement.dataset.hydrated !== 'true' && document.documentElement.dataset.theme === '${theme}'`), `${theme} bootstrap selects the stored theme without hydration`);
    await assertBrand(page, theme);
    await page.capture(`brand-before-hydration-${theme}`);
  }
  await page.send('Network.setBlockedURLs', { urls: [] });
  await page.eval(`localStorage.removeItem('vink.ui.theme')`);
  await page.goto(`${args.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'hydration restored after brand bootstrap check');
}

async function textContrast(page, theme, surface = 'shell') {
  const selectors = {
    desk: ['.vk-resume__title', '.vk-resume .vk-draft-meta', '.vk-resume__action', '.vk-desk-heading .vk-button', '.vk-draft-row__title', '.vk-draft-row .vk-draft-meta', '.vk-draft-row__open', '.vk-draft-tools input', '#draft-scope'],
    explore: ['.vk-explore__title', '.vk-explore__heading p', '#public-scope', '.vk-public-row__title', '.vk-public-row__author', '.vk-public-row time', '.vk-public-row__open', '#writing-prompt h2', '#writing-prompt p', '#writing-prompt .vk-button', '.vk-reading-note'],
    reader: ['#reader-return', '.vk-reader__title', '.vk-reader__byline', '.vk-reader .vk-eyebrow', '.vk-reader .vk-block'],
  }[surface] ?? ['.vk-nav a', '.vk-account-trigger', '#profile-menu .vk-muted', '#profile-menu .vk-theme-choice:has(:checked) span', '#profile-menu .vk-button'];
  const pairs = await page.eval(`(() => {
    const rgb = value => value.match(/[\\d.]+/g).map(Number);
    const luminance = color => rgb(color).slice(0, 3).map(v => v / 255).map(v => v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4).reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i], 0);
    const background = el => {
      for (; el; el = el.parentElement) {
        const color = getComputedStyle(el).backgroundColor;
        const alpha = rgb(color)[3] ?? 1;
        if (alpha === 1) return color;
        if (alpha !== 0) throw new Error('contrast requires alpha composition');
      }
      throw new Error('missing opaque background');
    };
    return ${JSON.stringify(selectors)}.map(selector => {
      const el = document.querySelector(selector);
      const foreground = getComputedStyle(el).color;
      const bg = background(el);
      const [a, b] = [luminance(foreground), luminance(bg)].sort((a, b) => b - a);
      return { selector, foreground, background: bg, ratio: (a + 0.05) / (b + 0.05) };
    });
  })()`);
  writeFileSync(join(dirname(args.state), `${surface}-contrast-${theme}.json`), JSON.stringify(pairs, null, 2));
  check(pairs.every(p => p.ratio >= 4.5), `${theme} ${surface} text contrast >= 4.5 (min ${Math.min(...pairs.map(p => p.ratio)).toFixed(2)})`);
}

async function adaptiveShell(page) {
  console.log('adaptive navigation and appearance:');
  await page.viewport(1440);
  await page.goto(`${args.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'shell hydration');
  await brandAssets(page);
  check(await page.eval(`location.pathname === '/' && [...document.querySelectorAll('.vk-nav a')].map(a => a.textContent).join('|') === 'Khám phá|Bàn viết'`), 'launch route stays /, Khám phá precedes Bàn viết');
  check((await page.eval(RAIL_WIDTH)) === 220, 'desktop defaults to a 220px rail');
  await page.click('[data-testid=rail-toggle]');
  check((await page.eval(RAIL_WIDTH)) === 96, 'manual compact rail is 96px');
  check(await page.eval(`getComputedStyle(document.querySelector('.vk-nav a')).flexDirection === 'column'`), 'compact rail places icons above labels');
  await page.key('Escape');
  check(await page.eval(`document.querySelector('[role=tooltip]').hidden`), 'Escape dismisses the rail tooltip');
  await page.eval(`document.querySelector('[data-testid=rail-toggle]').blur()`);
  await page.eval(`document.querySelector('[data-testid=rail-toggle]').focus()`);
  check(await page.eval(`!document.querySelector('[role=tooltip]').hidden`), 'focus exposes the rail tooltip');
  await page.key('Escape');
  await page.eval(`document.querySelector('[data-testid=rail-toggle]').blur()`);
  await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1000, y: 20 });
  const hover = await page.eval(`(() => { const r = document.querySelector('[data-testid=rail-toggle]').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...hover });
  await page.waitFor(`!document.querySelector('[role=tooltip]').hidden`, 'hover tooltip');
  check(await page.eval(`(() => { const r = document.querySelector('[role=tooltip]').getBoundingClientRect(); return r.left >= document.querySelector('.vk-rail').getBoundingClientRect().right && r.right <= innerWidth; })()`), 'hover tooltip is outside the scrolling rail and within the viewport');
  await page.capture('1440-rail-tooltip-light');
  await page.key('Escape');
  check(await page.eval(`document.querySelector('[role=tooltip]').hidden`), 'Escape also dismisses a hover tooltip without trigger focus');
  await page.reload();
  check((await page.eval(RAIL_WIDTH)) === 96, 'rail preference survives reload');
  await page.viewport(1024);
  check((await page.eval(RAIL_WIDTH)) === 96, 'manual compact preference survives resize');
  await page.click('[data-testid=rail-toggle]');
  check((await page.eval(RAIL_WIDTH)) === 220, 'manual expanded preference overrides tablet default');
  await page.eval(`localStorage.removeItem('vink.ui.rail')`);
  await page.reload();
  check((await page.eval(RAIL_WIDTH)) === 96, 'tablet defaults to a compact rail');
  await page.viewport(1440);
  await page.waitFor(`${RAIL_WIDTH} === 220`, 'desktop contextual default');

  await openProfile(page);
  check(await page.eval(`document.activeElement?.name === 'overlay-theme' && document.activeElement.value === 'system'`), 'overlay initially focuses the selected appearance option');
  check(await page.eval(`document.querySelector('#profile-menu').textContent.includes('Chưa có thông tin tài khoản') && !document.querySelector('#profile-menu').textContent.includes('An Nhiên')`), 'account utility reports unavailable identity honestly');
  await page.eval(`document.querySelector('[data-testid=profile-close]').focus()`);
  await page.key('Tab', SHIFT);
  check(await page.eval(`document.activeElement?.dataset.testid === 'open-me'`), 'Shift+Tab is contained at the beginning of the modal');
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.dataset.testid === 'profile-close'`), 'Tab is contained at the end of the modal');
  await chooseTheme(page, 'dark');
  await textContrast(page, 'dark');
  await page.capture('1440-overlay-dark');
  await page.key('Escape');
  check(await page.eval(`!${PROFILE_OPEN} && document.activeElement?.dataset.testid === 'profile-trigger'`), 'Escape closes Tôi and restores trigger focus');
  await page.reload();
  check(await page.eval(`document.documentElement.dataset.theme === 'dark'`), 'theme preference survives reload');
  await assertBrand(page, 'dark');
  await openProfile(page);
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: 1400, y: 20, button: 'left', clickCount: 1 });
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 1400, y: 20, button: 'left', clickCount: 1 });
  check(await page.eval(`!${PROFILE_OPEN} && document.activeElement?.dataset.testid === 'profile-trigger'`), 'outside click closes Tôi and restores focus');
  await openProfile(page);
  await page.click('[data-testid=profile-close]');
  check(await page.eval(`!${PROFILE_OPEN}`), 'explicit close works');
  await openProfile(page);
  await page.click('[data-testid=open-me]');
  await page.waitFor(`location.pathname === '/toi' && document.querySelector('#me-title')`, 'real Tôi destination');
  check(await page.eval(`document.querySelector('[data-testid=profile-trigger]').getAttribute('aria-current') === 'page'`), 'Tôi route marks the rail utility active');
  await openProfile(page);
  await page.viewport(390);
  await page.waitFor(`!${PROFILE_OPEN}`, 'resize closes overlay');
  check(await page.eval(`location.pathname === '/toi' && document.activeElement?.id === 'noi-dung'`), 'resize retains route and moves focus to the visible main');
  check(await page.eval(`[...document.querySelectorAll('.vk-bottom-nav a')].map(a => a.textContent).join('|') === 'Khám phá|Bàn viết|Tôi'`), 'mobile destination order is Khám phá, Bàn viết, Tôi');

  await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] });
  await chooseTheme(page, 'system', 'page-theme');
  const darkSurface = await page.eval(`getComputedStyle(document.body).backgroundColor`);
  await assertBrand(page, 'dark');
  await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
  await page.waitFor(`getComputedStyle(document.body).backgroundColor !== ${JSON.stringify(darkSurface)}`, 'system appearance follows OS changes');
  await assertBrand(page, 'light');
  await chooseTheme(page, 'dark', 'page-theme');
  check((await page.eval(`getComputedStyle(document.body).backgroundColor`)) === darkSurface, 'explicit dark overrides a light system');
  await chooseTheme(page, 'light', 'page-theme');
  await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] });
  check((await page.eval(`getComputedStyle(document.body).backgroundColor`)) !== darkSurface, 'explicit light overrides a dark system');
  await page.send('Emulation.setEmulatedMedia', { features: [] });
  await chooseTheme(page, 'system', 'page-theme');

  // Bootstrap and interactive writes must both tolerate browser privacy/storage denial.
  const { identifier } = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `for (const method of ['getItem','setItem','removeItem']) Storage.prototype[method] = () => { throw new DOMException('Storage denied', 'SecurityError'); };` });
  await page.reload();
  await chooseTheme(page, 'dark', 'page-theme');
  await page.viewport(1440);
  await page.click('[data-testid=rail-toggle]');
  check(await page.eval(`document.documentElement.dataset.theme === 'dark' && document.documentElement.dataset.rail === 'compact'`), 'storage-denied theme and rail still work in memory');
  await assertBrand(page, 'dark');
  await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier });
  await page.reload();
  await page.eval(`localStorage.removeItem('vink.ui.rail'); localStorage.removeItem('vink.ui.theme')`);
  await page.reload();

  for (const width of [1440, 1024, 390, 320]) {
    await page.viewport(width);
    for (const theme of ['light', 'dark']) {
      if (width >= 760) {
        await openProfile(page);
        await chooseTheme(page, theme);
        if (width === 1440) await textContrast(page, theme);
        await page.key('Escape');
      } else {
        await chooseTheme(page, theme, 'page-theme');
      }
      await page.capture(`${width}-me-${theme}`);
      check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), `${width}px ${theme}: no horizontal overflow`);
    }
  }
  await page.eval(`document.documentElement.style.fontSize = '200%'`);
  await page.capture('320-me-dark-text200');
  check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), '320px / 200% text reflows without horizontal overflow');
  await page.eval(`scrollTo(0, document.documentElement.scrollHeight)`);
  await page.waitFor(`parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--vk-bottom-height')) === document.querySelector('.vk-bottom-nav').getBoundingClientRect().height`, 'measured bottom clearance');
  check(await page.eval(`document.querySelector('.vk-footer').getBoundingClientRect().bottom <= document.querySelector('.vk-bottom-nav').getBoundingClientRect().top`), 'bottom bar clears the end of page content at 320px / 200% text');
  await page.capture('320-me-dark-text200-bottom');
  await page.click('.vk-bottom-nav a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('[data-testid=create-draft]')`, 'mobile desk destination');
  await page.capture('320-desk-dark-text200');
  await page.click('.vk-bottom-nav a[href="/"]');
  await page.waitFor(`location.pathname === '/'`, 'mobile discovery destination');
  await page.click('.vk-bottom-nav a[href="/toi"]');
  await page.waitFor(`location.pathname === '/toi'`, 'mobile Tôi screen');
  await chooseTheme(page, 'system', 'page-theme');
  await page.eval(`document.documentElement.style.fontSize = ''; scrollTo(0, 0)`);
  await page.viewport(1280);
}

async function shellWithDirtyEditor(page, editorPath, written, view, selected) {
  console.log('shell utilities preserve the live dirty editor:');
  let writes = 0;
  const listener = (event) => {
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent' && ['PUT', 'POST'].includes(event.params.request.method)) writes++;
  };
  page.cdp.listeners.push(listener);
  check((await page.eval(RAIL_WIDTH)) === 96, 'desktop editor defaults to compact');
  await page.click('[data-testid=rail-toggle]');
  check((await page.eval(RAIL_WIDTH)) === 220, 'manual preference can expand the editor rail');
  await page.click('[data-testid=rail-toggle]');
  await page.tabToBody();
  await page.waitFor(`${selected} === ${JSON.stringify(LAST_LINE)}`, 'selection after rail changes');
  await openProfile(page);
  check(!(await page.dialogOpen('leave-editor')), 'opening Tôi utility does not invoke the leave guard');
  await chooseTheme(page, 'dark');
  await page.capture('1280-editor-dirty-overlay-dark');
  await page.key('Escape');
  await page.tabToBody();
  await page.waitFor(`${selected} === ${JSON.stringify(LAST_LINE)}`, 'selection after appearance utility');
  await page.eval(`(() => { const content = document.querySelector('.cm-content'); content.blur(); content.focus(); })()`);
  check((await page.eval(selected)) === LAST_LINE, 'native content focus immediately restores the CodeMirror selection');
  check(await page.eval(view) && (await page.eval(selected)) === LAST_LINE && (await page.body()) === written && (await page.status()) === 'unsaved' && (await page.eval(`document.querySelector('input[name=title]').value`)) === TITLE, 'rail, overlay and theme preserve EditorView, selection, text and dirty state');

  for (const destination of ['/', '/ban-viet', '/toi']) {
    if (destination === '/toi') {
      await openProfile(page);
      await page.click('[data-testid=open-me]');
    } else {
      await page.click(`.vk-nav a[href="${destination}"]`);
    }
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, `dirty guard to ${destination}`);
    check(await page.eval(`!${PROFILE_OPEN} && location.pathname === '${editorPath}'`), `real ${destination} navigation consults the existing guard`);
    await page.key('Escape');
  }
  await openProfile(page);
  await page.viewport(390);
  await page.waitFor(`!${PROFILE_OPEN}`, 'editor overlay closes on mobile resize');
  check(await page.eval(view) && (await page.body()) === written && await page.eval(`location.pathname === '${editorPath}' && getComputedStyle(document.querySelector('.vk-bottom-nav')).display === 'none'`), 'mobile resize retains editor and hides bottom bar');
  await page.capture('390-editor-dirty-dark');
  await page.click('.vk-mobile-brand');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'mobile brand uses guard');
  await page.key('Escape');
  await page.viewport(1280);
  await page.tabToBody();
  await page.waitFor(`${selected} === ${JSON.stringify(LAST_LINE)}`, 'selection after mobile resize');
  check((await page.eval(selected)) === LAST_LINE && writes === 0, 'utility changes sent no draft writes and retained the caret selection');
  page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener), 1);
  await openProfile(page);
  await chooseTheme(page, 'system');
  await page.key('Escape');
  await page.tabToBody();
  await page.waitFor(`${selected} === ${JSON.stringify(LAST_LINE)}`, 'selection after returning to system appearance');
}

async function draftDashboard(page) {
  console.log('private dashboard: explicit list fixtures, search, states and non-idempotent creation:');
  const gate = new DraftGate(page);
  let manuscriptRequests = 0;
  const countManuscripts = message => {
    if (message.sessionId === page.sessionId && message.method === 'Network.requestWillBeSent'
        && new URL(message.params.request.url).pathname.startsWith('/api/v1/drafts/')) manuscriptRequests++;
  };
  page.cdp.listeners.push(countManuscripts);
  const longTitle = '[Fixture] Đường về — Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ tiếng Việt và những khổ thơ còn đang viết dở, dưới mái hiên của căn nhà nhỏ bên sông';
  const cards = Array.from({ length: 100 }, (_, i) => ({
    work_id: `88888888-8888-4888-8888-${String(i).padStart(12, '0')}`,
    title: i === 0 ? longTitle.normalize('NFD') : i === 1 ? '[Fixture] ĐƯỜNG VỀ CÓ NẮNG' : i === 2 ? '' : `[Fixture] Bản thảo ${i}`,
    version: i + 1, updated_at: '2026-11-01T08:30:00Z',
  }));
  gate.list = { body: { drafts: cards } };
  gate.holdGets = true;
  await gate.enable();
  await page.viewport(1440);
  await page.goto(`${args.base}/ban-viet`);
  const initial = await gate.next('gets');
  await page.waitFor(`document.querySelector('[data-testid=draft-loading]')`, 'initial dashboard loading');
  check(await page.eval(`document.querySelector('[data-testid=draft-content]').getAttribute('aria-busy') === 'true' && !document.querySelector('[data-testid=draft-empty]') && !document.querySelector('[data-testid=create-draft]')`), 'loading is distinct from empty and cannot create');
  gate.holdGets = false;
  await gate.answerGet(initial);
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 100`, '100 loaded rows');
  const resumeHref = `/ban-viet/${cards[0].work_id}`;
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resumeHref)} && document.querySelector('.vk-resume__title').textContent === ${JSON.stringify(cards[0].title)}`), 'resume opens exactly the first authorized card and preserves its full combining title');
  check(await page.eval(`document.querySelector('[data-testid=draft-scope]').textContent.includes('100 bản thảo gần đây nhất') && document.querySelector('[data-testid=draft-scope]').textContent.includes('có thể còn')`), '100 loaded rows are not claimed as a global total');
  check(await page.eval(`[...document.querySelectorAll('[data-testid=draft-row]')].every((row, i) => row.querySelector('a').getAttribute('href') === '/ban-viet/' + ${JSON.stringify(cards)}[i].work_id && row.textContent.includes('Phiên bản ' + (i + 1)) && row.querySelector('time').dateTime === '2026-11-01T08:30:00Z')`), 'every row shows the provider work_id, version and update timestamp');
  check(manuscriptRequests === 0, 'list/resume never fetch full manuscripts (no N+1)');
  check(await page.eval(`!document.querySelector('#desk').textContent.match(/Chưa đăng|Đã đăng|góp ý|thể loại|D-05|VINK_DEV_PRINCIPAL/)`), 'no invented publication/profile/feedback state or implementation copy');
  await page.eval(`document.querySelector('[data-testid=resume-draft]').focus()`);
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.dataset.testid === 'draft-search' && document.activeElement.matches(':focus-visible')`), 'keyboard order moves from Viết tiếp to search with visible focus');
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.dataset.testid === 'draft-sort'`), 'sort follows search in keyboard order');
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme = '${theme}'`);
    await page.eval(`Promise.all(document.getAnimations().map(animation => animation.finished.catch(() => {})))`);
    await textContrast(page, theme, 'desk');
    for (const width of [1440, 1024, 390, 320]) {
      await page.viewport(width);
      await page.eval(`scrollTo(0, 0)`);
      await page.capture(`desk-dashboard-${width}-${theme}`);
      if (width === 320) {
        await page.eval(`document.querySelector('#drafts').scrollIntoView({ block: 'start' })`);
        await page.capture(`desk-list-320-${theme}`);
      }
      check(await page.eval(`document.documentElement.scrollWidth <= innerWidth && document.querySelector('#resume').getBoundingClientRect().top < document.querySelector('#drafts').getBoundingClientRect().top`), `${theme} ${width}px dashboard reflows with resume before list`);
    }
    await page.eval(`document.documentElement.style.fontSize = '200%'`);
    const wholeWords = await page.eval(`(() => {
      const title = document.querySelector('.vk-resume__title');
      return ['Đường', 'Những'].every(word => {
        word = word.normalize('NFD');
        const node = [...title.childNodes].find(node => node.nodeType === Node.TEXT_NODE && node.textContent.includes(word));
        if (!node) return false;
        const start = node.textContent.indexOf(word);
        const range = document.createRange();
        range.setStart(node, start); range.setEnd(node, start + word.length);
        const rects = [...range.getClientRects()];
        return rects.length > 0 && rects.every(rect => Math.abs(rect.top - rects[0].top) < 1);
      });
    })()`);
    check(wholeWords, `${theme} 320px / 200% text keeps common Vietnamese title words on one line`);
    await page.eval(`document.querySelector('.vk-resume__title').scrollIntoView({ block: 'start' })`);
    await page.capture(`desk-dashboard-320-200-${theme}`);
    check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), `${theme} dashboard keeps 320px width at 200% text`);
    await page.eval(`document.documentElement.style.fontSize = ''`);
  }
  await page.viewport(1440);
  const search = async text => {
    await page.eval(`document.querySelector('[data-testid=draft-search]').focus(); document.querySelector('[data-testid=draft-search]').select()`);
    await page.send('Input.insertText', { text });
  };
  await search('duong ve');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 2`, 'accentless Vietnamese results');
  await search('ĐƯỜNG VỀ CÓ NẮNG'.normalize('NFD'));
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 1`, 'combining accented results');
  await search('không có tên này');
  await page.waitFor(`document.querySelector('[data-testid=draft-no-results]')`, 'no results');
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resumeHref)}`), 'search never changes the most recent resume target');
  await page.click('[data-testid=clear-draft-search]');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 100`, 'clear query');
  await page.eval(`const sort = document.querySelector('[data-testid=draft-sort]'); sort.value = 'title'; sort.dispatchEvent(new Event('change', { bubbles: true }))`);
  await page.waitFor(`document.querySelector('[data-testid=draft-row]').textContent.includes('[Fixture] Bản thảo 10')`, 'title sort');
  await search('duong ve');
  gate.list = { status: 503, body: { error: { code: 'dependency_unavailable', message: 'test fixture' } } };
  await page.click('[data-testid=refresh-drafts]');
  await page.waitFor(`document.querySelector('[data-testid=desk-error]')`, 'dependency failure');
  check(await page.eval(`!document.querySelector('[data-testid=draft-empty]') && !document.querySelector('[data-testid=draft-row]')`), 'load failure never becomes an empty or stale draft list');
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme = '${theme}'`);
    for (const width of [1440, 320]) {
      await page.viewport(width);
      await page.eval(`scrollTo(0, 0)`);
      await page.capture(`desk-error-${width}-${theme}`);
    }
  }
  gate.list = { body: { drafts: cards } };
  await page.click('[data-testid=retry-drafts]');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 2`, 'retry with preserved query');
  check(await page.eval(`document.querySelector('[data-testid=draft-search]').value === 'duong ve' && document.querySelector('[data-testid=draft-sort]').value === 'title'`), 'query and sort survive failed refresh and retry');
  gate.list = { status: 401, body: { error: { code: 'unauthenticated', message: 'test fixture' } } };
  await page.click('[data-testid=refresh-drafts]');
  await page.waitFor(`document.querySelector('[data-testid=desk-error]')?.textContent.includes('chưa đăng nhập')`, 'unauthenticated dashboard');
  check(await page.eval(`!document.querySelector('[data-testid=create-draft]') && document.querySelector('[data-testid=desk-error] a').getAttribute('href') === '/'`), '401 has an honest public-reading action and no fake login/create');
  gate.list = { body: { drafts: [] } };
  await page.click('[data-testid=retry-drafts]');
  await page.waitFor(`document.querySelector('[data-testid=draft-empty]')`, 'empty dashboard');
  check(await page.eval(`document.querySelectorAll('[data-testid=create-draft]').length === 1 && document.querySelector('[data-testid=create-draft]').textContent === 'Viết bản thảo đầu tiên' && !document.querySelector('[data-testid=draft-resume]')`), 'empty has exactly one primary create action');
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme = '${theme}'`);
    for (const width of [1440, 320]) {
      await page.viewport(width);
      await page.eval(`scrollTo(0, 0)`);
      await page.capture(`desk-empty-${width}-${theme}`);
    }
  }
  check(manuscriptRequests === 0, 'search, sort and refresh never prefetch manuscripts');
  page.cdp.listeners = page.cdp.listeners.filter(listener => listener !== countManuscripts);
  await page.click('[data-testid=create-draft]');
  const rejected = await gate.next('posts');
  await gate.fulfill(rejected, 403, { error: { code: 'forbidden', message: 'test fixture' } });
  await page.waitFor(`document.querySelector('[data-testid=create-notice]')?.textContent.includes('bị từ chối')`, 'explicit create rejection');
  check(await page.eval(`!document.querySelector('[data-testid=create-notice]').textContent.includes('có thể đã')`), 'pre-write rejection stays distinct from an uncertain outcome');
  check(await page.eval(`document.querySelector('[data-testid=create-draft]').disabled`), 'permission rejection requires an access recheck before another create');
  // Return to the real store for the commit + lost response case.
  gate.list = null;
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.querySelector('[data-testid=resume-draft]')`, 'real authorized list');
  const before = await page.eval(`fetch('/api/v1/drafts').then(r => r.json()).then(list => list.drafts.map(d => d.work_id))`);
  await page.click('[data-testid=create-draft]');
  const lost = await gate.next('posts');
  const postsBefore = gate.postCount;
  await page.eval(`for (let i = 0; i < 5; i++) document.querySelector('[data-testid=create-draft]').click()`);
  check(await page.eval(`document.querySelector('[data-testid=create-draft]').disabled && document.querySelector('[data-testid=refresh-drafts]').disabled`), 'create and refresh are disabled during the single POST');
  gate.dropAck = true;
  await page.send('Fetch.continueRequest', { requestId: lost });
  await page.waitFor(`document.querySelector('[data-testid=create-notice]')?.textContent.includes('có thể đã được tạo')`, 'uncertain create after lost 201');
  check(gate.droppedStatus === 201 && gate.postCount === postsBefore, 'one real POST committed and its ack was dropped without duplicate submissions');
  check(await page.eval(`document.querySelector('[data-testid=create-draft]').disabled && location.pathname === '/ban-viet'`), 'uncertain creation stays on desk and blocks another POST');
  // Failed reconciliation must keep creation blocked.
  gate.list = { status: 503, body: { error: { code: 'dependency_unavailable', message: 'test fixture' } } };
  await page.click('[data-testid=reconcile-drafts]');
  await page.waitFor(`document.querySelector('[data-testid=desk-error]')`, 'failed reconciliation');
  check(await page.eval(`document.querySelector('[data-testid=create-notice]').textContent.includes('có thể đã được tạo')`), 'failed reconciliation preserves the unknown outcome');
  gate.list = null;
  await page.click('[data-testid=reconcile-drafts]');
  await page.waitFor(`document.querySelector('[data-testid=create-draft]')?.textContent === 'Tạo thêm bản thảo'`, 'successful list reconciliation');
  const after = await page.eval(`fetch('/api/v1/drafts').then(r => r.json()).then(list => list.drafts.map(d => d.work_id))`);
  const created = after.filter(id => !before.includes(id));
  check(created.length === 1 && gate.postCount === postsBefore, 'GET reconciliation found the committed draft without retrying POST');
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === '/ban-viet/' + ${JSON.stringify(created[0])}`), 'newly reconciled draft is the resume target');
  await page.click('[data-testid=resume-draft]');
  await page.waitFor(`${EDITOR_READY} && location.pathname === '/ban-viet/${created[0]}'`, 'resume opens the real draft');
  check((await page.status()) === 'saved', 'reconciled draft opens at its real saved version');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('[data-testid=resume-draft]')`, 'return to dashboard');
  await page.viewport(1440);
  await page.click('[data-testid=create-draft]');
  const late = await gate.next('posts');
  const finishedBefore = gate.finishedPosts;
  await page.click('.vk-nav a[href="/"]');
  await page.waitFor(`location.pathname === '/'`, 'leave while create is pending');
  await page.send('Fetch.continueRequest', { requestId: late });
  const lateDeadline = Date.now() + 10000;
  while (gate.finishedPosts === finishedBefore && Date.now() < lateDeadline) await sleep(50);
  await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
  await page.waitFor(`document.querySelector('#explore')`, 'home after late create reply');
  check(gate.finishedPosts === finishedBefore + 1 && await page.eval(`location.pathname === '/'`), 'late successful create response does not navigate away from another route');
  await gate.disable();
  await page.viewport(1440);
  await page.eval(`document.documentElement.removeAttribute('data-theme')`);
}

async function publicReading(page) {
  console.log('public Explore and quiet reader: real navigation, then explicitly labelled HTTP fixtures:');
  const searchPublic = async query => {
    await page.eval(`document.querySelector('#public-query').focus(); document.querySelector('#public-query').select()`);
    await page.send('Input.insertText', { text: query });
    await page.click('#public-search button[type=submit]');
    await page.waitFor(`new URL(location.href).searchParams.get('q') === ${JSON.stringify(query)} && document.querySelector('#public-query')?.value === ${JSON.stringify(query)}`, 'submitted public search URL');
  };
  const setPublicTheme = async theme => {
    await page.eval(`document.documentElement.dataset.theme = '${theme}'`);
    await page.eval(`Promise.all(document.getAnimations().map(a => a.finished.catch(() => {})))`);
  };
  const captureReal = async surface => {
    for (const theme of ['light', 'dark']) {
      await setPublicTheme(theme);
      for (const width of [1440, 390]) {
        await page.viewport(width);
        await page.eval(`scrollTo(0, 0)`);
        await page.capture(`${surface}-real-${width}-${theme}`);
      }
    }
    await page.viewport(1440);
  };
  await page.viewport(1440);
  await page.goto(`${args.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('.vk-public-row__link')`, 'real Explore hydration');
  check(await page.eval(`document.querySelectorAll('.vk-public-row').length === 1 && document.querySelector('.vk-public-row__link').getAttribute('href') === '/tac-pham/${SAMPLE_WORK}'`), 'one real publication is shown once, with its exact reader link');
  await captureReal('explore');
  await page.eval(`window.__publicNavigation = 'kept'; document.querySelector('.vk-public-row__link').focus()`);
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('#writing-prompt a:focus-visible')`), 'keyboard moves from the real read link to the secondary writing action');
  await page.key('Tab', SHIFT);
  check(await page.eval(`document.activeElement.matches('.vk-public-row__link:focus-visible')`), 'read link has visible keyboard focus');
  await page.click('.vk-public-row__link');
  await page.waitFor(`document.querySelector('.vk-reader')?.textContent.includes(${JSON.stringify(PUBLISHED_LINE)})`, 'real reader from collection');
  check(await page.eval(`window.__publicNavigation === 'kept' && document.querySelector('#reader-return').getAttribute('href') === '/#work-${SAMPLE_WORK}' && document.querySelector('.vk-nav a[href="/"]').getAttribute('aria-current') === 'page'`), 'reader navigation is hydrated, belongs to Explore and returns to the opened row');
  await captureReal('reader');
  await page.click('#reader-return');
  await page.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row')`, 'return to real collection');
  const realQuery = 'mua tren pho cu';
  await searchPublic(realQuery);
  await page.click('.vk-public-row__link');
  await page.waitFor(`document.querySelector('#reader-return') && new URL(location.href).searchParams.get('q') === '${realQuery}'`, 'reader carries the real public query');
  await page.reload();
  check(await page.eval(`document.querySelector('link[rel=canonical]').href === '${args.base}/tac-pham/${SAMPLE_WORK}'`), 'reader reload keeps a public canonical without query context');
  await page.eval(`history.back()`);
  await page.waitFor(`document.querySelector('#public-query')?.value === '${realQuery}' && document.activeElement?.closest('.vk-public-row')?.dataset.workId === '${SAMPLE_WORK}'`, 'Back restores query and focus at the opened row');
  await page.eval(`history.forward()`);
  await page.waitFor(`document.querySelector('#reader-return') && location.pathname === '/tac-pham/${SAMPLE_WORK}'`, 'Forward restores the reader');
  await page.click('#reader-return');
  await page.waitFor(`document.querySelector('#public-query')?.value === '${realQuery}'`, 'reader return preserves submitted query');
  await page.reload();
  await page.waitFor(`document.querySelectorAll('.vk-public-row').length === 1 && document.activeElement?.closest('.vk-public-row')?.dataset.workId === '${SAMPLE_WORK}'`, 'Explore reload restores the real filtered row');
  check(await page.eval(`document.querySelector('#public-query').value === '${realQuery}' && window.__publicNavigation === undefined`), 'a new hydrated document retains query from its URL');
  await page.click('#clear-public-search');
  await page.waitFor(`!location.search && document.querySelector('#public-query')?.value === ''`, 'clear public search');

  const longTitle = '[Mẫu kiểm tra] Đường về — Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ tiếng Việt và những khổ thơ còn đang viết dở';
  const cards = Array.from({ length: 12 }, (_, i) => ({
    work_id: `77777777-7777-4777-8777-${String(i).padStart(12, '0')}`,
    title: i === 0 ? longTitle.normalize('NFD') : `[Mẫu kiểm tra] Một trang viết ${i}`,
    author_display_name: i % 2 ? '[Mẫu kiểm tra] Trần Linh' : '[Mẫu kiểm tra] Nguyễn Hoài Thương',
    published_at: '2026-11-01T08:30:00Z',
  }));
  const publicationPath = `/api/v1/works/${cards[0].work_id}/publication`;
  const literal = '<script>window.__manuscriptExecuted = true</script> & chữ Việt';
  const publication = {
    ...cards[0], revision_id: '99999999-9999-4999-8999-999999999999',
    document: { schema_version: 1, blocks: [
      { lines: [PUBLISHED_LINE, INDENTED_LINE, literal] },
      { lines: ['[Mẫu kiểm tra] Đoạn văn dài với đầy đủ ỹ ỷ ữ ợ Đ đ. ' + 'Một câu văn đi qua mùa mưa. '.repeat(30)] },
      { lines: ['đêm'.repeat(120)] },
    ] },
  };
  const answers = new Map([
    ['/api/v1/works', { status: 200, body: { works: cards } }],
    [publicationPath, { status: 200, body: publication }],
  ]);
  let holdList = false;
  let heldList;
  let publicationRequests = 0;
  let listRequests = 0;
  let gateError;
  const respond = (requestId, answer) => page.send('Fetch.fulfillRequest', {
    requestId, responseCode: answer.status,
    responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
    body: Buffer.from(JSON.stringify(answer.body)).toString('base64'),
  });
  const listener = message => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    const { requestId, request } = message.params;
    const path = new URL(request.url).pathname;
    if (path.endsWith('/publication')) publicationRequests++;
    if (path === '/api/v1/works') listRequests++;
    if (path === '/api/v1/works' && holdList) { heldList = requestId; return; }
    const answer = answers.get(path);
    (answer ? respond(requestId, answer) : page.send('Fetch.continueRequest', { requestId })).catch(error => { gateError = error; });
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: `${args.base}/api/v1/works*`, requestStage: 'Request' }] });
  const openCollection = async () => {
    await page.goto(`${args.base}/toi`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'utility hydration before public client read');
    const nav = await page.eval(`innerWidth < 760`) ? '.vk-bottom-nav a[href="/"]' : '.vk-nav a[href="/"]';
    await page.click(nav);
  };
  holdList = true;
  await openCollection();
  await page.waitFor(`document.querySelector('#community-stream [role=status]')`, 'public list loading');
  check(await page.eval(`!document.querySelector('[data-testid=public-empty]') && !document.querySelector('[data-testid=public-unavailable]')`), 'public loading is separate from empty and unavailable');
  const pendingDeadline = Date.now() + 10000;
  while (!heldList && Date.now() < pendingDeadline) await sleep(50);
  if (!heldList) throw new Error('public list request was not held');
  holdList = false;
  await respond(heldList, answers.get('/api/v1/works'));
  await page.waitFor(`document.querySelectorAll('.vk-public-row').length === 12`, 'finite public fixture collection');
  check(publicationRequests === 0, 'Explore never fetches full manuscripts to populate cards (no N+1)');
  check(await page.eval(`new Set([...document.querySelectorAll('.vk-public-row__link')].map(a => a.href)).size === 12 && document.querySelector('#feed-end') && document.querySelector('.vk-public-row time').getAttribute('datetime') === '2026-11-01T08:30:00Z'`), 'all 12 distinct public fields/links and the finite endpoint are rendered');
  check(await page.eval(`!document.querySelector('#explore').textContent.match(/tuyển chọn|Dành cho bạn|bình luận|bookmark|AI|lượt đọc/) && !document.querySelector('#explore select') && document.querySelector('#public-scope').textContent.includes('Không tìm toàn bộ thư viện')`), 'search discloses its finite scope without fabricated metadata, filters or social actions');
  const loadedRequests = listRequests;
  await searchPublic('duong ve');
  await page.waitFor(`document.querySelectorAll('.vk-public-row').length === 1`, 'unaccented public title match');
  check(await page.eval(`document.querySelector('.vk-public-row__title').textContent === ${JSON.stringify(cards[0].title)}`), 'search preserves the authored NFD title');
  await searchPublic('TRẦN LINH'.normalize('NFD'));
  await page.waitFor(`document.querySelectorAll('.vk-public-row').length === 6`, 'decomposed public author match');
  check(await page.eval(`JSON.stringify([...document.querySelectorAll('.vk-public-row')].map(row => row.dataset.workId)) === ${JSON.stringify(JSON.stringify(cards.filter((_, i) => i % 2).map(card => card.work_id)))}`), 'author results preserve the provider order');
  await searchPublic('không có tác phẩm này');
  await page.waitFor(`document.querySelector('[data-testid=public-no-results]')`, 'filtered public miss');
  check(await page.eval(`!document.querySelector('[data-testid=public-empty], [data-testid=public-unavailable], #feed-end')`), 'no-results is separate from empty, unavailable and the end of a visible list');
  check(listRequests === loadedRequests && publicationRequests === 0, 'search uses loaded public cards without new list or manuscript requests');
  await page.click('#clear-public-search');
  await page.waitFor(`document.querySelectorAll('.vk-public-row').length === 12`, 'clear restores all loaded public cards');
  for (const theme of ['light', 'dark']) {
    await setPublicTheme(theme);
    await textContrast(page, theme, 'explore');
    for (const width of [1440, 1024, 834, 390, 320]) {
      await page.viewport(width);
      for (const scale of [100, 200]) {
        await page.eval(`document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
        check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), `Explore reflows at ${width}px/${scale}%/${theme}`);
        await page.capture(`explore-${width}-${scale}-${theme}`);
      }
    }
  }
  await page.eval(`document.documentElement.style.fontSize = ''; scrollTo(0, 0)`);
  await page.viewport(1440);
  await searchPublic('đường về'.normalize('NFD'));
  await page.click('.vk-public-row__link');
  await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${publication.revision_id}'`, 'fixture publication reader');
  check(await page.eval(`document.querySelectorAll('.vk-reader .vk-block').length === 3 && document.querySelector('.vk-reader .vk-block').textContent.includes(${JSON.stringify(INDENTED_LINE)}) && document.querySelector('.vk-reader .vk-block').textContent.includes(${JSON.stringify(literal)}) && !document.querySelector('.vk-reader script') && !window.__manuscriptExecuted`), 'poetry, indentation, paragraphs and literal markup remain text, never executable HTML');
  for (const theme of ['light', 'dark']) {
    await setPublicTheme(theme);
    await textContrast(page, theme, 'reader');
    for (const width of [1440, 1024, 834, 390, 320]) {
      await page.viewport(width);
      for (const scale of [100, 200]) {
        await page.eval(`document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
        check(await page.eval(`document.documentElement.scrollWidth <= innerWidth && getComputedStyle(document.querySelector('.vk-reader .vk-block')).whiteSpace === 'pre-wrap'`), `reader preserves lines and reflows at ${width}px/${scale}%/${theme}`);
        await page.capture(`reader-${width}-${scale}-${theme}`);
        if (width === 320 && scale === 200) {
          await page.eval(`document.querySelector('.vk-reader .vk-document').scrollIntoView({ block: 'start' })`);
          await page.capture(`reader-body-320-200-${theme}`);
        }
      }
    }
  }
  await page.eval(`document.documentElement.style.fontSize = ''`);
  await page.viewport(1440);
  for (const status of [404, 503]) {
    answers.set(publicationPath, { status, body: { error: { code: status === 404 ? 'not_found' : 'dependency_unavailable' } } });
    await openCollection();
    await page.waitFor(`document.querySelector('.vk-public-row__link')`, 'collection before reader failure');
    await page.click('.vk-public-row__link');
    await page.waitFor(status === 404 ? `document.querySelector('h1')?.textContent === 'Không tìm thấy trang này.'` : `document.querySelector('[data-testid=public-unavailable]')`, `reader ${status} state`);
    check(await page.eval(`!document.querySelector('.vk-document') && !document.querySelector('[data-testid=public-empty]')`), `reader ${status} does not display stale text or a successful empty collection`);
  }
  answers.set('/api/v1/works', { status: 503, body: { error: { code: 'dependency_unavailable' } } });
  await openCollection();
  await page.waitFor(`document.querySelector('[data-testid=public-unavailable]')`, 'fixture collection outage');
  await searchPublic(realQuery);
  check(await page.eval(`document.querySelector('[data-testid=public-unavailable]') && !document.querySelector('[data-testid=public-no-results], [data-testid=public-empty]')`), 'search during an outage keeps the dependency error');
  await page.click('[data-testid=public-unavailable] a[rel=external]');
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('#public-query')?.value === '${realQuery}' && document.querySelectorAll('.vk-public-row').length === 1`, 'reload retry keeps the query and recovers from the fixture outage using real SSR data');
  for (const state of ['empty', 'unavailable']) {
    answers.set('/api/v1/works', { status: state === 'empty' ? 200 : 503, body: state === 'empty' ? { works: [] } : { error: { code: 'dependency_unavailable' } } });
    await openCollection();
    await page.waitFor(`document.querySelector('[data-testid=public-${state}]')`, `Explore ${state}`);
    check(await page.eval(`!document.querySelector('[data-testid=public-${state === 'empty' ? 'unavailable' : 'empty'}]') && document.querySelector('#writing-prompt a').getAttribute('href') === '/ban-viet'`), `${state} is distinct and keeps a truthful participation route`);
    for (const theme of ['light', 'dark']) {
      await setPublicTheme(theme);
      for (const width of [1440, 320]) {
        await page.viewport(width);
        await page.eval(`scrollTo(0, 0)`);
        check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), `${state} reflows at ${width}px/${theme}`);
        await page.capture(`explore-${state}-${width}-${theme}`);
      }
    }
  }
  await page.send('Fetch.disable');
  page.cdp.listeners = page.cdp.listeners.filter(l => l !== listener);
  if (gateError) throw gateError;
  await page.viewport(1440);
  await page.eval(`document.documentElement.removeAttribute('data-theme')`);
}

async function mainPhase(page) {
  await serverHtml();
  await adaptiveShell(page);
  await draftDashboard(page);
  await publicReading(page);

  console.log('reader in the browser:');
  await page.goto(`${args.base}/tac-pham/${SAMPLE_WORK}`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'hydration');
  check(true, 'page hydrated');
  check(await page.eval(`document.querySelector('.vk-nav a[href="/"]').getAttribute('aria-current') === 'page'`), 'reader belongs to the Khám phá destination');
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
  await page.lineStart(SHIFT);
  const selected = `getSelection().toString()`;
  check((await page.eval(selected)) === LAST_LINE, 'platform line-start shortcut selects the last line');
  await shellWithDirtyEditor(page, editorPath, written, view, selected);
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
  await page.capture('1280-preview-long-title-dark');
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

  await page.lineEnd();
  await page.send('Input.insertText', { text: ' Thêm' });
  check((await page.body()) === `${written} Thêm`, 'typed after the dialog');
  await page.key('z', page.mod);
  check((await page.body()) === written, 'Ctrl/Cmd+Z undoes the last insertion');
  await page.key('Z', page.mod | SHIFT);
  check((await page.body()) === `${written} Thêm`, 'Ctrl/Cmd+Shift+Z redoes it');
  check(await page.eval(view), 'undo and redo in the same EditorView');
  await page.eval(`delete document.documentElement.dataset.theme`);

  // Chrome asks before a page's second download, which headless Chrome cannot show: this phase
  // downloads once (the recovery export); the toolbar export runs in the after-restart phase.
  const downloads = mkdtempSync(join(tmpdir(), 'vink-downloads-'));
  try {
    await page.cdp.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads, eventsEnabled: true });

    console.log('shortcut save, then back/forward with unsaved text:');
    await page.key('s', page.mod);
    await page.waitStatus('saved', 'Ctrl/Cmd+S save');
    const afterShortcut = await page.draft(workId);
    check(afterShortcut.version === 5, 'Ctrl/Cmd+S saved version 5');
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
    await openProfile(page);
    await page.click('[data-testid=open-me]');
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
    await page.waitFor(`document.activeElement?.dataset.testid === 'discard-back'`, 'confirmation focus after its animation frame');
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
    await page.cdp.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads, eventsEnabled: true });
    await page.typeInBody(STANZA);
    const current = await page.body();
    check(current === state.text + STANZA, 'unsaved stanza typed after the restart');
    await page.click('[data-testid=export]');
    const file = await downloaded(downloads);
    // HFS+ returns decomposed file names; only the name comparison is canonicalized.
    check(file.name.normalize('NFC') === EXPORT_NAME, `export file uses the capped title name (${file.name})`);
    check(file.text === `${state.title}\n\n${current}\n`, 'export holds the title and the unsaved text with lines and stanzas');
    check((await page.status()) === 'unsaved', 'export is not a save');
  } finally {
    rmSync(downloads, { recursive: true, force: true });
  }
}

const chrome = await launchChrome();
let exitCode = 0;
let activePage;
try {
  const cdp = new Cdp(chrome.url);
  await cdp.open();
  cdp.listeners.push(message => {
    if (message.method === 'Browser.downloadProgress' && message.params.state !== 'inProgress') {
      const { state, receivedBytes, totalBytes } = message.params;
      console.log(`  ..  browser download: ${state}, ${receivedBytes}/${totalBytes} bytes`);
    }
  });
  const page = await openPage(cdp);
  activePage = page;
  if (args.phase === 'main') await mainPhase(page);
  else if (args.phase === 'after-restart') await afterRestartPhase(page);
  else throw new Error(`unknown phase ${args.phase}`);
  check(page.errors.length === 0, `no console errors or exceptions${page.errors.length ? `: ${page.errors.join(' | ')}` : ''}`);
  console.log(`browser-smoke: PASS (${args.phase})`);
} catch (error) {
  if (activePage) {
    try {
      await activePage.capture('failure');
      console.error('browser context:', await activePage.eval(`({ path: location.pathname, preferences: { ...document.documentElement.dataset }, active: document.activeElement?.outerHTML, selection: getSelection().toString(), view: document.querySelector('.cm-editor')?.__vinkView, status: document.querySelector('[data-testid=save-status]')?.dataset.status, errors: ${JSON.stringify(activePage.errors)} })`));
    } catch { /* Preserve the original failure if the page itself crashed. */ }
  }
  console.error(`browser-smoke: FAIL ${error.message}`);
  exitCode = 1;
} finally {
  chrome.child.kill();
  await sleep(200);
  rmSync(chrome.profile, { recursive: true, force: true });
}
process.exit(exitCode);
