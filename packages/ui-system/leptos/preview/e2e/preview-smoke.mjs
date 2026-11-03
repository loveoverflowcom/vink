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
import { dirname, join } from 'node:path';
import { captureTransaction, captureSpecimen, sourceIdentity } from './focus-capture.mjs';

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
const identity = JSON.parse(readFileSync(join(dirname(args.catalog), 'fixtures', 'identity.json'), 'utf8'));
const COMPONENTS = catalog.entries.filter((e) => !e.id.startsWith('foundation.'));
const LIGHT = '#fffaff';
const DARK = '#17121d';
const WIDTHS = [320, 390, 834, 1024, 1440];
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
    this.state.height = height;
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

  async key(name) {
    const keys = {
      Enter: { key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' },
      Space: { key: ' ', code: 'Space', windowsVirtualKeyCode: 32, text: ' ' },
      Tab: { key: 'Tab', code: 'Tab', windowsVirtualKeyCode: 9 },
      ArrowDown: { key: 'ArrowDown', code: 'ArrowDown', windowsVirtualKeyCode: 40 },
      Escape: { key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 },
    };
    const k = keys[name];
    await this.send('Input.dispatchKeyEvent', { type: 'keyDown', ...k });
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', ...k, text: undefined });
    await this.settle();
  }

  async log() {
    return this.eval(`document.querySelector('[data-testid=event-log]').textContent`);
  }

  async choose(name, label) {
    await this.click(`input[name=${name}][data-label="${label}"]`);
    await this.waitFor(`document.querySelector('input[name=${name}][data-label="${label}"]').checked`, `${name} → ${label}`);
    if (name === 'scale') this.state.scale = label;
    await this.settle();
  }

  async open(entry, scenario = 'default') {
    const hash = JSON.stringify(`#${entry}/${scenario}`);
    // Chrome drops same-document navigations past its flood limit ("Throttling navigation");
    // wait out the window instead of failing on a harness limit.
    for (let attempt = 0; attempt < 24; attempt += 1) {
      await this.eval(`location.hash = ${hash}`);
      if (await this.eval(`location.hash === ${hash}`)) break;
      await sleep(500);
    }
    try {
      await this.waitFor(
        `document.querySelector('#specimen-scope[data-catalog-id="${entry}"][data-scenario-id="${scenario}"]')`,
        `${entry}/${scenario}`,
      );
    } catch (error) {
      const state = await this.eval(`[location.href, document.getElementById('specimen-scope')?.dataset.catalogId, document.getElementById('specimen-scope')?.dataset.scenarioId, innerWidth]`).catch(() => 'unknown');
      throw new Error(`${error.message} (page: ${JSON.stringify(state)}, ${JSON.stringify(this.state)})`);
    }
    await this.settle();
  }

  async settle() {
    await this.eval(`new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))`);
  }

  css(selector, property) {
    return this.eval(`getComputedStyle(document.querySelector(${JSON.stringify(selector)})).getPropertyValue(${JSON.stringify(property)}).trim()`);
  }

  async capture(name) {
    return captureTransaction(this, async () => {
      await this.eval('document.fonts.ready');
      const height = await this.eval('Math.min(document.documentElement.scrollHeight, 6000)');
      await this.send('Emulation.setDeviceMetricsOverride', { width: this.state.width, height, deviceScaleFactor: 1, mobile: false });
      await this.eval('scrollTo(0,0)');
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
    });
  }
}

async function focusedCaptures(page, version) {
  const entries = ['layout.grid', 'identity.avatar-group', 'state.empty', 'state.error', 'navigation.rail', 'composition.feedback'];
  const records = entries.flatMap(catalog_id => ['light', 'dark'].flatMap(theme =>
    [[390, 100], [1440, 100], [390, 200]].map(([width, scale]) => ({
      catalog_id, scenario_id: 'default', theme, width, text_scale: scale,
      raw_name: `${catalog_id}--default--${theme}--${width}${scale === 200 ? '--text200' : ''}`,
      capture_status: 'BLOCKED', reason: 'Not reached in this run', inspected: false,
    }))));
  const manifest = { format: 'vink-preview-focus-v1', run: { ...sourceIdentity(), browser: version.product,
    host: 'headless-chrome', renderer: 'leptos-web', data_mode: 'synthetic', locale: 'vi-VN' },
    scope: { selection: 'six default specimens, light/dark, 390/1440 at 100%, 390 at 200%; opt-in',
      expected: records.length, raw_policy: 'preserve raw filenames; capped document is separate from complete specimen tiles' }, captures: records };
  const save = () => {
    manifest.coverage = { expected: records.length, captured: records.filter(r => r.capture_status === 'CAPTURED').length,
      inspected: records.filter(r => r.inspected).length, blocked: records.filter(r => r.capture_status === 'BLOCKED').length };
    writeFileSync(join(args.out, 'focus-manifest.json'), JSON.stringify(manifest, null, 2));
  };
  save();
  try {
    for (const record of records) {
      await page.choose('theme', record.theme === 'dark' ? 'Tối' : 'Sáng');
      await page.choose('scale', `${record.text_scale}%`);
      await page.viewport(record.width);
      await page.open(record.catalog_id, record.scenario_id);
      // Exercise scroll/focus preservation on a specimen that can start below 6000px.
      await page.eval(`document.querySelector('#specimen-scope button, #specimen-scope a, #specimen-scope input')?.focus();
        document.getElementById('specimen').scrollIntoView({block:'start'})`);
      if (!page.contexts[record.raw_name]) await page.capture(record.raw_name);
      Object.assign(record, await captureSpecimen(page, record, args.out), { capture_status: 'CAPTURED', reason: null });
      save();
    }
    check(sourceIdentity().source_fingerprint === manifest.run.source_fingerprint, 'focused capture source stayed unchanged');
    check(manifest.coverage.captured === manifest.scope.expected, `focused captures: ${manifest.coverage.captured}/${manifest.scope.expected}; no blank PNGs, exact state restoration`);
  } catch (error) {
    const current = records.find(record => record.capture_status === 'BLOCKED');
    if (current) current.reason = error.message;
    throw error;
  } finally {
    save();
    await page.choose('scale', '100%');
    await page.viewport(1440);
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
      const duplicates = await page.eval(`(() => { const seen = new Set(), dup = []; for (const el of document.querySelectorAll('[id]')) { if (seen.has(el.id)) dup.push(el.id); seen.add(el.id); } return dup; })()`);
      if (duplicates.length) throw new Error(`duplicate ids on ${entry.id}/${scenario.id}: ${duplicates.join(', ')}`);
      const missing = await page.eval(`[...document.querySelectorAll('[aria-describedby]')].flatMap((el) => el.getAttribute('aria-describedby').split(' ')).filter((id) => !document.getElementById(id))`);
      if (missing.length) throw new Error(`aria-describedby points at nothing on ${entry.id}/${scenario.id}: ${missing.join(', ')}`);
      const invalid = await page.eval(`(() => {
        const root = document.querySelector('#specimen-scope');
        const nested = [...root.querySelectorAll('button, a[href]')].filter((el) => el.querySelector('button, a[href], input, select, textarea'));
        const unnamed = [...root.querySelectorAll('button, a[href], input:not([type=hidden]), textarea, select')].filter((el) => {
          const labelled = (el.getAttribute('aria-labelledby') || '').split(' ').map((id) => document.getElementById(id)?.textContent || '').join('');
          return !(el.getAttribute('aria-label') || labelled || [...(el.labels || [])].map((l) => l.textContent).join('') || (el.matches('button, a') ? el.textContent : '')).trim();
        });
        return [...nested, ...unnamed].map((el) => el.outerHTML.slice(0, 200));
      })()`);
      if (invalid.length) throw new Error(`nested or unnamed controls on ${entry.id}/${scenario.id}: ${invalid.join(', ')}`);
    }
  }
  console.log('  ok  no duplicate ids and every aria-describedby resolves, every scenario');
  const status = await page.eval(`document.querySelector('[data-testid=inspector-status]').textContent`);
  check(status.includes('implemented-unverified'), 'inspector shows the registration status, not a pass');
}

