#!/usr/bin/env node
// Leptos UI preview checks over the Chrome DevTools Protocol. No npm dependencies: Node.js 22+
// and a Chrome/Chromium binary. Run through `cargo xtask ui-preview leptos verify`, which builds
// the preview and serves it on loopback.
//
//   node preview-smoke.mjs --base URL --chrome BIN --out DIR --catalog preview-catalog.json
//
// Evidence limits: one headless Chrome, DPR 1, emulated media features, programmatic clicks.
// Root font size 200% stands in for a user text-size setting. Captures are written for a person
// to open; this script asserts structure and computed values, not visual quality.

import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = Object.fromEntries(
  process.argv.slice(2).reduce((pairs, value, i, all) => {
    if (value.startsWith('--')) pairs.push([value.slice(2), all[i + 1]]);
    return pairs;
  }, []),
);
for (const key of ['base', 'chrome', 'out', 'catalog']) {
  if (!args[key]) {
    console.error(`preview-smoke: FAIL missing --${key}`);
    process.exit(1);
  }
}

const catalog = JSON.parse(readFileSync(args.catalog, 'utf8'));
const LIGHT = '#fffaff';
const DARK = '#17121d';
const WIDTHS = [320, 390, 834, 1440];
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function check(condition, message) {
  if (!condition) throw new Error(message);
  console.log(`  ok  ${message}`);
}

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
        resolve: (result) => { clearTimeout(timer); resolve(result); },
        reject: (error) => { clearTimeout(timer); reject(error); },
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
    this.media = { 'prefers-color-scheme': 'light', 'prefers-reduced-motion': 'no-preference' };
    this.state = { width: 1440, scale: '100%' };
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
    const result = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) {
      throw new Error(`evaluate failed: ${result.exceptionDetails.exception?.description ?? expression}`);
    }
    return result.result.value;
  }

  async waitFor(expression, what, timeout = 15000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      try {
        if (await this.eval(`Boolean(${expression})`)) return;
      } catch {
        // Navigating; retry.
      }
      await sleep(50);
    }
    throw new Error(`timed out waiting for ${what}`);
  }

  async emulate(feature, value) {
    this.media[feature] = value;
    await this.send('Emulation.setEmulatedMedia', {
      features: Object.entries(this.media).map(([name, v]) => ({ name, value: v })),
    });
  }

  async viewport(width, height = 900) {
    this.state.width = width;
    await this.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
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

  async choose(name, label) {
    await this.click(`input[name=${name}][data-label="${label}"]`);
    await this.waitFor(`document.querySelector('input[name=${name}][data-label="${label}"]').checked`, `${name} → ${label}`);
    if (name === 'scale') this.state.scale = label;
    await this.settle();
  }

  async open(entry, scenario = 'default') {
    await this.eval(`location.hash = ${JSON.stringify(`#${entry}/${scenario}`)}`);
    await this.waitFor(
      `document.querySelector('#specimen-scope[data-catalog-id="${entry}"][data-scenario-id="${scenario}"]')`,
      `${entry}/${scenario}`,
    );
    await this.settle();
  }

  async settle() {
    await this.eval(`new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))`);
  }

  css(selector, property) {
    return this.eval(`getComputedStyle(document.querySelector(${JSON.stringify(selector)})).getPropertyValue(${JSON.stringify(property)}).trim()`);
  }

  async capture(name) {
    await this.eval('document.fonts.ready');
    const height = await this.eval('Math.min(document.documentElement.scrollHeight, 6000)');
    await this.send('Emulation.setDeviceMetricsOverride', { width: this.state.width, height, deviceScaleFactor: 1, mobile: false });
    await this.settle();
    const { data } = await this.send('Page.captureScreenshot', { format: 'png' });
    mkdirSync(args.out, { recursive: true });
    writeFileSync(join(args.out, `${name}.png`), Buffer.from(data, 'base64'));
    const context = await this.eval(`({
      hash: location.hash,
      width: innerWidth,
      dpr: devicePixelRatio,
      rootFontSize: getComputedStyle(document.documentElement).fontSize,
      theme: document.querySelector('.vk-ui').dataset.theme,
      surface: getComputedStyle(document.getElementById('specimen-scope')).getPropertyValue('--vk-surface').trim(),
      fonts: ['16px "Be Vietnam Pro"', '16px "Literata"'].map((f) => [f, document.fonts.check(f)]),
    })`);
    this.contexts[name] = { ...context, media: { ...this.media } };
    await this.viewport(this.state.width);
  }
}

async function launchChrome() {
  const profile = mkdtempSync(join(tmpdir(), 'vink-ui-chrome-'));
  const flags = [
    '--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run',
    '--no-default-browser-check', '--disable-extensions', '--disable-gpu', '--window-size=1440,900',
    '--lang=vi-VN', 'about:blank',
  ];
  if (process.env.CI || process.env.VINK_CHROME_NO_SANDBOX) flags.push('--no-sandbox');
  const child = spawn(args.chrome, flags, { stdio: ['ignore', 'ignore', 'pipe'] });
  let output = '';
  let exited = null;
  child.stderr.on('data', (chunk) => (output += chunk));
  child.on('exit', (code) => (exited = code));
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
  page.contexts = {};
  await page.send('Page.enable');
  await page.send('Runtime.enable');
  await page.send('Page.bringToFront');
  await page.emulate('prefers-color-scheme', 'light');
  await page.viewport(1440);
  return page;
}

async function structure(page) {
  const ids = catalog.entries.map((e) => e.id);
  const links = await page.eval(`[...document.querySelectorAll('[data-catalog-link]')].map((a) => a.dataset.catalogLink)`);
  check(JSON.stringify(links) === JSON.stringify(ids), `navigation lists the shared catalog IDs in order (${ids.length})`);
  check((await page.eval(`document.querySelector('[data-testid=offline-note]').textContent`)).includes('Không kết nối dịch vụ'), 'preview states it calls no service');
  for (const entry of catalog.entries) {
    for (const scenario of entry.scenarios) {
      await page.open(entry.id, scenario.id);
      const inspector = await page.eval(`document.querySelector('[data-testid=inspector-id]').textContent`);
      check(inspector === `${entry.id} / ${scenario.id}`, `exact link renders ${entry.id}/${scenario.id}`);
    }
  }
  const status = await page.eval(`document.querySelector('[data-testid=inspector-status]').textContent`);
  check(status.includes('implemented-unverified'), 'inspector shows the registration status, not a pass');
}

async function themes(page) {
  await page.open('foundation.color-roles');
  const surface = () => page.css('#specimen-scope', '--vk-surface');
  await page.choose('theme', 'Tối');
  check((await surface()) === DARK, 'explicit Tối themes the specimen dark');
  await page.choose('theme', 'Sáng');
  check((await surface()) === LIGHT, 'explicit Sáng themes the specimen light');
  await page.emulate('prefers-color-scheme', 'dark');
  await page.settle();
  check((await surface()) === LIGHT, 'explicit Sáng overrides a dark system');
  await page.choose('theme', 'Hệ thống');
  check((await surface()) === DARK, 'Hệ thống follows a dark system');
  await page.emulate('prefers-color-scheme', 'light');
  await page.settle();
  check((await surface()) === LIGHT, 'Hệ thống follows the system back to light without reload');
  const effective = await page.eval(`document.querySelector('[data-testid=effective] [data-property="--vk-surface"] td')?.textContent`);
  check(effective === LIGHT, 'inspector reads the effective value from the rendered scope');
  const log = await page.eval(`document.querySelector('[data-testid=event-log]').textContent`);
  check(log.includes('giao diện → Hệ thống'), 'event log records control changes with the fixed clock');

  await page.open('foundation.color-roles', 'nested-theme');
  const nested = (which) => page.eval(`getComputedStyle(document.querySelector('[data-nested=${which}]')).getPropertyValue('--vk-surface').trim()`);
  check((await nested('dark')) === DARK && (await nested('light')) === LIGHT, 'nested explicit scopes override the light root');
  check((await nested('system')) === LIGHT, 'a System scope inside Tối follows the light system');
  const recipe = await page.eval(`getComputedStyle(document.querySelector('[data-nested=dark]')).getPropertyValue('--vk-action-primary-container').trim()`);
  check(recipe === '#d4bbff', 'recipe aliases resolve against the nested scope, not the root');
  await page.emulate('prefers-color-scheme', 'dark');
  await page.settle();
  check((await nested('system')) === DARK, 'the nested System scope follows the system to dark');
  await page.choose('theme', 'Tối');
  check((await nested('light')) === LIGHT, 'an explicit Sáng scope stays light inside a dark root');
  await page.emulate('prefers-color-scheme', 'light');
  await page.choose('theme', 'Hệ thống');
}

async function densityAndMotion(page) {
  await page.open('foundation.shape-spacing');
  check((await page.css('#specimen-scope', '--vk-density-control-padding')) === '0.75rem', 'comfortable control padding is 0.75rem');
  await page.open('foundation.shape-spacing', 'compact');
  check((await page.css('#specimen-scope', '--vk-density-control-padding')) === '0.5rem', 'compact scope uses 0.5rem');
  const control = await page.eval(`(() => { const s = getComputedStyle(document.querySelector('.vkp-control')); return [s.paddingTop, s.minHeight]; })()`);
  check(control[0] === '8px' && control[1] === '44px', `compact keeps the 44px hit target (${control})`);

  const loop = `(() => { const el = document.querySelector('.vkp-loop'); return { count: getComputedStyle(el).animationIterationCount, running: el.getAnimations().filter((a) => a.playState === 'running').length }; })()`;
  await page.open('foundation.motion');
  let state = await page.eval(loop);
  check(state.count === 'infinite' && state.running === 1, 'the loading loop runs by default');
  check((await page.css('#specimen-scope', '--vk-duration-medium')) === '320ms', 'finite transitions keep 320ms');
  await page.open('foundation.motion', 'reduced');
  state = await page.eval(loop);
  check(state.count === '0' && state.running === 0, 'a reduced scope stops the loop instead of spinning at 0ms');
  check((await page.css('#specimen-scope', '--vk-duration-medium')) === '0ms', 'a reduced scope makes transitions instant');
  await page.open('foundation.motion');
  await page.emulate('prefers-reduced-motion', 'reduce');
  await page.settle();
  state = await page.eval(loop);
  check(state.count === '0' && state.running === 0, 'the system reduced-motion setting stops the loop');
  await page.emulate('prefers-reduced-motion', 'no-preference');
  await page.choose('motion', 'Giảm');
  state = await page.eval(loop);
  check(state.count === '0', 'the preview Giảm preference reduces motion on the root');
  await page.choose('motion', 'Hệ thống');
}

async function search(page) {
  const type = async (text) => {
    await page.eval(`(() => { const i = document.querySelector('.vkp-search input'); i.value = ${JSON.stringify(text)}; i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
    await page.settle();
    return page.eval(`[...document.querySelectorAll('[data-catalog-link]')].map((a) => a.dataset.catalogLink)`);
  };
  const motion = await type('chuyen dong');
  check(motion.length === 1 && motion[0] === 'foundation.motion', 'search ignores Vietnamese marks (chuyen dong → Chuyển động)');
  const scenario = await type('DẤU TỔ HỢP');
  check(scenario.length === 1 && scenario[0] === 'foundation.typography', 'search covers scenario labels');
  await type('zzz');
  check(await page.eval(`document.querySelector('.vkp-empty')?.textContent === 'Không có mục khớp.'`), 'an unmatched search says so');
  await type('');
}

async function reflow(page) {
  for (const scale of ['100%', '200%']) {
    await page.choose('scale', scale);
    for (const width of WIDTHS) {
      await page.viewport(width);
      for (const entry of catalog.entries) {
        for (const scenario of entry.scenarios) {
          await page.open(entry.id, scenario.id);
          const overflow = await page.eval(`document.documentElement.scrollWidth - innerWidth`);
          if (overflow > 0) throw new Error(`horizontal overflow ${overflow}px at ${width}px/${scale} on ${entry.id}/${scenario.id}`);
        }
      }
    }
    console.log(`  ok  no horizontal page overflow at ${WIDTHS.join('/')}px, text ${scale}, every scenario`);
  }
  await page.choose('scale', '100%');
  const fonts = await page.eval(`document.fonts.ready.then(() => ['16px "Be Vietnam Pro"', '600 16px "Be Vietnam Pro"', '16px "Literata"'].every((f) => document.fonts.check(f)))`);
  check(fonts, 'self-hosted Be Vietnam Pro 400/600 and Literata load');
}

async function captures(page) {
  for (const [theme, label] of [['light', 'Sáng'], ['dark', 'Tối']]) {
    await page.choose('theme', label);
    for (const width of WIDTHS) {
      await page.viewport(width);
      for (const entry of catalog.entries) {
        await page.open(entry.id);
        await page.capture(`${entry.id}--default--${theme}--${width}`);
      }
    }
  }
  await page.choose('theme', 'Sáng');
  const extra = [
    ['foundation.color-roles', 'nested-theme', 1440],
    ['foundation.typography', 'combining-marks', 390],
    ['foundation.shape-spacing', 'compact', 834],
    ['foundation.motion', 'reduced', 834],
  ];
  for (const [entry, scenario, width] of extra) {
    await page.viewport(width);
    await page.open(entry, scenario);
    await page.capture(`${entry}--${scenario}--light--${width}`);
  }
  await page.choose('scale', '200%');
  for (const width of [320, 1440]) {
    await page.viewport(width);
    await page.open('foundation.typography', 'combining-marks');
    await page.capture(`foundation.typography--combining-marks--light--${width}--text200`);
  }
  await page.choose('scale', '100%');
  await page.viewport(1440);
}

const chrome = await launchChrome();
let exitCode = 0;
try {
  const cdp = new Cdp(chrome.url);
  await cdp.open();
  const page = await openPage(cdp);
  const version = await cdp.send('Browser.getVersion');
  await page.send('Page.navigate', { url: args.base });
  await page.waitFor(`document.documentElement.dataset.ready === 'true' && document.getElementById('specimen-scope')`, 'preview mounted');
  await structure(page);
  await themes(page);
  await densityAndMotion(page);
  await search(page);
  await reflow(page);
  await captures(page);
  check(page.errors.length === 0, `no console errors or exceptions${page.errors.length ? `: ${page.errors.join(' | ')}` : ''}`);
  writeFileSync(join(args.out, 'contexts.json'), JSON.stringify({ browser: version.product, userAgent: version.userAgent, captures: page.contexts }, null, 2));
  console.log(`preview-smoke: PASS (${Object.keys(page.contexts).length} captures in ${args.out})`);
} catch (error) {
  console.error(`preview-smoke: FAIL ${error.message}`);
  exitCode = 1;
} finally {
  const exited = chrome.child.exitCode !== null ? null : new Promise((resolve) => chrome.child.once('exit', resolve));
  chrome.child.kill();
  await Promise.race([exited, sleep(5000)]);
  rmSync(chrome.profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}
process.exit(exitCode);
