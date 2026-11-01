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
const SHIFT = 8;
const KEYS = {
  Escape: { code: 'Escape', keyCode: 27 },
  Tab: { code: 'Tab', keyCode: 9 },
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
    const context = await this.eval(`({ route: location.pathname, width: innerWidth, height: innerHeight, dpr: devicePixelRatio, theme: document.documentElement.dataset.theme || 'system', fontSize: getComputedStyle(document.documentElement).fontSize, userAgent: navigator.userAgent })`);
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

  /** Focuses the CodeMirror body, moves to the end with Ctrl+End and inserts committed text. */
  async typeInBody(text) {
    await this.eval(`document.querySelector('[data-testid=editor-body] .cm-content').focus()`);
    await this.key('End', CTRL);
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
  await page.send('Network.enable');
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
}

async function shellContrast(page, theme) {
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
    return ['.vk-nav a', '.vk-account-trigger', '#profile-menu .vk-muted', '#profile-menu .vk-theme-choice:has(:checked) span', '#profile-menu .vk-button'].map(selector => {
      const el = document.querySelector(selector);
      const foreground = getComputedStyle(el).color;
      const bg = background(el);
      const [a, b] = [luminance(foreground), luminance(bg)].sort((a, b) => b - a);
      return { selector, foreground, background: bg, ratio: (a + 0.05) / (b + 0.05) };
    });
  })()`);
  writeFileSync(join(dirname(args.state), `shell-contrast-${theme}.json`), JSON.stringify(pairs, null, 2));
  check(pairs.every(p => p.ratio >= 4.5), `${theme} shell text contrast >= 4.5 (min ${Math.min(...pairs.map(p => p.ratio)).toFixed(2)})`);
}

async function adaptiveShell(page) {
  console.log('adaptive navigation and appearance:');
  await page.viewport(1440);
  await page.goto(`${args.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'shell hydration');
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
  await shellContrast(page, 'dark');
  await page.capture('1440-overlay-dark');
  await page.key('Escape');
  check(await page.eval(`!${PROFILE_OPEN} && document.activeElement?.dataset.testid === 'profile-trigger'`), 'Escape closes Tôi and restores trigger focus');
  await page.reload();
  check(await page.eval(`document.documentElement.dataset.theme === 'dark'`), 'theme preference survives reload');
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
  await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
  await page.waitFor(`getComputedStyle(document.body).backgroundColor !== ${JSON.stringify(darkSurface)}`, 'system appearance follows OS changes');
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
        if (width === 1440) await shellContrast(page, theme);
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

async function mainPhase(page) {
  await serverHtml();
  await adaptiveShell(page);

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
  await page.key('Home', SHIFT);
  const selected = `getSelection().toString()`;
  check((await page.eval(selected)) === LAST_LINE, 'last line selected with Shift+Home');
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
    check(file.name === EXPORT_NAME, `export file uses the capped title name (${file.name})`);
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