async function forcedColors(page) {
  await page.open('action.button');
  await page.emulate('forced-colors', 'active');
  await page.eval(`document.querySelector('#specimen-scope button').focus()`);
  const focus = await page.eval(`(() => {
    const el = document.activeElement, style = getComputedStyle(el);
    return { name: el.textContent.trim(), outline: parseFloat(style.outlineWidth), adjustment: style.forcedColorAdjust };
  })()`);
  check(focus.name && focus.outline >= 3 && focus.adjustment !== 'none', 'forced-colors keeps named control and visible keyboard focus under system colors');
  await page.capture('action.button--forced-colors--1440');
  await page.emulate('forced-colors', 'none');
}

async function compactVietnameseWords(page) {
  await page.viewport(1024);
  await page.choose('scale', '200%');
  await page.open('navigation.rail', 'compact');
  const broken = await page.eval(`(() => {
    const labels = [...document.querySelectorAll('.vk-ui-rail .vk-ui-nav-item__label')];
    const selected = labels.filter((el) => el.textContent === 'Khám phá');
    if (!selected.length) throw new Error('Missing Vietnamese rail label');
    return selected.flatMap((el) => {
      const node = el.firstChild;
      return [[0,4],[5,8]].filter(([a,b]) => {
        const r = document.createRange(); r.setStart(node,a); r.setEnd(node,b);
        const rects = [...r.getClientRects()];
        return rects.length !== 1;
      });
    });
  })()`);
  const geometry = await page.eval(`[...document.querySelectorAll('.vk-ui-rail .vk-ui-nav-item__label')].map(el => ({label:el.textContent, width:el.getBoundingClientRect().width,font:getComputedStyle(el).font,rail:el.closest('.vk-ui-rail').getBoundingClientRect().width, item:el.parentElement.getBoundingClientRect().width}))`);
  await page.eval(`document.querySelector('#specimen-scope .vk-ui-rail .vk-ui-nav-item__label').scrollIntoView({block:'center'})`);
  const shot = await page.send('Page.captureScreenshot', {format:'png', captureBeyondViewport:false});
  writeFileSync(join(args.out, 'compact-words-viewport.png'), Buffer.from(shot.data, 'base64'));
  console.log('  compact geometry ' + JSON.stringify(geometry));
  await page.capture('navigation.rail--compact-words--1024--text200');
  check(broken.length === 0, 'compact rail keeps Vietnamese words intact at 200% text: ' + JSON.stringify(broken));
  await page.choose('scale', '100%');
  await page.viewport(1440);
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

// Resolved-colour helpers evaluated in the page: WCAG 2.x on computed colours, walking up to the
// first opaque background.
const CONTRAST_JS = `
  const rgb = (c) => { const m = c.match(/[\\d.]+/g).map(Number); return { r: m[0], g: m[1], b: m[2], a: m.length > 3 ? m[3] : 1 }; };
  const lin = (v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
  const lum = (c) => 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
  const over = (top, below) => ({ r: top.r * top.a + below.r * (1 - top.a), g: top.g * top.a + below.g * (1 - top.a), b: top.b * top.a + below.b * (1 - top.a), a: 1 });
  const bgOf = (el) => { const layers = []; for (let n = el; n; n = n.parentElement) { const c = rgb(getComputedStyle(n).backgroundColor); if (c.a > 0) { layers.push(c); if (c.a >= 1) break; } } return layers.reverse().reduce((acc, c) => over(c, acc), { r: 255, g: 255, b: 255, a: 1 }); };
  const ratio = (el) => { const bg = bgOf(el); const fg = over(rgb(getComputedStyle(el).color), bg); const [a, b] = [lum(fg), lum(bg)].sort((x, y) => y - x); return (a + 0.05) / (b + 0.05); };
`;

async function lowestContrast(page, selector) {
  return page.eval(`(() => { ${CONTRAST_JS} return Math.min(...[...document.querySelectorAll(${JSON.stringify(selector)})].map(ratio)); })()`);
}

async function components(page) {
  const count = () => page.eval(`document.querySelector('[data-testid=press-count]').textContent`);
  await page.open('action.button');
  await page.click('[data-testid=press-once] button');
  check((await count()).includes('1 lần'), 'Button: a click dispatches exactly once');
  await page.eval(`document.querySelector('[data-testid=press-once] button').focus()`);
  await page.key('Enter');
  check((await count()).includes('2 lần'), 'Button: Enter dispatches exactly once');
  await page.key('Space');
  check((await count()).includes('3 lần'), 'Button: Space dispatches exactly once');
  const ring = await page.eval(`(() => { const el = document.activeElement; const s = getComputedStyle(el); return [el.matches(':focus-visible'), s.outlineStyle, s.outlineWidth]; })()`);
  check(ring[0] && ring[1] === 'solid' && ring[2] === '3px', `Button: keyboard focus shows the 3px ring (${ring})`);
  await page.click('[data-testid=in-form-button] button');
  const formLog = await page.log();
  check(formLog.includes('trong form') && !formLog.includes('form → submit'), 'Button: the default kind never submits its form');
  const heights = await page.eval(`[...document.querySelectorAll('.vk-ui-button')].map((b) => b.getBoundingClientRect().height)`);
  check(heights.every((h) => h >= 48), `Button: every target is at least 48px tall (${Math.min(...heights)})`);

  await page.open('action.button', 'disabled');
  await page.click('#disabled-filled');
  const disabled = await page.eval(`(() => { const b = document.getElementById('disabled-filled'); b.focus(); return [b.getAttribute('aria-disabled'), document.activeElement === b, document.getElementById(b.getAttribute('aria-describedby'))?.textContent]; })()`);
  check((await count()).includes('0 lần') && disabled[0] === 'true', 'Button: a disabled click dispatches nothing');
  check(disabled[1] && disabled[2] === 'Chưa có thay đổi để lưu', 'Button: disabled stays focusable and its reason is linked');

  await page.open('action.button', 'pending');
  const width = () => page.eval(`document.getElementById('pending-save').getBoundingClientRect().width`);
  const before = await width();
  await page.click('#pending-save');
  const busy = await page.eval(`document.getElementById('pending-save').getAttribute('aria-busy')`);
  await page.click('#pending-save');
  check((await count()).includes('1 lần') && busy === 'true', 'Button: pending ignores a second activation');
  const after = await width();
  const focused = await page.eval(`document.activeElement.id`);
  check(Math.abs(after - before) < 0.5 && focused === 'pending-save', `Button: pending keeps width (${before}→${after}) and focus`);

  await page.open('action.link');
  const links = await page.eval(`(() => { const ext = document.querySelector('a[target=_blank]'); const bad = document.querySelector('[data-testid=refused-link] [data-invalid-href]'); return [ext?.rel, ext?.textContent.includes('(mở tab mới)'), bad && !bad.hasAttribute('href'), document.querySelectorAll('a[href^="javascript"]').length]; })()`);
  check(links[0] === 'noopener noreferrer' && links[1], 'Link: a new tab is announced and isolated');
  check(links[2] && links[3] === 0, 'Link: a script URL renders as text, never as a link');

  await page.open('action.filter-chip');
  await page.eval(`document.querySelector('.vk-ui-chip').focus()`);
  await page.key('Space');
  const chip = await page.eval(`document.querySelector('.vk-ui-chip').getAttribute('aria-pressed')`);
  await page.key('Tab');
  const next = await page.eval(`[...document.querySelectorAll('.vk-ui-chip')].indexOf(document.activeElement)`);
  check(chip === 'true' && next === 1, 'FilterChip: Space toggles aria-pressed and Tab moves to the next chip');

  await page.open('field.text-field');
  await page.eval(`document.getElementById('title').focus()`);
  await page.send('Input.insertText', { text: 'Bàn viết' });
  await page.eval(`document.getElementById('title').setSelectionRange(3, 3)`);
  await page.send('Input.insertText', { text: 'x' });
  await page.settle();
  const caret = await page.eval(`(() => { const i = document.getElementById('title'); return [i.value, i.selectionStart, document.querySelector('[data-testid=title-value]').textContent]; })()`);
  check(caret[0] === 'Bànx viết' && caret[1] === 4 && caret[2].includes('Bànx viết'), `TextField: controlled value keeps the caret mid-text (${caret.slice(0, 2)})`);
  await page.send('Input.imeSetComposition', { text: 'Vi', selectionStart: 2, selectionEnd: 2 });
  await page.send('Input.imeSetComposition', { text: 'Việ', selectionStart: 3, selectionEnd: 3 });
  await page.send('Input.insertText', { text: 'Việt' });
  await page.settle();
  const ime = await page.eval(`[document.getElementById('title').value, document.querySelector('[data-testid=event-log]').textContent]`);
  check(ime[0].includes('Việt') && ime[1].includes('(đang soạn IME)'), 'TextField: composition updates are flagged as composing');
  check(/title → "[^"]*Việt[^"]*"(?! \(đang soạn IME\))/.test(ime[1]), 'TextField: the committed text arrives without the composing flag');
  const assoc = await page.eval(`(() => { const i = document.getElementById('title'); return [document.querySelector('label[for=title]')?.textContent, i.getAttribute('aria-describedby'), i.hasAttribute('aria-invalid'), i.getAttribute('aria-required')]; })()`);
  check(assoc[0].startsWith('Tiêu đề') && assoc[1] === 'title-help' && !assoc[2] && assoc[3] === 'true', `TextField: label, help and required are linked; no error means not invalid (${assoc})`);
  await page.open('field.text-field', 'invalid');
  const invalid = await page.eval(`(() => { const i = document.getElementById('title'); return [i.getAttribute('aria-invalid'), i.getAttribute('aria-describedby'), document.getElementById('title-error').textContent]; })()`);
  check(invalid[0] === 'true' && invalid[1] === 'title-help title-error' && invalid[2].includes('Cần có tiêu đề'), 'TextField: an error is linked and marks the field invalid');
  await page.open('field.text-field', 'read-only');
  const ro = await page.eval(`(() => { const i = document.getElementById('title'); i.focus(); return [i.readOnly, i.disabled, document.activeElement === i]; })()`);
  await page.open('field.text-field', 'disabled');
  const off = await page.eval(`(() => { const i = document.getElementById('title'); i.focus(); return [i.disabled, document.activeElement === i]; })()`);
  check(ro[0] && !ro[1] && ro[2] && off[0] && !off[1], 'TextField: read-only stays focusable, disabled does not');
  await page.open('field.text-field', 'long-label');
  check((await page.eval(`document.getElementById('title').value`)) === '0', 'TextField: the value 0 is kept as typed');

  await page.open('field.search-field', 'long-label');
  await page.eval(`document.getElementById('search').focus()`);
  await page.key('Enter');
  check((await page.log()).includes('search → gửi "Hàng Đường"'), 'SearchField: Enter submits the current text');
  await page.click('.vk-ui-field__control[data-search] .vk-ui-icon-button');
  const cleared = await page.eval(`[document.getElementById('search').value, document.activeElement.id]`);
  check(cleared[0] === '' && cleared[1] === 'search', 'SearchField: clear empties the value and returns focus');

  await page.open('field.checkbox');
  await page.click('#agree + .vk-ui-choice__label');
  const box = await page.eval(`[document.getElementById('agree').checked, (document.querySelector('[data-testid=event-log]').textContent.match(/checkbox agree/g) || []).length]`);
  check(box[0] && box[1] === 1, 'Checkbox: the label and box are one target with one dispatch');
  await page.open('field.radio');
  await page.eval(`document.querySelector('input[name=genre]:checked').focus()`);
  await page.key('ArrowDown');
  check((await page.log()).includes('radio genre = van-xuoi'), 'RadioGroup: arrow keys move the native selection');

  for (const [entry, scenario] of [['field.text-field', 'default'], ['field.text-area', 'invalid'], ['field.select', 'invalid'], ['field.form', 'invalid'], ['composition.nested', 'default']]) {
    await page.open(entry, scenario);
    const unlabelled = await page.eval(`[...document.querySelectorAll('#specimen-scope input:not([type=radio]):not([type=checkbox]), #specimen-scope textarea, #specimen-scope select')].filter((c) => !document.querySelector('label[for="' + c.id + '"]')).map((c) => c.id || c.outerHTML.slice(0, 40))`);
    if (unlabelled.length) throw new Error(`unlabelled controls in ${entry}/${scenario}: ${unlabelled}`);
  }
  console.log('  ok  every text, area and select control has a <label for>');
  const targets = await page.eval(`[...document.querySelectorAll('#specimen-scope :is(.vk-ui-input, .vk-ui-choice__target, .vk-ui-button, .vk-ui-icon-button)')].map((e) => e.getBoundingClientRect().height)`);
  check(targets.every((h) => h >= 48), `fields and choices are at least 48px tall (${Math.min(...targets)})`);

  await page.open('identity.avatar', 'image-error');
  await page.waitFor(`(() => { const r = document.querySelector('[data-case=ready] img'); return r && r.complete && r.naturalWidth > 0 && !document.querySelector('[data-case=broken] img'); })()`, 'avatar images settle');
  const fallbacks = await page.eval(`['broken', 'unsafe', 'failed', 'loading'].map((c) => [c, !!document.querySelector('[data-case=' + c + '] img'), document.querySelector('[data-case=' + c + '] .vk-ui-avatar').dataset.initials])`);
  check(fallbacks.every(([, img, letters]) => !img && letters), `Avatar: broken AVIF, unsafe URL, failed and loading show initials (${fallbacks.map((f) => f[0] + ':' + f[2])})`);
  check((await page.eval(`document.querySelectorAll('img[src^="javascript"]').length`)) === 0, 'Avatar: a script URL never reaches an <img>');

  await page.open('identity.avatar', 'long-label');
  const toneMap = () => page.eval(`Object.fromEntries([...document.querySelectorAll('[data-testid=avatar-order] figure')].map((f) => [f.dataset.key, f.querySelector('.vk-ui-avatar').dataset.tone]))`);
  const first = await toneMap();
  await page.click('.vk-ui-button--tonal');
  const second = await toneMap();
  const expected = Object.fromEntries(identity.tones_vectors.filter((v) => v.key && first[v.key]).map((v) => [v.key, String(v.expected)]));
  const order = (map) => Object.keys(map).join();
  check(order(first) !== order(second) && Object.keys(first).every((k) => first[k] === second[k]), 'Avatar: reordering keeps every tone (seeded by key, not position)');
  check(Object.keys(expected).length >= 2 && Object.entries(expected).every(([k, t]) => first[k] === t), `Avatar: rendered tones equal the shared golden vectors (${JSON.stringify(first)})`);

  await page.open('identity.avatar-group', 'absent-identity');
  const unknown = await page.eval(`(() => { const g = document.querySelector('[data-testid=group-unknown] .vk-ui-avatar-group'); return [g.textContent.includes('+'), g.getAttribute('aria-label'), g.querySelectorAll('[tabindex], a, button').length]; })()`);
  check(!unknown[0] && !unknown[1].includes('người khác') && unknown[2] === 0, `AvatarGroup: an unknown total shows no count and no focus stops (${unknown[1]})`);
  await page.open('identity.avatar-group');
  check((await page.eval(`document.querySelector('[data-testid=group-known] .vk-ui-avatar-group__more').textContent`)) === '+9', 'AvatarGroup: a known total shows +9');

  await page.open('identity.author-byline');
  const byline = await page.eval(`(() => { const b = document.querySelector('[data-byline=feed]'); const n = getComputedStyle(b.querySelector('.vk-ui-byline__name')); const m = getComputedStyle(b.querySelector('.vk-ui-byline__meta')); return [Number(n.fontWeight), parseFloat(n.fontSize), parseFloat(m.fontSize), b.querySelector('.vk-ui-avatar').getAttribute('aria-hidden'), b.textContent.split('An Nhiên').length - 1]; })()`);
  check(byline[0] >= 600 && byline[1] > byline[2] && byline[3] === 'true' && byline[4] === 1, `AuthorByline: bold name above smaller meta; the avatar is hidden and the name read once (${byline})`);
  const size = await page.eval(`document.querySelector('[data-byline=reader] .vk-ui-avatar').getBoundingClientRect().width`);
  check(size === 50, `AuthorByline: reader avatar is 50px wide on a wide screen (${size})`);

  for (const [theme, label] of [['light', 'Sáng'], ['dark', 'Tối']]) {
    await page.choose('theme', label);
    const results = [];
    for (const [entry, scenario, selector] of [
      ['action.button', 'default', '#specimen-scope .vk-ui-button'],
      ['action.filter-chip', 'selected', '#specimen-scope .vk-ui-chip'],
      ['field.text-field', 'invalid', '#specimen-scope :is(.vk-ui-input, .vk-ui-field__label, .vk-ui-field__help, .vk-ui-field__error)'],
      ['field.form', 'invalid', '#specimen-scope .vk-ui-summary :is(.vk-ui-summary__title, a)'],
      ['identity.avatar', 'default', '#specimen-scope .vk-ui-avatar'],
      ['identity.author-byline', 'default', '#specimen-scope :is(.vk-ui-byline__name, .vk-ui-byline__meta)'],
      // Disabled controls are exempt (WCAG 1.4.3 inactive UI); they keep a visible text reason.
      ['composition.nested', 'default', '#specimen-scope :is(.vk-ui-byline__name, .vk-ui-byline__meta, .vk-ui-input, .vk-ui-button:not([data-state=disabled]), .vk-ui-choice__target:not(:has(:disabled)) .vk-ui-choice__label)'],
    ]) {
      await page.open(entry, scenario);
      await page.eval('new Promise((r) => setTimeout(r, 200))');
      results.push([`${entry}/${scenario}`, await lowestContrast(page, selector)]);
    }
    const low = results.filter(([, r]) => r < 4.5);
    if (low.length) throw new Error(`contrast below 4.5 (${theme}): ${JSON.stringify(low)}`);
    console.log(`  ok  resolved text contrast ≥4.5 (${theme}); lowest ${Math.min(...results.map((r) => r[1])).toFixed(2)}`);
  }
  await page.choose('theme', 'Hệ thống');
}

// UIS-03: states, feedback and motion. Programmatic input over CDP; the playground's manual clock
// makes every timing deterministic. Animation checks read the Web Animations API, not pixels.
async function states(page, cdp) {
  const text = (selector) => page.eval(`document.querySelector(${JSON.stringify(selector)})?.textContent ?? null`);
  const exists = (selector) => page.eval(`Boolean(document.querySelector(${JSON.stringify(selector)}))`);
  const log = () => page.log();
  const running = (selector) => page.eval(`[...document.querySelectorAll(${JSON.stringify(selector)})].flatMap((el) => el.getAnimations()).filter((a) => a.playState === 'running').length`);
  const press = async (testid) => { await page.click(`[data-testid=${testid}] button`); await page.settle(); };
  const input = () => page.eval(`document.getElementById('playground-input').value`);
  const type = async (value) => {
    await page.eval(`(() => { const i = document.getElementById('playground-input'); i.focus(); i.value = ${JSON.stringify(value)}; i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
    await page.settle();
  };
  const layout = () => page.eval(`document.querySelector('[data-testid=playground-region] .vk-ui-region').dataset.layout`);
  const ANCHOR = 'Phố cũ mưa phùn, [Mẫu] — đang viết dở';
  // Re-opening the current hash keeps the specimen's state; pass through another entry first.
  const fresh = async (entry, scenario = 'default') => { await page.open('state.status'); await page.open(entry, scenario); };
  // The event log spans the whole session: count matches added since `base`.
  const count = async (pattern) => (await log()).match(pattern)?.length ?? 0;

  // Lawful region states render as their layout says.
  await page.open('state.loading');
  check(await page.eval(`[...document.querySelectorAll('.vk-ui-region')].every((r) => r.getAttribute('aria-busy') === 'true' && r.querySelector('.vk-ui-skeleton[aria-hidden=true]'))`), 'initial loading: skeleton is decorative and the region is aria-busy');
  check((await running('.vk-ui-skeleton')) >= 1, 'SurfacePulse runs by default');
  await page.open('state.loading', 'slow');
  check((await text('.vk-ui-notice[data-tone=info]'))?.includes('Vẫn đang tải'), 'slow loading is an info notice, not an error');
  check(!(await exists('[data-tone=error]')), 'slow loading shows no error tone');
  await page.open('state.empty', 'no-results');
  check(await exists('[data-panel="empty:no-results"] [data-intent=clear-filter]') && !(await exists('[data-tone=error]')), 'no-results is an empty panel with ClearFilter, never an error');
  await page.open('state.empty', 'low-content');
  check((await page.eval(`document.querySelectorAll('.vkp-works li').length`)) === 1, 'low content shows the one real item, nothing duplicated');
  for (const [scenario, panel, intents] of [
    ['default', 'error:network', ['retry', 'return']],
    ['forbidden', 'error:forbidden', ['return']],
    ['not-found', 'error:not-found', ['return']],
    ['auth-required', 'error:auth-required', ['return']],
    ['rate-limited', 'error:rate-limited', ['retry']],
  ]) {
    await page.open('state.error', scenario);
    const offered = await page.eval(`[...document.querySelectorAll('[data-panel="${panel}"] [data-intent]')].map((e) => e.dataset.intent)`);
    check(JSON.stringify(offered) === JSON.stringify(intents), `state.error/${scenario}: panel offers ${intents.join(', ')}`);
  }
  await page.open('state.error', 'auth-required');
  check(!(await page.eval(`[...document.querySelectorAll('#specimen-scope button')].some((b) => /đăng nhập/i.test(b.textContent))`)), 'auth-required has no fake sign-in button');
  await page.open('state.error', 'unknown');
  check(!(await text('#specimen-scope')).includes('thất bại'), 'unknown error copy does not promise a certain failure');
  check((await text('.vk-ui-panel__code'))?.includes('works.list.unexpected-500'), 'a stable diagnostic code is shown');
  await page.open('state.error', 'partial');
  check(await exists('.vkp-works li') && (await text('.vk-ui-notice[data-tone=warning]'))?.includes('Một phần'), 'partial failure keeps the loaded content with a warning notice');

  // Announcements: explicit, not every notice an alert.
  await page.open('state.notice');
  check((await page.eval(`document.querySelectorAll('#specimen-scope [role=alert]').length`)) === 0, 'inline notices are polite statuses, none is role=alert');
  await page.open('state.notice', 'banner');
  check((await page.eval(`document.querySelectorAll('#specimen-scope [role=alert]').length`)) === 1, 'only the session-expired banner is assertive');
  const notices = await lowestContrast(page, '#specimen-scope .vk-ui-notice__message, #specimen-scope .vk-ui-notice__title');
  check(notices >= 4.5, `banner text contrast ≥ 4.5 (${notices.toFixed(2)})`);

  // Snackbar timing on the manual clock.
  await page.open('state.notice', 'snackbar');
  await press('snackbar-show');
  check(await exists('.vk-ui-snackbar[data-dismiss=auto]'), 'a snackbar without an action auto-dismisses');
  await press('snackbar-advance');
  check(!(await exists('.vk-ui-snackbar')), 'it closes after duration.snackbar (6000ms) on the injected clock');
  await press('snackbar-show-action');
  await press('snackbar-advance');
  check(await exists('.vk-ui-snackbar[data-dismiss=manual]'), 'a snackbar with an action waits for the person');
  const snack = await lowestContrast(page, '.vk-ui-snackbar__text, .vk-ui-snackbar__action');
  check(snack >= 4.5, `snackbar contrast ≥ 4.5 (${snack.toFixed(2)})`);

  await page.open('state.notice', 'field-summary');
  await page.click('.vk-ui-summary a');
  check((await page.eval(`document.activeElement.id`)) === 'notice-title', 'the summary link focuses its field');
  check((await page.eval(`document.getElementById('notice-title').value`)) === '   ', 'the field keeps its raw text');

  // Playground: delay, no flash, slow, retry.
  await page.open('state.playground');
  const shown = () => page.eval(`document.querySelector('[data-testid=playground-region] .vk-ui-region__skeleton')?.dataset.shown ?? null`);
  check((await shown()) === 'false', 'loading → ready: no indicator at +0ms');
  await press('playground-advance-100');
  await press('playground-advance-100');
  check((await shown()) === 'false', 'still hidden at +200ms');
  await press('playground-advance-100');
  check((await shown()) === 'true', 'skeleton appears at +300ms (duration.loading-delay)');
  await press('playground-step');
  check((await layout()) === 'content|-|-' && await exists('[data-testid=playground-region] .vkp-works'), 'step → content');
  await fresh('state.playground');
  await press('playground-step');
  check((await layout()) === 'content|-|-', 'a load finishing before the delay never showed a skeleton');
  await fresh('state.playground');
  await press('playground-advance-8000');
  check((await layout()) === 'skeleton|-|slow', 'after duration.slow-notice the region adds a slow notice, still loading');

  await page.open('state.playground', 'loading-error-retry');
  await type(ANCHOR);
  await press('playground-step');
  check((await layout()) === 'panel:error:network|-|-', 'loading → error panel');
  let base = await count(/intent retry/g);
  await page.click('[data-testid=playground-region] [data-intent=retry] button');
  await page.settle();
  check((await count(/intent retry/g)) - base === 1, 'Retry dispatches the caller intent exactly once');
  check((await layout()).startsWith('skeleton'), 'retry reloads (caller policy)');
  await press('playground-step');
  check((await layout()) === 'content|-|-' && (await input()) === ANCHOR, 'error → retry → content, input kept');

  // Refresh keeps content, focus and scroll.
  await page.open('state.playground', 'refresh-partial');
  await page.eval(`document.querySelector('[data-testid=playground-region] .vkp-works a').focus()`);
  const before = await page.eval(`[document.activeElement.textContent, scrollY]`);
  await page.eval(`document.querySelector('[data-testid=playground-step] button').click()`);
  await page.settle();
  check((await layout()) === 'content|+|-', 'refresh keeps content with EdgeProgress');
  check(JSON.stringify(await page.eval(`[document.activeElement.textContent, scrollY]`)) === JSON.stringify(before), 'refresh keeps focus and scroll position');
  await page.eval(`document.querySelector('[data-testid=playground-step] button').click()`);
  await page.settle();
  check((await layout()) === 'content|-|partial' && (await page.eval(`document.activeElement.textContent`)) === before[0], 'partial failure keeps content and focus');

  // Pending → cancel, late acknowledgment ignored.
  await page.open('state.playground', 'pending-cancel');
  const width = () => page.eval(`document.querySelector('[data-testid=playground-action] button').getBoundingClientRect().width`);
  const idleWidth = await width();
  base = await count(/gửi vé/g);
  await press('playground-action');
  check((await page.eval(`document.querySelector('[data-testid=playground-action] button').getAttribute('aria-busy')`)) === 'true' && await exists('[data-testid=playground-action] .vk-ui-button__status .vk-ui-arc'), 'pending: aria-busy and a MicroArc in the status slot');
  check(Math.abs((await width()) - idleWidth) < 0.5, 'the pending button keeps its width');
  await press('playground-action');
  check((await count(/gửi vé/g)) - base === 1, 'a second activation while pending sends nothing');
  await press('playground-cancel');
  check((await page.eval(`document.querySelector('[data-testid=playground-status] .vk-ui-status').dataset.phase`)) === 'cancelled' && !(await exists('[data-testid=playground-status] [data-tone=error]')), 'cancel is neutral, not a red error');
  check(await exists('.vk-ui-snackbar'), 'cancel confirms with a snackbar');
  await press('playground-step');
  check((await log()).includes('ack muộn') && (await page.eval(`document.querySelector('[data-testid=playground-status] .vk-ui-status').dataset.phase`)) === 'cancelled', 'a late acknowledgment of the cancelled ticket shows no "saved"');
  await press('playground-advance-8000');
  check(!(await exists('.vk-ui-snackbar')), 'the snackbar closed on the clock');

  // Uncertain create: reconcile before a deliberate resend; nothing replays.
  await page.open('state.playground', 'submit-uncertain');
  await type(ANCHOR);
  base = await count(/gửi vé/g);
  await press('playground-action');
  await press('playground-step');
  check((await page.eval(`document.querySelector('[data-testid=playground-status] .vk-ui-status').dataset.phase`)) === 'uncertain', 'unknown outcome → uncertain');
  check((await page.eval(`document.querySelector('[data-testid=playground-action] button').getAttribute('aria-disabled')`)) === 'true', 'resend is blocked until reconciled');
  check(!(await exists('[data-testid=playground-notice] [data-intent=retry]')), 'no Retry is offered for an uncertain write');
  await press('playground-action');
  await page.click('[data-testid=playground-notice] [data-intent=reconcile] button');
  await page.settle();
  check((await text('[data-testid=playground-action] button'))?.includes('Gửi lại có chủ ý'), 'after reconcile the resend is a deliberate action');
  check((await count(/gửi vé/g)) - base === 1 && (await input()) === ANCHOR, 'no automatic replay; input kept');

  // Conflict: text kept, never "saved".
  await page.open('state.playground', 'save-conflict');
  await type(ANCHOR);
  check((await page.eval(`document.querySelector('[data-testid=playground-status] .vk-ui-status').dataset.phase`)) === 'dirty', 'typing marks the action dirty');
  await press('playground-action');
  await press('playground-step');
  check((await text('[data-testid=playground-notice]'))?.includes('Bản trên máy chủ đã thay đổi') && (await input()) === ANCHOR, 'conflict keeps the text and explains');
  check(!(await text('#specimen-scope')).includes('Đã lưu (máy chủ xác nhận)'), 'conflict never shows a saved status');

  // Session expired while writing: decision required, text kept, no fake sign-in.
  await page.open('state.playground', 'session-expired');
  await type(ANCHOR);
  await press('playground-step');
  check(await page.eval(`document.getElementById('playground-session').open`), 'session expired while writing opens an alert dialog');
  await page.key('Escape');
  check(await page.eval(`document.getElementById('playground-session').open`), 'Escape cannot dismiss a required decision');
  check(!(await page.eval(`[...document.querySelectorAll('#playground-session button')].some((b) => /đăng nhập/i.test(b.textContent))`)), 'no fake sign-in button');
  await page.click('[data-testid=playground-stay] button');
  await page.settle();
  check(!(await page.eval(`document.getElementById('playground-session').open`)) && (await input()) === ANCHOR, 'staying closes the dialog with the text kept');

  // Dialog focus: initial safe focus, trap, Escape, return.
  await page.open('state.dialog');
  await page.eval(`document.querySelector('[data-testid=dialog-open] button').focus()`);
  await page.key('Enter');
  check((await page.eval(`document.activeElement.textContent`)) === 'Giữ bản nháp', 'destructive confirmation focuses the safe choice first');
  for (let i = 0; i < 4; i += 1) await page.key('Tab');
  check(await page.eval(`document.getElementById('delete-dialog').contains(document.activeElement) || document.activeElement === document.body`), 'Tab stays inside the modal dialog');
  await page.key('Escape');
  check(!(await page.eval(`document.getElementById('delete-dialog').open`)), 'Escape closes an EscapeOnly dialog');
  check((await page.eval(`document.activeElement.textContent`)) === 'Xóa bản nháp…', 'closing returns focus to the trigger');
  await page.click('[data-testid=dialog-open] button');
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: 4, y: 4, button: 'left', clickCount: 1 });
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 4, y: 4, button: 'left', clickCount: 1 });
  await page.settle();
  check(await page.eval(`document.getElementById('delete-dialog').open`), 'an outside click does not close an EscapeOnly dialog');
  await page.click('[data-testid=dialog-keep] button');
  await page.open('state.dialog', 'sheet');
  await page.click('[data-testid=dialog-open] button');
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: 4, y: 4, button: 'left', clickCount: 1 });
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 4, y: 4, button: 'left', clickCount: 1 });
  await page.settle();
  check(!(await page.eval(`document.getElementById('sort-sheet').open`)) && (await log()).includes('bấm ngoài'), 'an EscapeAndOutside sheet closes on an outside click');

  // Motion: purposes run by default; reduced, inactive and hidden stop every loop.
  const LOOPS = '#specimen-scope .vk-ui-arc svg, #specimen-scope .vk-ui-edge__bar, #specimen-scope .vk-ui-skeleton';
  await page.open('state.progress');
  check((await running('#specimen-scope .vk-ui-arc svg')) >= 2 && (await running('#specimen-scope .vk-ui-edge__bar')) === 1, 'MicroArc and EdgeProgress run by default');
  const widths = await page.eval(`['button-enabled', 'button-pending'].map((t) => document.querySelector('[data-testid=' + t + '] button').getBoundingClientRect().width)`);
  check(Math.abs(widths[0] - widths[1]) < 0.5, `pending keeps the button width (${widths.map((w) => w.toFixed(1)).join(' = ')})`);
  const overlap = await page.eval(`(() => { const b = document.querySelector('[data-testid=button-pending] button'); const l = b.querySelector('.vk-ui-button__label').getBoundingClientRect(); const s = b.querySelector('.vk-ui-arc').getBoundingClientRect(); return s.left < l.right && l.left < s.right; })()`);
  check(!overlap, 'the MicroArc does not cover the label');
  // Timestamped frames: the animation clock advances between samples.
  const frames = [];
  for (let i = 0; i < 3; i += 1) {
    frames.push(await page.eval(`(() => { const a = document.querySelector('[data-testid=arc-labelled] svg').getAnimations()[0]; return { t: performance.now(), current: a.currentTime, rotate: getComputedStyle(document.querySelector('[data-testid=arc-labelled] svg')).rotate }; })()`));
    const clip = await page.eval(`(() => { const r = document.querySelector('[data-testid=arc-labelled]').getBoundingClientRect(); return { x: r.x, y: r.y, width: Math.max(1, r.width), height: Math.max(1, r.height), scale: 1 }; })()`);
    const { data } = await page.send('Page.captureScreenshot', { format: 'png', clip });
    mkdirSync(join(args.out, 'motion'), { recursive: true });
    writeFileSync(join(args.out, 'motion', `arc-frame-${i}.png`), Buffer.from(data, 'base64'));
    await sleep(180);
  }
  writeFileSync(join(args.out, 'motion', 'arc-frames.json'), JSON.stringify(frames, null, 2));
  check(frames[2].current > frames[0].current && new Set(frames.map((f) => f.rotate)).size > 1, `MicroArc advances over time (${frames.map((f) => Math.round(f.current)).join(' → ')} ms)`);
  await page.open('state.progress', 'reduced');
  check((await running(LOOPS)) === 0, 'reduced: no loop runs');
  check(await page.eval(`document.querySelector('#specimen-scope .vk-ui-edge__label').getBoundingClientRect().width > 1`), 'reduced: EdgeProgress shows its label as text');
  await page.open('state.progress', 'inactive');
  check((await running(LOOPS)) === 0, 'inactive host: no loop runs');
  await fresh('state.progress');
  await page.emulate('prefers-reduced-motion', 'reduce');
  await page.settle();
  check((await running(LOOPS)) === 0, 'the system reduced-motion setting stops every loop');
  await page.emulate('prefers-reduced-motion', 'no-preference');
  await page.open('state.progress', 'determinate');
  check((await page.eval(`document.querySelector('#specimen-scope [role=progressbar]').getAttribute('aria-valuenow')`)) === '42' && (await running(LOOPS)) === 0, 'determinate progress reports the real value and does not loop');

  // Hidden tab: the preview injects document_visibility(); a foreground tab hides this one.
  await fresh('state.progress');
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank', background: false });
  await cdp.send('Target.activateTarget', { targetId });
  await sleep(300);
  const hidden = await page.eval('document.visibilityState');
  if (hidden === 'hidden') {
    check((await running(LOOPS)) === 0, 'hidden tab (real visibilitychange): no loop runs');
  } else {
    console.log(`  NOT_RUN  hidden-tab lifecycle: headless Chrome kept visibilityState=${hidden}`);
  }
  await cdp.send('Target.closeTarget', { targetId });
  await page.send('Page.bringToFront');
  await sleep(300);
  await page.settle();
  check((await page.eval('document.visibilityState')) === 'visible' && (await running('#specimen-scope .vk-ui-arc svg')) >= 2, 'foreground again: loops resume');

  // Announcements change only with their text (no per-frame or repeated messages).
  await fresh('state.playground');
  await page.eval(`window.__announced = []; new MutationObserver(() => { const t = document.querySelector('[data-testid=playground-region] [role=status]').textContent; if (window.__announced.at(-1) !== t) window.__announced.push(t); else window.__announced.push('REPEAT:' + t); }).observe(document.querySelector('[data-testid=playground-region] [role=status]'), { childList: true, characterData: true, subtree: true })`);
  for (let i = 0; i < 6; i += 1) await press('playground-advance-100');
  await press('playground-step');
  const announced = await page.eval('window.__announced');
  check(announced.length <= 2 && !announced.some((t) => t.startsWith('REPEAT:')), `the region status changes once per state (${JSON.stringify(announced)})`);
}

async function navigation(page) {
  const width = (selector) => page.eval(`Math.round(document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect().width)`);
  const toggle = '#nav-rail-toggle';
  const route = () => page.eval(`document.querySelector('[data-testid=nav-route]').textContent`);
  const current = (scope) => page.eval(`[...document.querySelectorAll(${JSON.stringify(scope + ' [aria-current=page]')})].map((a) => a.dataset.destination)`);
  const labels = (scope) => page.eval(`[...document.querySelectorAll(${JSON.stringify(scope + ' .vk-ui-nav-item__label')})].map((s) => s.textContent).join('|')`);

  await page.open('navigation.rail');
  check((await labels('#nav-rail-primary')) === 'Khám phá|Bàn viết' && (await labels('#nav-rail-account')) === 'Tôi', 'rail: Khám phá → Bàn viết, Tôi at the foot');
  check((await width('#nav-rail')) === 220 && JSON.stringify(await current('#nav-rail')) === '["explore"]', 'expanded rail is 220px with Khám phá current');
  check(await page.eval(`document.querySelectorAll('#specimen [role=tab], #specimen [role=tablist]').length === 0 && [...document.querySelectorAll('#nav-rail-primary a')].every((a) => a.getAttribute('href').startsWith('/'))`), 'destinations are real links, not tabs');
  await page.click('#nav-rail-primary a[data-destination=desk]');
  await page.settle();
  check((await route()).includes('desk') && JSON.stringify(await current('#nav-rail')) === '["desk"]' && (await page.log()).includes('điều hướng → desk'), 'a plain click dispatches the destination key once; the host moves aria-current');
  await page.open('navigation.rail', 'reader');
  check(JSON.stringify(await current('#nav-rail')) === '["explore"]', 'a reader page belongs to Khám phá');
  await page.open('navigation.rail', 'guest');
  check((await labels('#nav-rail-primary')) === 'Khám phá', 'the rail renders only the destinations the host grants');
  await page.open('navigation.bottom-bar');
  check((await labels('#nav-bottom')) === 'Khám phá|Bàn viết|Tôi', 'bottom bar: Khám phá → Bàn viết → Tôi');
  await page.open('navigation.scaffold', 'editor');
  check(await page.eval(`(() => { const s = document.querySelector('#nav-shell'); return s.hasAttribute('data-hide-bar') && s.hasAttribute('data-compact-rail') && s.dataset.layout === 'rail:compact'; })()`), 'editor context: compact rail, bar hidden');

  // Toggle 96 → 220 → 96 on the same page, with text, caret selection and route kept.
  await page.open('navigation.scaffold', 'toggle');
  check(await page.eval(`(() => { const s = document.querySelector('#nav-shell .vk-ui-skip-link'); const hidden = getComputedStyle(s).clipPath !== 'none'; s.focus(); const shown = getComputedStyle(s).clipPath === 'none' && s.getBoundingClientRect().width > 44; s.blur(); return hidden && shown; })()`), 'the skip link stays clipped until it has focus');
  await page.eval(`(() => { const t = document.getElementById('nav-draft'); t.focus(); t.setSelectionRange(5, 12); })()`);
  const text = await page.eval(`document.getElementById('nav-draft').value`);
  const before = await route();
  const compact = await page.eval(`(() => { const b = document.querySelector('${toggle}'); const r = b.getBoundingClientRect(); return { w: r.width, h: r.height, label: b.getAttribute('aria-label'), expanded: b.getAttribute('aria-expanded'), icon: b.querySelector('[data-icon]').dataset.icon }; })()`);
  check((await width('#nav-rail')) === 96 && compact.w >= 44 && compact.h >= 44 && compact.label === 'Mở rộng điều hướng' && compact.expanded === 'false' && compact.icon === 'menu', 'compact rail keeps a named ≥44px expand control');
  await page.capture('navigation.scaffold--toggle--light--1440--96');
  await page.click(toggle);
  await page.settle();
  check((await width('#nav-rail')) === 220 && (await page.eval(`document.querySelector('${toggle}').getAttribute('aria-label')`)) === 'Thu gọn điều hướng', 'pointer toggle expands to 220px and renames the action');
  await page.capture('navigation.scaffold--toggle--light--1440--220');
  await page.eval(`document.querySelector('${toggle}').focus()`);
  await page.key('Enter');
  check((await width('#nav-rail')) === 96, 'Enter on the toggle collapses back to 96px');
  await page.key('Space');
  check((await width('#nav-rail')) === 220, 'Space toggles too');
  await page.key('Space');
  await page.capture('navigation.scaffold--toggle--light--1440--96-again');
  check((await route()) === before && (await page.eval(`(() => { const t = document.getElementById('nav-draft'); return [t.value, t.selectionStart, t.selectionEnd]; })()`)).join('|') === `${text}|5|12`, 'toggling keeps the page, the text and its selection');
  const toggles = ((await page.log()).match(/toggle →/g) ?? []).length;
  check(toggles >= 4, `each activation dispatched once (${toggles} toggles logged)`);

  // Tooltip: focus/hover, outside the rail, Escape hides it.
  await page.eval(`document.querySelector('${toggle}').blur(); document.querySelector('${toggle}').focus()`);
  await page.settle();
  check(await page.eval(`(() => { const t = document.getElementById('nav-rail-tooltip'); const r = t.getBoundingClientRect(); return !t.hidden && r.left >= document.getElementById('nav-rail').getBoundingClientRect().right && t.textContent === document.querySelector('${toggle}').getAttribute('aria-label'); })()`), 'focus shows the tooltip outside the rail with the same name');
  await page.key('Escape');
  check(await page.eval(`document.getElementById('nav-rail-tooltip').hidden`), 'Escape hides the tooltip');

  // Overlay layer: one modal, Back closes it before leaving.
  await page.open('navigation.overlay', 'busy');
  await page.waitFor(`document.getElementById('nav-profile')?.open`, 'Tôi overlay open');
  await page.click('[data-testid=nav-second] button');
  await page.settle();
  check((await page.log()).includes('từ chối: nav-profile đang mở'), 'a second modal is refused while Tôi is open');
  await page.click('[data-testid=nav-back] button');
  await page.waitFor(`!document.getElementById('nav-profile').open`, 'Back closes the overlay');
  check((await page.log()).includes('Back → đóng lớp phủ trước') && (await route()).includes('explore'), 'Back closes the overlay first and stays on the page');
  await page.click('[data-testid=nav-back-outside] button');
  await page.settle();
  check((await page.log()).includes('host rời màn'), 'the next Back goes to the host');
  await page.open('navigation.overlay');
  await page.click('#nav-profile-trigger');
  await page.waitFor(`document.getElementById('nav-profile')?.open`, 'Tôi overlay opens from the rail');
  check(await page.eval(`document.getElementById('nav-profile-trigger').getAttribute('aria-expanded') === 'true'`), 'the utility reports its open overlay');
  await page.key('Escape');
  await page.waitFor(`!document.getElementById('nav-profile').open`, 'Escape closes Tôi');
  check(await page.eval(`document.getElementById('nav-profile-trigger').getAttribute('aria-expanded') === 'false' && document.activeElement?.id === 'nav-profile-trigger'`), 'Escape closes Tôi and returns focus to its trigger');
}

async function search(page) {
  const type = async (text) => {
    await page.eval(`(() => { const i = document.querySelector('.vkp-search input'); i.value = ${JSON.stringify(text)}; i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
    await page.settle();
    return page.eval(`[...document.querySelectorAll('[data-catalog-link]')].map((a) => a.dataset.catalogLink)`);
  };
  const motion = await type('chuyen dong');
  check(motion.includes('foundation.motion'), 'search ignores Vietnamese marks (chuyen dong → Chuyển động)');
  const scenario = await type('DẤU TỔ HỢP');
  check(scenario.includes('foundation.typography') && scenario.includes('text.text'), 'search covers scenario labels');
  await type('zzz');
  check(await page.eval(`document.querySelector('.vkp-empty')?.textContent === 'Không có mục khớp.'`), 'an unmatched search says so');
  await type('');
}

async function reflow(page) {
  // One navigation per scenario and scale, then every width: Chrome throttles rapid hash changes.
  for (const scale of ['100%', '200%']) {
    await page.choose('scale', scale);
    for (const entry of catalog.entries) {
      for (const scenario of entry.scenarios) {
        await page.open(entry.id, scenario.id);
        for (const width of WIDTHS) {
          await page.viewport(width);
          await page.settle();
          const overflow = await page.eval(`document.documentElement.scrollWidth - innerWidth`);
          if (overflow > 0) throw new Error(`horizontal overflow ${overflow}px at ${width}px/${scale} on ${entry.id}/${scenario.id}`);
        }
      }
    }
    console.log(`  ok  no horizontal page overflow at ${WIDTHS.join('/')}px, text ${scale}, every scenario`);
  }
  await page.choose('scale', '100%');
  await page.viewport(1440);
  const fonts = await page.eval(`document.fonts.ready.then(() => ['16px "Be Vietnam Pro"', '600 16px "Be Vietnam Pro"', '16px "Literata"'].every((f) => document.fonts.check(f)))`);
  check(fonts, 'self-hosted Be Vietnam Pro 400/600 and Literata load');
}

async function screenPatterns(page) {
  const text = 'Góp ý chưa gửi\n\n  giữ nguyên khổ thơ. Tiếng Việt: Ỗ Ữ Đường, Ỗ Ữ.';
  const intentCount = () => page.eval(`[...document.querySelectorAll('[data-testid=event-log] li')].filter(n => n.textContent.includes('feedback → submit intent (sample)')).length`);
  const requests = [];
  await page.send('Network.enable');
  const listener = event => {
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent') requests.push(event.params.request.url);
  };
  page.cdp.listeners.push(listener);
  for (const [entry, scenario] of [['screen.reader', 'feedback'], ['composition.feedback', 'default'], ['composition.feedback', 'error']]) {
    await page.open(entry, scenario);
    await page.eval(`document.fonts.ready`);
    await page.eval(`(() => { const field = document.querySelector('.vk-feedback-composer textarea'); field.focus(); field.select(); })()`);
    await page.send('Input.insertText', { text });
    await page.eval(`window.feedbackField = document.querySelector('.vk-feedback-composer textarea'); feedbackField.setSelectionRange(4, 9); window.feedbackButton = document.querySelector('.vk-feedback-composer button')`);
    const count = await intentCount();
    requests.length = 0;
    // DOM click leaves the input focused so the local status update is tested without a
    // pointer moving focus. Enter/Space below separately exercise actual keyboard activation.
    await page.eval(`feedbackButton.click()`);
    await page.settle();
    check(await intentCount() === count + 1, `${entry}/${scenario}: one local intent per submit`);
    check(await page.eval(`document.querySelector('[data-testid=sample-result]').textContent.includes('chưa gửi hoặc lưu')
      && document.activeElement === feedbackField && feedbackField === document.querySelector('.vk-feedback-composer textarea')
      && feedbackField.value === ${JSON.stringify(text)} && feedbackField.selectionStart === 4 && feedbackField.selectionEnd === 9`), `${entry}/${scenario}: local status preserves the input node, text, focus and selection`);
    check(requests.length === 0, `${entry}/${scenario}: sample submit issues no network request`);
    if (scenario === 'error') {
      check(await page.eval(`document.querySelector('.vk-ui-notice').textContent.includes('Chưa tải/lưu được')`), 'error notice remains after a sample intent');
      await page.eval(`[...document.querySelectorAll('[data-screen] button')].find(n => n.textContent === 'Thử lại').click()`);
      await page.settle();
      check(await page.eval(`document.querySelector('[data-testid=sample-result]').textContent.includes('không gọi dịch vụ') && feedbackField.value === ${JSON.stringify(text)} && document.activeElement === feedbackField && feedbackField.selectionStart === 4 && feedbackField.selectionEnd === 9`), 'sample retry keeps text, focus and selection without claiming a save');
    }
    for (const key of ['Enter', 'Space']) {
      await page.eval(`feedbackButton.focus()`);
      const before = await intentCount();
      await page.key(key);
      check(await intentCount() === before + 1 && await page.eval(`document.activeElement === feedbackButton && feedbackField.value === ${JSON.stringify(text)} && feedbackField.selectionStart === 4 && feedbackField.selectionEnd === 9`), `${entry}/${scenario}: ${key} emits once and keeps button focus and draft selection`);
    }
    check(requests.length === 0, `${entry}/${scenario}: keyboard and retry issue no network request`);
    await page.eval(`feedbackField.focus(); document.querySelector('input[name=theme][data-label="Tối"]').click()`);
    await page.viewport(320);
    check(await page.eval(`document.activeElement === feedbackField && feedbackField === document.querySelector('.vk-feedback-composer textarea') && feedbackField.value === ${JSON.stringify(text)} && feedbackField.selectionStart === 4 && feedbackField.selectionEnd === 9`), `${entry}/${scenario}: theme and resize preserve input focus/selection`);
  }
  for (const state of ['pending', 'auth-required', 'uncertain']) {
    await page.open('composition.feedback', state);
    const count = await intentCount();
    await page.eval(`document.querySelector('.vk-feedback-composer button').focus(); document.querySelector('.vk-feedback-composer button').click()`);
    await page.key('Enter');
    await page.key('Space');
    check(await page.eval(`document.querySelector('.vk-feedback-composer button').getAttribute('aria-disabled') === 'true' && !document.querySelector('[data-testid=sample-result]').textContent`), `feedback ${state} cannot submit`);
    check(await intentCount() === count, `feedback ${state} emits no local intent`);
  }
  page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener), 1);
  await page.send('Network.disable');
  await page.choose('theme', 'Sáng');
  await page.viewport(1440);
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
  for (const [theme, label] of [['light', 'Sáng'], ['dark', 'Tối']]) {
    await page.choose('theme', label);
    for (const width of [390, 1440]) {
      await page.viewport(width);
      for (const entry of COMPONENTS) {
        await page.open(entry.id);
        await page.capture(`${entry.id}--default--${theme}--${width}`);
      }
    }
  }
  await page.choose('theme', 'Sáng');
  await page.viewport(390);
  for (const [entry, scenario] of [
    ['action.button', 'disabled'], ['action.button', 'pending'], ['field.text-field', 'invalid'], ['field.text-field', 'read-only'],
    ['field.text-area', 'multiline'], ['field.form', 'invalid'], ['identity.avatar', 'image-error'], ['identity.avatar', 'absent-identity'],
    ['identity.avatar', 'long-label'], ['identity.avatar-group', 'absent-identity'], ['identity.image-frame', 'image-error'], ['composition.nested', 'narrow'],
  ]) {
    await page.open(entry, scenario);
    await page.capture(`${entry}--${scenario}--light--390`);
  }
  for (const entry of catalog.entries.filter((e) => e.family === 'state' || e.family === 'navigation')) {
    for (const scenario of entry.scenarios.slice(1)) {
      await page.open(entry.id, scenario.id);
      await page.capture(`${entry.id}--${scenario.id}--light--390`);
    }
  }
  // UIS-04 at the widths their frames are about, and long labels at 200% text.
  for (const [entry, scenario, width, scale] of [
    ['navigation.overlay', 'open', 1440, '100%'], ['navigation.scaffold', 'short', 834, '100%'],
    ['navigation.rail', 'long-labels', 834, '200%'], ['navigation.bottom-bar', 'long-labels', 320, '200%'],
  ]) {
    await page.choose('scale', scale);
    await page.viewport(width);
    await page.open(entry, scenario);
    await page.capture(`${entry}--${scenario}--light--${width}${scale === '200%' ? '--text200' : ''}`);
  }
  await page.choose('scale', '100%');
  await page.viewport(390);
  // Transitions: the state after the recipe's first step, with any overlay open.
  for (const [scenario, prepare] of [
    ['loading-error-retry', ['playground-step']],
    ['save-conflict', ['playground-action', 'playground-step']],
    ['pending-cancel', ['playground-action', 'playground-cancel']],
    ['session-expired', ['playground-step']],
  ]) {
    await page.open('state.playground', scenario);
    for (const testid of prepare) {
      await page.click(`[data-testid=${testid}] button`);
      await page.settle();
    }
    await page.capture(`state.playground--${scenario}--after-step--light--390`);
  }
  for (const [theme, label] of [['light', 'Sáng'], ['dark', 'Tối']]) {
    await page.choose('theme', label);
    await page.open('state.dialog');
    await page.click('[data-testid=dialog-open] button');
    await page.settle();
    await page.capture(`state.dialog--default--open--${theme}--390`);
    await page.click('[data-testid=dialog-keep] button');
  }
  await page.choose('theme', 'Sáng');
  await page.choose('scale', '200%');
  await page.viewport(320);
  for (const entry of ['state.error', 'state.notice', 'state.playground']) {
    await page.open(entry, entry === 'state.error' ? 'long-label' : 'default');
    await page.capture(`${entry}--${entry === 'state.error' ? 'long-label' : 'default'}--light--320--text200`);
  }
  for (const entry of ['field.form', 'identity.author-byline', 'action.button']) {
    await page.open(entry);
    await page.capture(`${entry}--default--light--320--text200`);
  }
  for (const width of [320, 1440]) {
    await page.viewport(width);
    await page.open('foundation.typography', 'combining-marks');
    await page.capture(`foundation.typography--combining-marks--light--${width}--text200`);
  }
  for (const entry of catalog.entries.filter(e => e.family === 'screen' || e.id === 'composition.feedback')) {
    await page.choose('scale', '100%');
    await page.viewport(834);
    for (const scenario of entry.scenarios) {
      await page.open(entry.id, scenario.id);
      await page.capture(`${entry.id}--${scenario.id}--light--834`);
    }
    await page.choose('scale', '200%');
    for (const width of WIDTHS) {
      await page.viewport(width);
      await page.open(entry.id, entry.scenarios.some(s => s.id === 'long') ? 'long' : 'default');
      await page.capture(`${entry.id}--long--light--${width}--text200`);
    }
  }
  await page.choose('scale', '100%');
  await page.viewport(1440);
}

const chrome = await launchChrome();
let exitCode = 0;
let page;
try {
  const cdp = new Cdp(chrome.url);
  await cdp.open();
  page = await openPage(cdp);
  const version = await cdp.send('Browser.getVersion');
  await page.send('Page.navigate', { url: args.base });
  await page.waitFor(`document.documentElement.dataset.ready === 'true' && document.getElementById('specimen-scope')`, 'preview mounted');
  await structure(page);
  await themes(page);
  await forcedColors(page);
  await densityAndMotion(page);
  await components(page);
  await states(page, cdp);
  await navigation(page);
  await compactVietnameseWords(page);
  await search(page);
  await screenPatterns(page);
  await reflow(page);
  await captures(page);
  if (args.focus === '1') await focusedCaptures(page, version);
  check(page.errors.length === 0, `no console errors or exceptions${page.errors.length ? `: ${page.errors.join(' | ')}` : ''}`);
  writeFileSync(join(args.out, 'contexts.json'), JSON.stringify({ browser: version.product, userAgent: version.userAgent, captures: page.contexts }, null, 2));
  console.log(`preview-smoke: PASS (${Object.keys(page.contexts).length} captures in ${args.out})`);
} catch (error) {
  console.error(`preview-smoke: FAIL ${error.message}`);
  if (page?.errors.length) console.error(`  page errors: ${page.errors.join(' | ')}`);
  exitCode = 1;
} finally {
  const exited = chrome.child.exitCode !== null ? null : new Promise((resolve) => chrome.child.once('exit', resolve));
  chrome.child.kill();
  await Promise.race([exited, sleep(5000)]);
  rmSync(chrome.profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}
process.exit(exitCode);
