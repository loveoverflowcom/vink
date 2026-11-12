#!/usr/bin/env node
// Browser smoke for Vink SSR, hydration and the desk, driven over the Chrome DevTools Protocol.
// No npm dependencies: Node.js 22+ (global WebSocket/fetch) and a Chrome/Chromium binary.
// Run through `cargo xtask verify web`, which prepares a scratch database and the server.
//
//   node browser-smoke.mjs --base URL --chrome BIN --state FILE --phase main|after-restart|endorsement|review-withdrawn|appearance|first-paint|bottom-navigation|desk-layout|public-search|public-cache|editor-chrome
//
// Evidence limits: text is entered with Input.insertText / Input.imeSetComposition and keys with
// Input.dispatchKeyEvent, which is programmatic input, not a real OS IME or keyboard layout. One
// headless Chrome is used at desktop/tablet/mobile CSS viewport sizes. Save responses are held or failed with the Fetch domain.

import { execFileSync, spawn } from 'node:child_process';
import { checkSelectGeometry, selectCharacter } from '../../../packages/ui-system/leptos/preview/e2e/select-geometry.mjs';
import { discoveryReading, discoveryCorpus } from './discovery-smoke.mjs';
import { expressionPilot } from './expressions-smoke.mjs';
import { endorsementAsAuthor, endorsementAsReader } from './endorsement-smoke.mjs';
import { authWeb } from './auth-smoke.mjs';
import { reviewWithdrawnPage } from './review-withdrawn-smoke.mjs';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

const SAMPLE_WORK = '33333333-3333-4333-8333-333333333333';
const BUILD_SHA = process.env.GITHUB_SHA ?? execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
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
const LIGHT_SYSTEM = { name: 'prefers-color-scheme', value: 'light' };
const CTRL = 2;
const META = 4;
const SHIFT = 8;
const KEYS = {
  Enter: { code: 'Enter', keyCode: 13, text: '\r' },
  Escape: { code: 'Escape', keyCode: 27 },
  Tab: { code: 'Tab', keyCode: 9 },
  Space: { code: 'Space', keyCode: 32, key: ' ', text: ' ' },
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

async function searchBarGeometry(page, selector, scenario) {
  const result = await page.eval(`(() => {
    const input = document.querySelector(${JSON.stringify(selector)}), label = input.labels[0];
    const shell = input.closest('.vk-ui-search__shell'), s = shell.getBoundingClientRect();
    const controls = [...shell.querySelectorAll('button,a[href]')];
    const a = input.getBoundingClientRect(), l = label.getBoundingClientRect();
    const hidden = getComputedStyle(label);
    return { width: s.width, inputWidth: a.width, firstContentTop: document.querySelector('.vk-public-row,#draft-results')?.getBoundingClientRect().top,
      ok: a.width >= 100 && s.height >= 56 && a.left >= s.left && a.right <= s.right &&
        controls.every(el => { const r=el.getBoundingClientRect(); return r.width>=48 && r.height>=48 &&
          r.left>=s.left && r.right<=s.right && r.top>=s.top && r.bottom<=s.bottom && (r.right<=a.left || r.left>=a.right); }) &&
        input.placeholder===label.textContent && l.width<=1 && l.height<=1 && hidden.position==='absolute' && hidden.display!=='none' &&
        input.getAttribute('aria-label')===null && input.getAttribute('aria-hidden')===null &&
        !shell.querySelector('.vk-ui-field__help,details,[aria-haspopup]') &&
        shell.querySelectorAll('input').length===1 && shell.querySelectorAll('button[type=submit]').length===1 &&
        getComputedStyle(shell).boxShadow==='none' && document.documentElement.scrollWidth<=innerWidth };
  })()`);
  check(result.ok, `${scenario}: one-line search with hidden native name, separate >=48px actions and no horizontal overflow (${result.inputWidth}px query)`);
}

async function searchBarFocus(page) {
  for (const forced of [false, true]) {
    await page.send('Emulation.setEmulatedMedia', {features: [LIGHT_SYSTEM,
      {name: 'forced-colors', value: forced ? 'active' : 'none'},
      {name: 'prefers-reduced-motion', value: 'reduce'}]});
    for (const selector of ['#public-query', '#clear-public-search', '#public-search button[type=submit]']) {
      await page.eval(`document.querySelector(${JSON.stringify(selector)}).focus()`);
      await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
      const focus = await page.eval(`(() => {
        const input=document.querySelector('#public-query'), shell=input.closest('.vk-ui-search__shell'), active=document.activeElement;
        const outline=el=>getComputedStyle(el).outlineStyle==='none' ? 0 : parseFloat(getComputedStyle(el).outlineWidth);
        return {active:active===document.querySelector(${JSON.stringify(selector)}), input:outline(input), shell:outline(shell), target:outline(active)};
      })()`);
      check(focus.active && focus.input===0 && (selector==='#public-query' ? focus.shell===2 : focus.shell===0 && focus.target===2),
        `${forced ? 'forced colors' : 'tonal'} / reduced motion / ${selector}: exactly one focus perimeter ${JSON.stringify(focus)}`);
    }
    await page.capture(`search-focus-${forced ? 'forced-colors' : 'tonal'}-320-200`);
  }
  await page.send('Emulation.setEmulatedMedia', {features: [LIGHT_SYSTEM]});
}

async function quietListSurfaces(page, selector, scenario) {
  check(await page.eval(`(() => {
    const rows = [...document.querySelectorAll(${JSON.stringify(selector)})];
    const flat = el => {
      const s = getComputedStyle(el);
      return ['Top','Right','Bottom','Left'].every(side => parseFloat(s['border'+side+'Width'])===0) &&
        s.boxShadow==='none' && s.backgroundImage==='none';
    };
    return rows.length>0 && rows.every(el => flat(el) && flat(el.parentElement) &&
      el.getBoundingClientRect().height>=48 && el.querySelectorAll('button,input,a').length===0) &&
      rows.every((el,i) => i===0 || el.getBoundingClientRect().top>rows[i-1].getBoundingClientRect().bottom) &&
      !document.querySelector('#feed-end');
  })()`), `${scenario}: full-row links are separated by space and flat tone, without row rules or endpoint chrome`);
}

async function bottomNavigationGeometry(page, scenario) {
  if (!await page.eval(`document.querySelector('.vk-ui-bottom-nav').getClientRects().length > 0`)) return;
  await page.eval(`document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))`);
  await page.waitFor(`Math.abs(parseFloat(getComputedStyle(document.querySelector('.vk-ui-scaffold')).getPropertyValue('--vk-ui-bar-height')) - document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().height) < 1`, 'bottom bar resize clearance');
  check(await page.eval(`(() => {
    const bar = document.querySelector('.vk-ui-bottom-nav'), b = bar.getBoundingClientRect();
    const links = [...bar.querySelectorAll('a')];
    return links.map(a => a.querySelector('.vk-ui-nav-item__label').textContent).join('|') === 'Khám phá|Bàn viết|Tôi' && links.every(a => {
      const r = a.getBoundingClientRect(), label = a.querySelector('.vk-ui-nav-item__label');
      const l = label.getBoundingClientRect(), range = document.createRange();
      range.selectNodeContents(label);
      return r.width >= 44 && r.height >= 44 && r.left >= b.left && r.right <= b.right &&
        r.top >= b.top && r.bottom <= b.bottom && label.scrollWidth <= label.clientWidth + 1 &&
        l.height <= parseFloat(getComputedStyle(label).lineHeight) * 2 + 1 &&
        [...range.getClientRects()].every(t => t.left >= r.left - 1 && t.right <= r.right + 1 && t.top >= r.top && t.bottom <= r.bottom);
    });
  })()`), `${scenario}: all three labels fit in at most two lines inside targets >=44px`);
  check(await page.eval(`document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().height <= innerHeight * 0.18`), `${scenario}: bottom navigation leaves at least 82% of viewport height for the page`);
}

// Compare the icon row and the first rendered text line, never the centre of the
// whole label: one- and two-line labels intentionally have different heights.
async function bottomNavigationAlignment(page, scenario) {
  if (!await page.eval(`document.querySelector('.vk-ui-bottom-nav').getClientRects().length > 0`)) return;
  await bottomNavigationGeometry(page, scenario);
  const geometry = await page.eval(`(() => [...document.querySelectorAll('.vk-ui-bottom-nav a')].map(a => {
    const target = a.getBoundingClientRect(), icon = a.querySelector('.vk-ui-icon').getBoundingClientRect();
    const label = a.querySelector('.vk-ui-nav-item__label'), style = getComputedStyle(label);
    const range = document.createRange(); range.selectNodeContents(label);
    const first = [...range.getClientRects()].find(r => r.width > 0 && r.height > 0);
    return { name: label.textContent, x: target.x, y: target.y, width: target.width, height: target.height,
      iconCenterY: icon.y + icon.height / 2, firstLineY: first?.y,
      font: style.font, lineHeight: style.lineHeight };
  }))()`);
  check(geometry.length === 3 && geometry.every(g => Number.isFinite(g.firstLineY)) &&
    ['y', 'width', 'height', 'iconCenterY', 'firstLineY'].every(key =>
      Math.max(...geometry.map(g => g[key])) - Math.min(...geometry.map(g => g[key])) <= 1) &&
    geometry.every(g => g.font === geometry[0].font && g.lineHeight === geometry[0].lineHeight),
  `${scenario}: equal targets, icon centres and first label lines align within 1px: ${JSON.stringify(geometry)}`);
  return geometry;
}

async function bottomNavigationRegression(page) {
  for (const theme of ['dark', 'light']) {
    await page.viewport(320);
    await page.goto(`${args.base}/toi`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'navigation hydration');
    await chooseTheme(page, theme, 'page-theme');
    for (const width of [320, 390]) {
      await page.viewport(width);
      for (const scale of [100, 200]) {
        let reference;
        for (const route of ['/toi', '/', '/ban-viet']) {
          await page.goto(`${args.base}${route}`);
          await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'navigation route hydration');
          await page.eval(`document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
          const name = `nav-${route === '/' ? 'explore' : route.slice(1)}-${width}-${scale}-${theme}`;
          await page.capture(name);
          const geometry = await bottomNavigationAlignment(page, name);
          // Routes may add/remove the desktop scrollbar; compare their vertical tracks.
          if (reference) check(geometry.every((g, i) => ['y', 'height', 'iconCenterY', 'firstLineY'].every(k => Math.abs(g[k] - reference[i][k]) <= 1)), `${name}: selected route preserves geometry`);
          reference = geometry;
          await page.eval(`document.querySelector('.vk-ui-bottom-nav a').focus()`);
          await page.key('Tab');
          check(await page.eval(`document.activeElement === document.querySelectorAll('.vk-ui-bottom-nav a')[1] && getComputedStyle(document.activeElement).outlineStyle !== 'none'`), `${name}: keyboard order and visible focus`);
          const focused = await bottomNavigationAlignment(page, `${name} focused`);
          check(focused.every((g, i) => ['y', 'width', 'height', 'iconCenterY', 'firstLineY'].every(k => Math.abs(g[k] - geometry[i][k]) <= 1)), `${name}: focus preserves geometry`);
          await page.eval(`scrollTo(0, document.documentElement.scrollHeight)`);
          check(await page.eval(`(() => {
            const content = document.querySelector('.vk-ui-scaffold__content'), bar = document.querySelector('.vk-ui-bottom-nav');
            return parseFloat(getComputedStyle(content).paddingBottom) >= bar.getBoundingClientRect().height + 27;
          })()`), `${name}: measured content clearance follows the bar`);
        }
      }
    }
  }
  // Font-size and viewport changes above exercise ResizeObserver after hydration.
  await page.eval(`document.documentElement.style.fontSize = ''`);
  await page.viewport(834);
  await page.goto(`${args.base}/toi`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'rail hydration');
  check(await page.eval(`document.querySelector('.vk-ui-rail').getClientRects().length > 0 && document.querySelector('.vk-ui-bottom-nav').getClientRects().length === 0`), '834px keeps the rail and hides the bottom bar');
  await page.capture('nav-rail-834');
  await page.viewport(320);
  await page.goto(`${args.base}/ban-viet/${SAMPLE_WORK}`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'editor hydration');
  check(await page.eval(`document.querySelector('.vk-ui-scaffold').hasAttribute('data-hide-bar') && document.querySelector('.vk-ui-bottom-nav').getClientRects().length === 0`), 'mobile editor still hides the bottom bar');
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

  async capture(name, evidence = {}) {
    await this.eval(`document.fonts.ready`);
    const { data } = await this.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    const dir = join(dirname(args.state), 'screenshots');
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, `${name}.png`), Buffer.from(data, 'base64'));
    const context = await this.eval(`(() => {
      const geometry = selector => {
        const el = document.querySelector(selector);
        if (!el || !el.getClientRects().length) return null;
        const r = el.getBoundingClientRect(), style = getComputedStyle(el);
        return { x: r.x, y: r.y, width: r.width, height: r.height, fontSize: style.fontSize, lineHeight: style.lineHeight };
      };
      const rowTitle = document.querySelector('.vk-draft-row__title');
      const titleLines = rowTitle ? (() => {
        const range = document.createRange(); range.selectNodeContents(rowTitle);
        return new Set([...range.getClientRects()].filter(r => r.width > 0).map(r => Math.round(r.top))).size;
      })() : null;
      return { route: location.pathname, width: innerWidth, layoutWidth: document.documentElement.clientWidth, height: innerHeight, dpr: devicePixelRatio, scrollX, scrollY,
        theme: document.documentElement.dataset.theme || 'system', fontSize: getComputedStyle(document.documentElement).fontSize, userAgent: navigator.userAgent,
        appearance: { effectiveTheme: document.documentElement.dataset.theme || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'), osTheme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light', hydrated: document.documentElement.dataset.hydrated === 'true', rail: geometry('.vk-ui-rail'), railScrollWidth: document.querySelector('.vk-ui-rail')?.scrollWidth, railClientWidth: document.querySelector('.vk-ui-rail')?.clientWidth, pageChoice: document.querySelector('#page-theme input:checked')?.value, overlayChoice: document.querySelector('#overlay-theme input:checked')?.value, overlayOpen: document.querySelector('#profile-menu')?.open, overlayScroll: document.querySelector('#profile-menu')?.scrollTop, overlay: geometry('#profile-menu'), account: geometry('#page-account'), themeGroup: geometry('#page-theme') },
        mePanel: (() => {
          const panel = document.querySelector('#profile-menu');
          const prefix = panel?.open ? 'overlay' : document.querySelector('#me') ? 'page' : null;
          if (!prefix) return null;
          const bounds = selector => {
            const el = document.querySelector(selector);
            if (!el || !el.getClientRects().length) return null;
            const r = el.getBoundingClientRect(), s = getComputedStyle(el);
            const range = document.createRange(); range.selectNodeContents(el);
            return { ...geometry(selector), clientWidth: el.clientWidth, scrollWidth: el.scrollWidth,
              contentLeft: r.left + parseFloat(s.borderLeftWidth) + parseFloat(s.paddingLeft),
              contentWidth: el.clientWidth - parseFloat(s.paddingLeft) - parseFloat(s.paddingRight),
              paddingLeft: s.paddingLeft, paddingRight: s.paddingRight,
              textLines: new Set([...range.getClientRects()].filter(r => r.width > 0).map(r => Math.round(r.top))).size };
          };
          return { prefix, surface: bounds(prefix === 'page' ? '#me' : '#profile-menu'),
            heading: bounds(prefix === 'page' ? '#me-title' : '.vk-profile-heading'),
            account: bounds('#' + prefix + '-account'), identity: bounds('#' + prefix + '-account .vk-account-status'),
            avatar: bounds('#' + prefix + '-account .vk-ui-avatar'),
            accountText: bounds('#' + prefix + '-account .vk-account-status > div'),
            accountLabel: bounds('#' + prefix + '-account h2'),
            accountHelp: bounds('#' + prefix + '-account .vk-ui-tone-muted'),
            appearanceHeading: bounds(prefix === 'page' ? '#page-appearance-title' : '#profile-title'),
            choices: bounds((prefix === 'page' ? '#me' : '#profile-menu') + ' .vk-appearance-region'),
            group: bounds('#' + prefix + '-theme'),
            action: prefix === 'overlay' ? bounds('#open-me') : null,
            footer: prefix === 'page' ? bounds('.vk-footer') : null };
        })(),
        desk: { heading: geometry('#heading'), resume: geometry('#resume'), title: geometry('#resume h2'), disclosure: geometry('#resume-title-toggle'), metadata: geometry('#resume .vk-draft-meta'), cta: geometry('[data-testid=resume-draft]'), list: geometry('#drafts'), tools: geometry('#draft-tools'), search: geometry('#draft-search'), sort: geometry('#draft-sort'), refresh: geometry('#refresh-drafts'), scope: geometry('#draft-scope'), firstRow: geometry('[data-testid=draft-row]'), rowLink: geometry('.vk-draft-row__link'), rowTitle: geometry('.vk-draft-row__title'), rowTitleLines: titleLines, rowMetadata: geometry('.vk-draft-row .vk-draft-meta'), rowOpen: geometry('.vk-draft-row__open'), results: geometry('#draft-results'), listHeading: geometry('#draft-list-title'), searchField: geometry('.vk-draft-search > .vk-ui-field'), sortField: geometry('#draft-tools > .vk-ui-field:nth-child(2)') },
        reader: { title: geometry('#reader-header h1'), header: geometry('#reader-header'), body: geometry('.vk-reader .vk-document') },
        editor: { status: document.querySelector('[data-testid=save-status]')?.dataset.status, toolbar: geometry('#editor-toolbar'), save: geometry('#save'), title: geometry('#draft-title'), body: geometry('[data-testid=editor-body]'), preview: geometry('#draft-preview[open]'), leave: geometry('#leave-editor[open]') },
        explore: { heading: geometry('#explore-heading'), tools: geometry('#discovery-tools'), brand: geometry('.vk-brand'), search: geometry('#public-search'), input: geometry('#public-query'), label: geometry('#public-query-label'), shell: geometry('#public-search .vk-ui-search__shell'), clear: geometry('#clear-public-search'), submit: geometry('#public-search button[type=submit]'), stream: geometry('#community-stream'), firstRow: geometry('.vk-public-row'), byline: geometry('.vk-public-row .vk-work-byline'), author: geometry('.vk-public-row .vk-ui-byline__name'), date: geometry('.vk-public-row time'), title: geometry('.vk-public-row h3'), context: geometry('#context') },
        navigation: { bar: geometry('.vk-ui-bottom-nav'), labels: [...document.querySelectorAll('.vk-ui-bottom-nav a')].map(a => ({ label: a.textContent, target: geometry('#bottom-navigation a[href="' + a.getAttribute('href') + '"]'), icon: geometry('#bottom-navigation a[href="' + a.getAttribute('href') + '"] .vk-ui-icon'), text: geometry('#bottom-navigation a[href="' + a.getAttribute('href') + '"] .vk-ui-nav-item__label'), firstLine: (() => { const range = document.createRange(); range.selectNodeContents(a.querySelector('.vk-ui-nav-item__label')); const r = [...range.getClientRects()].find(r => r.width > 0 && r.height > 0); return r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null; })() })) },
      };
    })()`);
    const manifest = join(dir, 'contexts.json');
    const contexts = readdirSync(dir).includes('contexts.json') ? JSON.parse(readFileSync(manifest, 'utf8')) : {};
    contexts[name] = { ...context, captured_at: new Date().toISOString(), evidence: { build: BUILD_SHA, ...evidence } };
    writeFileSync(manifest, JSON.stringify(contexts, null, 2));
    return context;
  }

  async click(selector) {
    // A committed CodeMirror input can still have a queued selection scroll. Resolve a fresh
    // node and hit-test after layout; retry geometry only, never dispatch a second activation.
    let point;
    for (let attempt = 0; attempt < 12; attempt++) {
      point = await this.eval(`(async () => {
        const el = document.querySelector(${JSON.stringify(selector)});
        if (!el) throw new Error('Missing click target: ' + ${JSON.stringify(selector)});
        el.scrollIntoView({ block: 'center', behavior: 'instant' });
        await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
        const current = document.querySelector(${JSON.stringify(selector)});
        if (!current) return null;
        const r = current.getBoundingClientRect();
        const x = r.x + r.width / 2, y = r.y + r.height / 2;
        const hit = document.elementFromPoint(x, y);
        return r.width > 0 && r.height > 0 && hit && current.contains(hit) ? { x, y } : null;
      })()`);
      if (point) break;
    }
    if (!point) throw new Error(`click target stayed outside the pointer hit area: ${selector}`);
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
  async key(name, modifiers = 0, { settle = true } = {}) {
    const { code, keyCode, text, key = name } = KEYS[name];
    // A key that types text (Enter) needs `keyDown` with that text to activate links/buttons.
    for (const type of [text ? 'keyDown' : 'rawKeyDown', 'keyUp']) {
      await this.send('Input.dispatchKeyEvent', { type, key, code, modifiers, windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode, ...(text && type === 'keyDown' ? { text } : {}) });
    }
    // Give native selectionchange and CodeMirror's focus observer a rendering opportunity
    // before the next synthetic input, as between separate human key/pointer actions.
    // `settle: false` is for a check of what happens inside those frames.
    if (settle) await this.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
  }

  /** Native document/line movement differs on macOS; don't edit a selection through the DOM. */
  lineStart(modifiers = 0) {
    return this.key(this.mac ? 'ArrowLeft' : 'Home', (this.mac ? META : 0) | modifiers);
  }

  lineEnd(options) {
    return this.key(this.mac ? 'ArrowRight' : 'End', this.mac ? META : 0, options);
  }

  /** Focuses CodeMirror, uses the platform document-end shortcut and inserts committed text. */
  async typeInBody(text) {
    // Enter through native Tab, whose focus/selection events settle before document-end.
    // Direct DOM focus can leave Chrome's transient caret at zero while CodeMirror is
    // restoring its selection, racing the next synthetic key only a millisecond later.
    if (!await this.eval(`document.activeElement?.classList.contains('cm-content')`)) {
      await this.eval(`document.querySelector('#draft-title').focus()`);
      await this.tabToBody();
    }
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
  constructor(page, stage = 'Request') {
    this.page = page;
    this.stage = stage;
    this.enabled = false;
    this.held = [];
    page.cdp.listeners.push((message) => {
      if (!this.enabled || message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
      const { requestId, request } = message.params;
      if (request.method === 'PUT' && (stage === 'Request' || message.params.responseStatusCode !== undefined || message.params.responseErrorReason)) this.held.push(requestId);
      else page.send('Fetch.continueRequest', { requestId }).catch(() => {});
    });
  }

  enable() {
    this.enabled = true;
    return this.page.send('Fetch.enable', { patterns: [{ urlPattern: '*/api/v1/drafts/*', requestStage: this.stage }] });
  }

  disable() {
    this.enabled = false;
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
  // Headless Chrome follows the host appearance (dark on a dark-mode Mac); the checks assume a
  // light system and emulate a dark one explicitly where they need it.
  await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM] });
  page.mac = await page.eval(`/Mac/.test(navigator.platform)`);
  page.mod = page.mac ? META : CTRL;
  return page;
}

/** READ-01: the wire contract of conditional public reads, then the browser's HTTP cache
 * revalidating a real client read (wire 304) while the application still receives a full 200. */
async function conditionalPublicReads(page) {
  console.log('conditional public reads (READ-01): wire contract, then browser cache vs application value:');
  const path = `/api/v1/works/${SAMPLE_WORK}/publication`;
  const first = await fetch(`${args.base}${path}`);
  const body = await first.text();
  const etag = first.headers.get('etag');
  check(first.status === 200 && /^"[0-9a-f]{32}"$/.test(etag ?? '') && first.headers.get('cache-control') === 'private, no-cache' && !first.headers.has('vary'), 'public read carries a strong validator, private, no-cache and no Vary');
  const revalidated = await fetch(`${args.base}${path}`, { headers: { 'If-None-Match': `"other", W/${etag}` } });
  check(revalidated.status === 304 && (await revalidated.text()) === '' && revalidated.headers.get('etag') === etag && revalidated.headers.get('cache-control') === 'private, no-cache', 'raw revalidation (weak comparison, tag list) is a 304 without content');
  const malformed = await fetch(`${args.base}${path}`, { headers: { 'If-None-Match': etag.slice(1) } });
  check(malformed.status === 200 && (await malformed.text()) === body, 'a malformed If-None-Match gets the full 200');
  for (const missing of ['00000000-0000-4000-8000-000000000000', 'not-a-uuid']) {
    const response = await fetch(`${args.base}/api/v1/works/${missing}/publication`, { headers: { 'If-None-Match': '*' } });
    check(response.status === 404 && response.headers.get('cache-control') === 'no-store' && !response.headers.has('etag'), `${missing}: still 404 (no-store) with If-None-Match: *`);
  }
  const list = await fetch(`${args.base}/api/v1/works`);
  const listEtag = list.headers.get('etag');
  await list.arrayBuffer();

  const requests = new Map();
  const entry = id => requests.get(id) ?? requests.set(id, {}).get(id);
  const listener = ({ sessionId, method, params }) => {
    if (sessionId !== page.sessionId) return;
    if (method === 'Network.requestWillBeSent') entry(params.requestId).path = new URL(params.request.url).pathname;
    if (method === 'Network.requestWillBeSentExtraInfo') entry(params.requestId).ifNoneMatch = Object.entries(params.headers).find(([name]) => name.toLowerCase() === 'if-none-match')?.[1] ?? null;
    if (method === 'Network.responseReceivedExtraInfo') entry(params.requestId).wire = params.statusCode;
    if (method === 'Network.responseReceived') entry(params.requestId).reported = params.response.status;
  };
  page.cdp.listeners.push(listener);
  try {
    // The app's own client read (Explore after client navigation), twice.
    for (const round of ['first', 'second']) {
      await page.goto(`${args.base}/toi`);
      await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, `utility hydration before ${round} client read`);
      const nav = await page.eval(`innerWidth < 760`) ? '.vk-ui-bottom-nav a[href="/"]' : '#navigation-primary a[href="/"]';
      await page.click(nav);
      await page.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row a[href="/tac-pham/${SAMPLE_WORK}"]')`, `${round} client read renders the real public row`);
    }
    // What fetch() hands the application on a revalidated read.
    const seen = [];
    for (let read = 0; read < 2; read++) seen.push(await page.eval(`fetch(${JSON.stringify(path)}).then(async r => ({ status: r.status, etag: r.headers.get('etag'), body: await r.text() }))`));
    check(seen.every(read => read.status === 200 && read.etag === etag && read.body === body), 'the application receives a full 200 with the same bytes, never a raw 304');
    await sleep(200);
  } finally {
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
  }
  const reads = [...requests.values()].filter(read => read.path === '/api/v1/works');
  const details = [...requests.values()].filter(read => read.path === path);
  console.log(`  ..  /api/v1/works wire: ${JSON.stringify(reads)}; publication wire: ${JSON.stringify(details)}`);
  check(reads.length === 2 && reads[1].ifNoneMatch === listEtag && reads[1].wire === 304, 'the second client list read was revalidated on the wire (If-None-Match → 304)');
  check(details.length === 2 && details[0].wire === 200 && !details[0].ifNoneMatch && details[1].ifNoneMatch === etag && details[1].wire === 304, 'stored publication bytes were reused only after the server answered 304');
}

/** READ-02: the tab's public snapshot renders first, lifecycle signals revalidate it through the
 * browser HTTP cache, failures keep the last success and a 404 removes it. The app's monotonic
 * clock (`performance.now`) and the document visibility are driven by the test. */
async function cacheFirstPublicReads(page) {
  console.log('cache-first public reads (READ-02): snapshot first, lifecycle revalidation, failure and removal:');
  const worksPath = '/api/v1/works';
  const publicationPath = `/api/v1/works/${SAMPLE_WORK}/publication`;
  const original = await (await fetch(`${args.base}${publicationPath}`)).json();
  const seen = [];
  // ExtraInfo events may arrive before or after requestWillBeSent: merge by request id.
  const byId = new Map();
  const record = id => byId.get(id) ?? byId.set(id, {}).get(id);
  const network = ({ sessionId, method, params }) => {
    if (sessionId !== page.sessionId) return;
    if (method === 'Network.requestWillBeSent') {
      const path = new URL(params.request.url).pathname;
      if (path === worksPath || path === publicationPath) seen.push(Object.assign(record(params.requestId), { path }));
    }
    if (method === 'Network.requestWillBeSentExtraInfo') record(params.requestId).ifNoneMatch = Object.entries(params.headers).find(([name]) => name.toLowerCase() === 'if-none-match')?.[1] ?? null;
    if (method === 'Network.responseReceivedExtraInfo') record(params.requestId).wire = params.statusCode;
  };
  const count = path => seen.filter(entry => entry.path === path).length;
  // Fetch-domain fixtures: undefined = continue to the real server, 'hold' = keep pending.
  const fixtures = new Map();
  const held = [];
  const gate = ({ sessionId, method, params }) => {
    if (sessionId !== page.sessionId || method !== 'Fetch.requestPaused') return;
    const answer = fixtures.get(new URL(params.request.url).pathname);
    if (answer === 'hold') return held.push(params.requestId);
    (answer ? page.send('Fetch.fulfillRequest', {
      requestId: params.requestId, responseCode: answer.status,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify(answer.body)).toString('base64'),
    }) : page.send('Fetch.continueRequest', { requestId: params.requestId })).catch(() => {});
  };
  // Freeze cache time before validation: host scheduling must not turn 119 s into >=120 s.
  // Keep the real clock separately for the time-to-rows measurements below.
  const clock = `(window.__vinkClock ??= (() => { const realNow = performance.now.bind(performance), clock = { now: realNow(), realNow }; performance.now = () => clock.now; return clock; })())`;
  const advance = seconds => page.eval(`${clock}.now += ${seconds * 1000}`);
  const signals = events => page.eval(`${JSON.stringify(events)}.forEach(name => (name === 'visibilitychange' ? document : window).dispatchEvent(new Event(name)))`);
  const setHidden = hidden => page.eval(`Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => ${hidden ? "'hidden'" : "'visible'"} }); document.dispatchEvent(new Event('visibilitychange'))`);
  const settled = (selector, what) => page.waitFor(`document.querySelector('${selector}')?.dataset.refreshing === 'false'`, what);
  page.cdp.listeners.push(network, gate);
  try {
    await page.goto(`${args.base}/`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('.vk-public-row')`, 'hydrated Explore');
    await sleep(400);
    check(count(worksPath) === 0 && await page.eval(`document.querySelector('#public-collection').dataset.refreshing === 'false'`), 'hydrated Explore reuses its SSR snapshot: no client list request after hydration');

    await page.goto(`${args.base}/toi`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'utility hydration before the first client list read');
    await page.eval(`${clock}; true`);
    const nav = await page.eval(`innerWidth < 760`) ? '.vk-ui-bottom-nav a[href="/"]' : '#navigation-primary a[href="/"]';
    const timed = (selector, ready) => page.eval(`(async () => {
      window.__sawLoading = false;
      const watch = new MutationObserver(() => { if (document.querySelector('[data-testid=discovery-loading]')) window.__sawLoading = true; });
      watch.observe(document.body, { subtree: true, childList: true, characterData: true });
      const realNow = window.__vinkClock.realNow;
      const start = realNow();
      document.querySelector(${JSON.stringify(selector)}).click();
      while (!(${ready})) await new Promise(resolve => requestAnimationFrame(resolve));
      watch.disconnect();
      return { ms: Math.round(realNow() - start), loading: window.__sawLoading };
    })()`);
    const ROWS = `location.pathname === '/' && document.querySelector('.vk-public-row a[href="/tac-pham/${SAMPLE_WORK}"]')`;
    const first = await timed(nav, ROWS);
    check(count(worksPath) === 1 && first.loading, `a first client visit fetches once and shows its loading state (${first.ms} ms to rows)`);
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${original.revision_id}'`, 'reader by client navigation');
    const back = await timed('#reader-return', ROWS);
    await sleep(300);
    check(count(worksPath) === 1 && !back.loading, `returning within the stale threshold renders the snapshot at once with no request and no loading state (${back.ms} ms to rows)`);
    console.log(`  ..  time to rows: first client visit ${first.ms} ms (network), return ${back.ms} ms (tab snapshot)`);

    await page.eval(`document.querySelector('.vk-public-row').__kept = true; document.querySelector('.vk-public-row__link').focus()`);
    // Deliberately exceed the former one-second timing margin on even a fast host.
    await sleep(1500);
    await advance(119);
    await signals(['focus']);
    await sleep(300);
    check(count(worksPath) === 1, '119 s after validation a focus does not revalidate');
    await advance(2);
    await signals(['focus', 'online', 'visibilitychange', 'pageshow']);
    await settled('#public-collection', 'list revalidation');
    await sleep(300);
    const revalidation = seen.filter(entry => entry.path === worksPath).at(-1);
    check(count(worksPath) === 2, `four simultaneous lifecycle signals at 121 s share one request (${count(worksPath) - 1} sent)`);
    check(revalidation.ifNoneMatch && revalidation.wire === 304, `the revalidation used the browser's stored validator and the origin answered 304 (${JSON.stringify(revalidation)})`);
    check(await page.eval(`document.querySelector('.vk-public-row').__kept === true && document.activeElement?.matches('.vk-public-row__link')`), 'an unchanged refresh keeps the same row elements and focus');

    await setHidden(true);
    await advance(300);
    await signals(['focus', 'online']);
    await sleep(800);
    check(count(worksPath) === 2, 'a hidden document does not revalidate on focus/online and pauses the timer');
    await setHidden(false);
    await settled('#public-collection', 'revalidation after becoming visible');
    await sleep(300);
    check(count(worksPath) === 3, 'becoming visible with a stale snapshot sends exactly one request');

    await page.send('Fetch.enable', { patterns: [{ urlPattern: `${args.base}/api/v1/works*`, requestStage: 'Request' }] });
    fixtures.set(worksPath, { status: 503, body: { error: { code: 'dependency_unavailable' } } });
    await advance(121);
    await signals(['focus']);
    await page.waitFor(`document.querySelector('[data-testid=public-refresh-failed]')`, 'refresh failure notice');
    check(await page.eval(`document.querySelector('.vk-public-row').__kept === true && !document.querySelector('[data-testid=public-unavailable], [data-testid=public-empty]') && document.querySelector('#public-collection').getAttribute('aria-busy') === 'false'`), 'a temporary refresh failure keeps the last success on screen, separate from the unavailable/empty states');
    await page.capture('explore-refresh-failed');
    fixtures.delete(worksPath);
    await page.click('[data-testid=public-refresh-failed] button');
    await page.waitFor(`!document.querySelector('[data-testid=public-refresh-failed]') && document.querySelector('#public-collection').dataset.refreshing === 'false'`, 'manual retry recovers');
    check(await page.eval(`document.querySelector('.vk-public-row').__kept === true`), 'manual retry restores freshness without remounting the list');

    // Reader: a changed representation is offered, never swapped mid-reading; a 404 removes it.
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${original.revision_id}'`, 'reader again');
    await settled('#reader-freshness', 'reader revalidation on mount');
    const updated = { ...original, revision_id: '[mẫu-kiểm-tra]-revision', title: '[Mẫu kiểm tra] Mưa trên phố cũ (bản cập nhật)', document: { schema_version: 1, blocks: [{ lines: ['Khổ thay thế chỉ có trong fixture kiểm tra.'] }] } };
    fixtures.set(publicationPath, { status: 200, body: updated });
    await advance(121);
    await signals(['focus']);
    await page.waitFor(`document.querySelector('[data-testid=reader-update]')`, 'new version notice');
    check(await page.eval(`document.querySelector('.vk-reader').dataset.revision === '${original.revision_id}' && document.querySelector('.vk-document').textContent.includes(${JSON.stringify(PUBLISHED_LINE)})`), 'a changed publication is offered without replacing the text being read');
    await page.capture('reader-update-available');
    await page.click('[data-testid=reader-update] button');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${updated.revision_id}' && !document.querySelector('[data-testid=reader-update]')`, 'applied new version');
    check(await page.eval(`document.querySelectorAll('.vk-reader').length === 1 && document.querySelector('.vk-document').textContent.includes('Khổ thay thế') && !document.querySelector('.vk-document').textContent.includes(${JSON.stringify(PUBLISHED_LINE)})`), 'the reader applies the new version only on request, replacing the whole text once');
    await page.waitFor(`document.activeElement?.id === 'noi-dung'`, 'focus moves to the content start after applying');

    fixtures.set(publicationPath, { status: 404, body: { error: { code: 'not_found' } } });
    await advance(121);
    await signals(['focus']);
    await page.waitFor(`document.querySelector('h1')?.textContent === 'Không tìm thấy trang này.'`, 'authoritative removal');
    check(await page.eval(`!document.querySelector('.vk-document') && !document.querySelector('.vk-reader')`), 'an authoritative 404 removes the text at once instead of keeping a stale copy');
    fixtures.set(worksPath, 'hold');
    await page.click('[data-panel] a');
    await page.waitFor(`location.pathname === '/' && document.querySelector('#public-collection')`, 'Explore after removal');
    check(await page.eval(`!document.querySelector('a[href^="/tac-pham/${SAMPLE_WORK}"]')`), 'the removed work’s card is dropped from the tab snapshot while the list revalidates');
    const deadline = Date.now() + 10000;
    while (!held.length && Date.now() < deadline) await sleep(50);
    check(held.length === 1, 'returning with a stale list sends one revalidation');
    fixtures.delete(worksPath);
    fixtures.delete(publicationPath);
    await page.send('Fetch.continueRequest', { requestId: held.pop() });
    await page.waitFor(`document.querySelector('a[href^="/tac-pham/${SAMPLE_WORK}"]')`, 'origin list restores the card');
    check(true, 'the origin’s list, not the derived preview, decides membership once it answers');
  } finally {
    await page.eval(`if (window.__vinkClock) { performance.now = window.__vinkClock.realNow; delete window.__vinkClock; }`).catch(() => {});
    await page.send('Fetch.disable').catch(() => {});
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== network && item !== gate);
  }
}

async function serverHtml() {
  console.log('server HTML before any JavaScript:');
  const home = await fetch(args.base);
  const homeHtml = await home.text();
  check(home.status === 200 && homeHtml.includes('<title>Khám phá — Vink</title>') && homeHtml.includes('1 tác phẩm trên trang này.') && homeHtml.includes('vk-public-row__link'), 'Explore HTML contains its finite public collection and actual inventory before JavaScript');
  check(homeHtml.match(/class="vk-brand"/g)?.length === 1 && !homeHtml.includes('vk-ui-top-bar') && !homeHtml.includes('vk-ui-rail__brand'), 'SSR places exactly one brand on Explore and omits empty shell chrome');
  check(homeHtml.includes(`href="/tac-pham/${SAMPLE_WORK}"`) && !homeHtml.includes(DRAFT_ONLY_LINE), 'Explore uses real reader links and excludes private draft text');
  for (const query of ['mua tren pho cu', 'TÁC GIẢ MẪU'.normalize('NFD')]) {
    const response = await fetch(`${args.base}/?q=${encodeURIComponent(query)}`);
    const filtered = await response.text();
    check(response.status === 200 && filtered.includes(`href="/tac-pham/${SAMPLE_WORK}?q=`) && !filtered.includes(DRAFT_ONLY_LINE), 'SSR finds the public title or author before JavaScript without exposing a draft');
    const canonical = filtered.match(/<link\b(?=[^>]*\brel="canonical")[^>]*>/)?.[0];
    check(canonical?.includes(`href="${args.base}/?q=`), 'Explore canonical carries the public query context');
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
  const me = await (await fetch(`${args.base}/toi`)).text();
  check([html, desk, index, me].every(text => !text.includes('class="vk-brand"') && !text.includes('vk-ui-top-bar')), 'direct reader, editor, Desk and Tôi HTML has no global brand or empty header');
  check(!me.includes('Tác giả mẫu (dữ liệu phát triển)') && me.includes('data-initials=""'), 'Tôi SSR uses a neutral avatar and omits current account details');
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

const RAIL_WIDTH = `Math.round(document.querySelector('.vk-ui-rail').getBoundingClientRect().width)`;
const PROFILE_OPEN = `document.querySelector('[data-testid=profile-menu]').open`;

async function openProfile(page) {
  await page.click('#profile-trigger');
  await page.waitFor(PROFILE_OPEN, 'Tôi overlay');
}

async function chooseTheme(page, value, group = 'overlay-theme') {
  await page.click(`label:has(input[name="${group}"][value="${value}"])`);
  await page.waitFor(value === 'system' ? `!document.documentElement.hasAttribute('data-theme')` : `document.documentElement.dataset.theme === '${value}'`, `${value} appearance`);
  // Sample contrast after the current CSS transitions finish. Holding on to a snapshot
  // of Animation.finished promises stalled DevTools after the dirty editor resized.
  // Poll the live transitions instead, without waiting for CSS loading animations.
  await page.waitFor(`!document.getAnimations().some(a =>
    a instanceof CSSTransition && (a.pending || a.playState === 'running'))`, `${value} appearance transitions`);
  await assertBrand(page, value);
}

async function assertBrand(page, preference) {
  const theme = preference === 'system'
    ? await page.eval(`matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'`)
    : preference;
  await page.waitFor(`document.querySelector('#vink-favicon').getAttribute('href') === '/pkg/brand/vpen-r1-mark-${theme}.svg' && document.querySelector('#vink-manifest').getAttribute('href') === '/pkg/brand/vpen-r1-manifest-${theme}.webmanifest'`, `${theme} head icons`);
  await assertBrandPlacement(page);
  if (await page.eval(`location.pathname === '/'`)) {
    check(await page.eval(`(() => {
      const mark = document.querySelector('#discovery-tools .vk-brand__lockup');
      const style = getComputedStyle(mark), rect = mark.getBoundingClientRect();
      return style.backgroundImage.includes('vpen-r1-lockup-${theme}.svg') && style.filter === 'none' &&
        Math.abs(rect.width / rect.height - 1018 / 410) < 0.001 && rect.height === 2.25 * parseFloat(getComputedStyle(document.documentElement).fontSize) && mark.parentElement.textContent.trim() === '';
    })()`), `${theme} Explore lockup uses the approved artwork once and reserves its ratio before decoding`);
  }
}

async function assertBrandPlacement(page) {
  check(await page.eval(`(() => {
    const brands = document.querySelectorAll('.vk-brand');
    if (document.querySelector('#navigation .vk-ui-rail__brand, .vk-ui-top-bar')) return false;
    if (location.pathname !== '/') return brands.length === 0;
    return brands.length === 1 && brands[0].closest('#explore #discovery-tools') &&
      brands[0].getClientRects().length > 0 && brands[0].getAttribute('href') === '/' &&
      brands[0].getAttribute('aria-label') === 'Vink — trang chủ';
  })()`), 'brand appears once in the Explore tools, links home, and is absent on other routes');
}

async function assertExploreHeader(page, scenario) {
  await page.eval('document.fonts.ready');
  check(await page.eval(`(() => {
    const brand=document.querySelector('#discovery-tools .vk-brand').getBoundingClientRect();
    const heading=document.querySelector('#explore-heading h1').getBoundingClientRect();
    const search=document.querySelector('#public-search').getBoundingClientRect();
    return brand.width>=44 && brand.right<=innerWidth && heading.top<search.top && heading.left===search.left && document.documentElement.scrollWidth<=innerWidth;
  })()`), scenario + ': brand and title/search hierarchy reflow');
}

async function brandAssets(page) {
  console.log('V-pen R1 logo asset delivery and decoding:');
  const stylesheet = await page.eval(`document.querySelector('link[rel=stylesheet]').getAttribute('href')`);
  check(stylesheet === '/pkg/vink.css?brand=vpen-r1', 'brand migration uses a fresh stylesheet cache key');
  const cssResponse = await fetch(`${args.base}${stylesheet}`);
  const css = await cssResponse.text();
  check(cssResponse.ok && cssResponse.headers.get('content-type')?.startsWith('text/css') && css.includes('brand/vpen-r1-lockup-light.svg') && !css.includes('brand/vink-mark-'), 'versioned CSS resolves R1 artwork without historical asset URLs');
  const fallbackResponse = await fetch(`${args.base}/pkg/brand/vpen-r1-favicon.svg`);
  check(fallbackResponse.ok && fallbackResponse.headers.get('content-type')?.startsWith('image/svg+xml'), 'OS-adaptive favicon is served as SVG');
  check(await page.eval(`(async () => { const image = new Image(); image.src = '/pkg/brand/vpen-r1-favicon.svg'; await image.decode(); return image.naturalWidth > 0; })()`), 'OS-adaptive favicon decodes');
  for (const theme of ['light', 'dark']) {
    for (const size of [24, 32, 48, 96, 192, 512]) {
      const path = `/pkg/brand/vpen-r1-mark-${theme}-${size}.png`;
      const response = await fetch(`${args.base}${path}`);
      check(response.ok && response.headers.get('content-type')?.startsWith('image/png'), `${theme} ${size}px PNG is served as an image`);
      check(await page.eval(`(async () => { const image = new Image(); image.src = '${path}'; await image.decode(); return image.naturalWidth === ${size} && image.naturalHeight === ${size}; })()`), `${theme} ${size}px PNG decodes at the declared size`);
    }
    for (const variant of ['mark', 'lockup']) {
      const path = `/pkg/brand/vpen-r1-${variant}-${theme}.svg`;
      const response = await fetch(`${args.base}${path}`);
      check(response.ok && response.headers.get('content-type')?.startsWith('image/svg+xml'), `${theme}${variant} SVG is served as an image`);
      check(await page.eval(`(async () => { const image = new Image(); image.src = '${path}'; await image.decode(); return image.naturalWidth > 0 && image.naturalHeight > 0; })()`), `${theme}${variant} selected SVG decodes`);
    }
    const response = await fetch(`${args.base}/pkg/brand/vpen-r1-manifest-${theme}.webmanifest`);
    const manifest = await response.json();
    check(response.ok && response.headers.get('content-type')?.includes('manifest') && manifest.id === '/' &&
      manifest.icons.every(icon => icon.src.startsWith(`/pkg/brand/vpen-r1-mark-${theme}`) && icon.purpose === 'any') &&
      manifest.icons.some(icon => icon.sizes === '192x192') && manifest.icons.some(icon => icon.sizes === '512x512'), `${theme} manifest has stable identity and only R1 any-purpose 192/512 icons`);
    for (const icon of manifest.icons) {
      check((await fetch(`${args.base}${icon.src}`)).ok, `${theme} manifest icon ${icon.src} exists`);
    }
  }
  for (const size of [24, 32, 48, 96, 192, 512]) {
    check(await page.eval(`(async () => {
      const masks = [];
      for (const theme of ['light', 'dark']) {
        const image = new Image(); image.src = '/pkg/brand/vpen-r1-mark-' + theme + '-${size}.png';
        await image.decode();
        const canvas = document.createElement('canvas'); canvas.width = canvas.height = ${size};
        const context = canvas.getContext('2d'); context.drawImage(image, 0, 0);
        const pixels = context.getImageData(0, 0, ${size}, ${size}).data;
        masks.push(Array.from({length: ${size} * ${size}}, (_, i) => pixels[i * 4 + 3]));
      }
      return masks[0].some(alpha => alpha > 0) && masks[0][0] === 0 &&
        masks[0].every((alpha, i) => alpha === masks[1][i]);
    })()`), `${size}px Light/Dark PNG has identical silhouette/alpha and transparent corners`);
  }
  for (const legacy of ['favicon.svg', 'vink-mark-light.svg', 'vink-mark-dark-compact.svg', 'vink-light-192.png']) {
    check((await fetch(`${args.base}/pkg/brand/${legacy}`)).status === 404, `historical ${legacy} is not served`);
  }
  check(await page.eval(`(async () => {
    const xml = new DOMParser();
    const svg = async name => xml.parseFromString(await (await fetch('/pkg/brand/' + name)).text(), 'image/svg+xml');
    const fallback = await svg('vpen-r1-favicon.svg');
    for (const theme of ['light', 'dark']) {
      const source = await svg('vpen-r1-mark-' + theme + '.svg');
      const paths = node => [...node.querySelectorAll('path')].map(p => p.getAttribute('d'));
      if (JSON.stringify(paths(source)) !== JSON.stringify(paths(fallback.querySelector('g.' + theme)))) return false;
    }
    return true;
  })()`), 'no-JS favicon contains exactly the original V-pen paths in both themes');
  // Suppress both hydration assets to inspect the real server document with CSS +
  // appearance bootstrap. Leaving the WASM preload active while its JS consumer is
  // blocked strands an unconsumed fetch across these rapid navigations, stalling
  // the next document's same-URL fetch even after its DOM has finished loading.
  // Restore both assets before the unchanged hydration/interaction assertions.
  // Leptos's hydration script does not catch its `import()`, so each blocked document reports
  // exactly that rejected import; account for those here and keep any other error.
  const blockedImport = `TypeError: Failed to fetch dynamically imported module: ${args.base}/pkg/vink.js`;
  const errorsBefore = page.errors.length;
  await page.send('Network.setBlockedURLs', { urls: [`${args.base}/pkg/vink.js*`, `${args.base}/pkg/vink_bg.wasm*`] });
  for (const [theme, system] of [['light', 'light'], ['dark', 'light'], ['system', 'dark'], ['system', 'light']]) {
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: system }] });
    await page.eval(theme === 'system' ? `localStorage.removeItem('vink.ui.theme')` : `localStorage.setItem('vink.ui.theme', '${theme}')`);
    await page.goto(`${args.base}/`);
    const selected = theme === 'system' ? `!document.documentElement.hasAttribute('data-theme')` : `document.documentElement.dataset.theme === '${theme}'`;
    check(await page.eval(`document.documentElement.dataset.hydrated !== 'true' && ${selected}`), `${theme}/${system} bootstrap selects the preference without hydration`);
    await assertBrand(page, theme);
    await assertExploreHeader(page, `${theme}/${system} before hydration`);
    await page.capture(`brand-before-hydration-${theme}${theme === 'system' ? '-' + system : ''}`);
  }
  await page.send('Network.setBlockedURLs', { urls: [] });
  for (const deadline = Date.now() + 5000; page.errors.length < errorsBefore + 4 && Date.now() < deadline;) await sleep(50);
  const blocked = page.errors.splice(errorsBefore);
  const onlyBlocked = blocked.length === 4 && blocked.every((e) => e === blockedImport);
  check(onlyBlocked, `the blocked bundle caused only its four rejected hydration imports${onlyBlocked ? '' : `: ${JSON.stringify(blocked)}`}`);
  await page.eval(`localStorage.removeItem('vink.ui.theme')`);
  await page.goto(`${args.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'hydration restored after brand bootstrap check');
  await page.send('Emulation.setEmulatedMedia', {features: [LIGHT_SYSTEM, {name: 'forced-colors', value: 'active'}]});
  check(await page.eval(`(() => {
    const s = getComputedStyle(document.querySelector('.vk-brand__lockup'));
    return s.maskImage.includes('vpen-r1-lockup-mono.svg') && s.backgroundImage === 'none' &&
      s.backgroundColor !== 'rgba(0, 0, 0, 0)';
  })()`), 'forced colors renders the selected mono wordmark as a system-color mask');
  await page.capture('brand-forced-colors');
  await page.send('Emulation.setEmulatedMedia', {features: [LIGHT_SYSTEM]});
}

async function textContrast(page, theme, surface = 'shell') {
  const selectors = surface.startsWith('editor-') ? ['[data-testid=save-status]', '#preview', '#export', '#save', '#draft-title', '#editor-body-label', '#editor-body-hint', '[data-testid=editor-body]', '.vk-editor-help'] : {
    desk: ['#resume h2', '#resume-title-toggle', '.vk-resume .vk-draft-meta', '[data-testid=resume-draft]', '.vk-desk-heading .vk-ui-button', '.vk-draft-row__title', '.vk-draft-row .vk-draft-meta', '.vk-draft-row__open', '.vk-draft-search input', '#draft-sort', '#draft-sort-label', '#draft-scope'],
    explore: ['#public-query', '#explore-heading h1', '.vk-explore__heading p', '#public-query-help', '.vk-public-row h3', '.vk-public-row .vk-ui-byline__name', '.vk-public-row time', '#works-title', '.vk-public-scope', '.vk-discovery-filter-form legend', '.vk-discovery-options .vk-ui-choice__label'],
    reader: ['#reader-return', '#reader-header h1', '#reader-header .vk-work-byline', '#reader-header .vk-ui-type-caption', '.vk-reader .vk-block'],
    me: ['#me-title', '#page-account h2', '#page-account .vk-ui-tone-muted', '#page-appearance-title', '#page-theme-title', '#page-text-title', '#page-rail-title', ...['light', 'dark', 'system'].map(value => `#page-theme label:has([value="${value}"])`)],
    account: ['#profile-title', '#overlay-theme-title', '#overlay-text-title', '#overlay-rail-title', ...['light', 'dark', 'system'].map(value => `#overlay-theme label:has([value="${value}"])`)],
  }[surface] ?? ['#navigation-toggle', '#navigation-primary a', '#profile-trigger', '#profile-title', '#profile-menu .vk-ui-choice__target:has(:checked)', '#profile-menu .vk-ui-button'];
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
    return ${JSON.stringify(selectors)}.flatMap(selector => {
      const el = document.querySelector(selector);
      if (!el) throw new Error('missing contrast target: ' + selector);
      if (el.dataset.state === 'disabled') return { selector, exempt: 'disabled control' };
      const foreground = getComputedStyle(el).color;
      const bg = background(el);
      const [a, b] = [luminance(foreground), luminance(bg)].sort((a, b) => b - a);
      const pair = { selector, foreground, background: bg, ratio: (a + 0.05) / (b + 0.05) };
      if (el.type !== 'search') return [pair];
      const placeholder = getComputedStyle(el, '::placeholder').color;
      const [p, q] = [luminance(placeholder), luminance(bg)].sort((a,b)=>b-a);
      return [pair, { selector: selector+'::placeholder', foreground: placeholder, background: bg, ratio: (p+0.05)/(q+0.05) }];
    });
  })()`);
  writeFileSync(join(dirname(args.state), `${surface}-contrast-${theme}.json`), JSON.stringify(pairs, null, 2));
  check(pairs.every(p => p.exempt || p.ratio >= 4.5), `${theme} ${surface} text contrast >= 4.5 (min ${Math.min(...pairs.filter(p => !p.exempt).map(p => p.ratio)).toFixed(2)}; disabled controls exempt)`);
}

// Account and appearance remain aligned as a unit. The panel has a single scroller.
function accountLayoutGeometry(context, scenario) {
  const m = context.mePanel, near = (a, b) => Math.abs(a - b) <= 1;
  check(Boolean(m), `${scenario}: appearance geometry is present`);
  const fullWidth = m.prefix === 'page' ? [m.heading, m.account, m.identity] : [m.heading, m.choices];
  check(fullWidth.every(r => r && near(r.x, m.surface.contentLeft) && near(r.width, m.surface.contentWidth)),
    `${scenario}: appearance uses the available surface width`);
  if (m.prefix === 'page') {
    check([m.accountLabel, m.accountHelp].every(r => r && r.scrollWidth <= r.clientWidth + 1),
      `${scenario}: truthful account text fits without clipping`);
    if (context.width < 760) {
      check(near(m.surface.x, 16) && near(m.surface.width, context.layoutWidth - 32),
        `${scenario}: narrow Tôi keeps one 16px gutter on both sides`);
    }
  } else {
    check(m.surface.paddingLeft === m.surface.paddingRight && near(m.action.x, m.surface.contentLeft) &&
      m.action.x + m.action.width <= m.surface.contentLeft + m.surface.contentWidth + 1,
      `${scenario}: the panel action shares the content axis`);
  }
}

function appearanceRows(page, prefix) {
  return page.eval(`(() => {
    const scope = document.querySelector(${JSON.stringify(prefix === 'page' ? '#me' : '#profile-menu')});
    const rows = [...scope.querySelectorAll('.vk-appearance-row')];
    const choices = [['Hệ thống', 'Sáng', 'Tối'], ['100%', '200%'], ['Tự động', 'Mở rộng', 'Thu gọn']];
    return rows.length === choices.length && rows.every((row, index) => {
      const heading = row.querySelector('h3'), group = row.querySelector('fieldset');
      const icon = heading.querySelector('[aria-hidden=true]');
      const labels = [...group.querySelectorAll('label')];
      return icon && getComputedStyle(icon).maskImage.startsWith('url(') &&
        heading.textContent === group.querySelector('legend').textContent &&
        labels.map(el => el.textContent).join('|') === choices[index].join('|') &&
        labels.filter(el => el.querySelector(':checked')).length === 1 && labels.every(el => {
          const style = getComputedStyle(el), label = el.querySelector('.vk-ui-choice__label');
          return el.getBoundingClientRect().height >= 48 && style.borderRadius === getComputedStyle(document.documentElement).getPropertyValue('--vk-radius-full').trim() &&
            el.scrollWidth <= el.clientWidth + 1 &&
            getComputedStyle(el, '::before').maskImage.startsWith('url(') &&
            getComputedStyle(el, '::after').opacity === (el.querySelector(':checked') ? '1' : '0') &&
            [...label.textContent.matchAll(/\\S+/g)].every(word => {
              const range = document.createRange();
              range.setStart(label.firstChild, word.index);
              range.setEnd(label.firstChild, word.index + word[0].length);
              return range.getClientRects().length === 1;
            });
        });
    });
  })()`);
}

async function appearanceFocusContrast(page, scenario) {
  await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect?.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
  const contrast = await page.eval(`(() => {
    const input=document.activeElement, face=input.closest('label'), s=getComputedStyle(face);
    const luminance=color=>color.match(/[\\d.]+/g).slice(0,3).map(n=>{const v=Number(n)/255; return v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4;}).reduce((sum,v,i)=>sum+v*[0.2126,0.7152,0.0722][i],0);
    const a=luminance(s.outlineColor), b=luminance(s.backgroundColor);
    return {ratio:(Math.max(a,b)+0.05)/(Math.min(a,b)+0.05), visible:input.checked && input.matches(':focus-visible') && s.outlineStyle==='solid' && parseFloat(s.outlineWidth)>=2};
  })()`);
  check(contrast.visible && contrast.ratio>=3, `${scenario}: selected keyboard perimeter contrasts with the filled face >=3:1 (${contrast.ratio.toFixed(2)})`);
}

async function preferenceSurfaces(page) {
  console.log('R2.2 shared appearance surfaces and native build disclosure:');
  for (const theme of ['light', 'dark']) {
    await page.viewport(390, 844);
    await chooseTheme(page, theme, 'page-theme');
    await page.waitFor(`document.querySelector('#page-account [data-kind=account]')?.dataset.initials === 'TT'`, 'current account name loaded');
    await page.eval(`document.querySelector('#page-theme input:checked').focus()`);
    await page.key('Space');
    await appearanceFocusContrast(page, theme);
    await page.capture(`r22-me-${theme}-selected-focus`);
    check(await appearanceRows(page, 'page'), `${theme}: page uses three compact native groups with icons and selection checks`);
    check(await page.eval(`document.querySelectorAll('#build-info').length === 1 &&
      document.querySelector('#page-account h2').textContent === 'Tài khoản' &&
      document.querySelector('#page-account p').textContent === 'Tác giả mẫu (dữ liệu phát triển)' &&
      document.querySelector('#page-account .vk-account-illustration').getAttribute('aria-hidden') === 'true'`),
      `${theme}: truthful account and one build disclosure`);
    await page.click('#build-info summary');
    check(await page.eval(`document.querySelector('#build-info').open && document.querySelector('#build-info').textContent.includes('Nội dung gắn Mẫu là dữ liệu thử.')`),
      `${theme}: build detail keeps sample meaning reachable`);
    await page.click('#build-info summary');
  }

  await page.send('Network.setBlockedURLs', { urls: [`${args.base}/pkg/vink.js*`, `${args.base}/pkg/vink_bg.wasm*`] });
  await page.send('Emulation.setScriptExecutionDisabled', { value: true });
  const errorsBefore = page.errors.length;
  try {
    await page.goto(`${args.base}/toi`);
    for (const id of ['build-info']) {
      await page.eval(`document.querySelector('#${id} summary').focus()`);
      for (const type of ['keyDown','keyUp']) await page.send('Input.dispatchKeyEvent', {type,key:' ',code:'Space',windowsVirtualKeyCode:32,nativeVirtualKeyCode:32,...(type==='keyDown'?{text:' '}:{})});
      await page.waitFor(`document.querySelector('#${id}').open`, 'native no-JS '+id);
      check(await page.eval(`document.documentElement.dataset.hydrated!=='true' && document.activeElement===document.querySelector('#${id} summary') && document.querySelector('#${id} .vk-ui-supporting-details__content').getBoundingClientRect().height>0`), `${id}: SSR native detail works without page JavaScript`);
    }
  } finally {
    await page.send('Emulation.setScriptExecutionDisabled', { value: false });
    await page.send('Network.setBlockedURLs', { urls: [] });
    const blocked = page.errors.splice(errorsBefore);
    check(blocked.every(error => /Failed to fetch dynamically imported module|Importing a module script failed|ERR_BLOCKED_BY_CLIENT/.test(error)), 'no-JS disclosure check produces only expected blocked imports');
  }
  await page.goto(`${args.base}/toi`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'hydration restored after the native build detail');
}

async function accountAppearance(page) {
  console.log('shared account presentation and bounded appearance overlay:');
  await page.viewport(1440);
  await page.goto(`${args.base}/toi`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'canonical Tôi route for the appearance matrix');
  await page.waitFor(`['#profile-trigger', '#page-account', '#bottom-navigation a[href="/toi"]'].every(selector => {
    const badge = document.querySelector(selector + ' [data-kind=account]');
    return badge?.dataset.initials === 'TT' && !badge.querySelector('[data-icon=person]');
  })`, 'the same current account initials appear on the rail, bottom bar and Tôi');
  check(await page.eval(`fetch('/api/v1/auth/session/account').then(async response => {
    const body = await response.json();
    return response.status === 200 && response.headers.get('cache-control') === 'no-store' &&
      body.display_name === document.querySelector('#page-account p').textContent &&
      Object.keys(body).sort().join(',') === 'display_name,principal_id';
  })`), 'current account is read from the private session endpoint');
  await page.eval(`document.querySelector('#profile-trigger').focus()`);
  await page.key('Enter');
  await page.waitFor(`${PROFILE_OPEN} && document.activeElement?.name === 'overlay-theme' && !document.querySelector('#open-me')`, 'Tôi opens the shared modal on /toi without a self-link');
  await page.click('#overlay-text label:has([value="200"])');
  await page.click('#overlay-rail label:has([value="compact"])');
  await page.waitFor(`document.querySelector('#page-text input:checked')?.value === '200' &&
    document.querySelector('#page-rail input:checked')?.value === 'compact' &&
    getComputedStyle(document.documentElement).fontSize === '32px' &&
    ${RAIL_WIDTH} === 96`, 'panel updates page radios, text size and rail immediately');
  await page.key('Escape');
  check(await page.eval(`document.activeElement?.id === 'profile-trigger'`), 'Escape returns to the trigger on /toi');
  await page.reload();
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' &&
    document.querySelector('#page-text input:checked')?.value === '200' &&
    document.querySelector('#page-rail input:checked')?.value === 'compact'`, 'text and rail choices survive reload');
  await page.click('#page-text label:has([value="100"])');
  await page.click('#page-rail label:has([value="auto"])');
  await page.waitFor(`!document.documentElement.dataset.text && !document.documentElement.dataset.rail &&
    localStorage.getItem('vink.ui.text') === null && localStorage.getItem('vink.ui.rail') === null &&
    ${RAIL_WIDTH} === 220 && document.querySelector('#overlay-rail input:checked')?.value === 'auto'`, '100% and automatic clear overrides and sync the panel');
  await chooseTheme(page, 'system', 'page-theme');
  await page.eval(`document.querySelector('#page-theme input:checked').focus()`);
  await page.key('ArrowRight');
  await page.waitFor(`document.querySelector('#page-theme input:checked')?.value === 'light' && document.documentElement.dataset.theme === 'light'`, 'native Arrow updates page preference');
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'appearance utility host');
  await openProfile(page);
  check(await page.eval(`document.querySelector('#profile-title').textContent === 'Giao diện' &&
    document.querySelector('#profile-title [aria-hidden=true] [data-appearance-icon=palette]') &&
    document.querySelector('#profile-trigger').getAttribute('aria-haspopup') === 'dialog' &&
    document.querySelector('#profile-trigger').getAttribute('aria-controls') === 'profile-menu' &&
    document.querySelector('#navigation-primary a[href="/ban-viet"]').getAttribute('aria-current') === 'page' &&
    !document.querySelector('#profile-menu .vk-account-status') && document.querySelector('#open-me')`), 'compact appearance dialog identifies its purpose and preserves the active route');
  check(await appearanceRows(page, 'overlay'), 'panel uses the same three native groups, glyphs and selection checks');
  await chooseTheme(page, 'system');
  await page.eval(`document.querySelector('#overlay-theme input:checked').focus()`);
  await page.key('ArrowRight');
  await page.waitFor(`document.querySelector('#overlay-theme input:checked')?.value === 'light' && document.documentElement.dataset.theme === 'light'`, 'native Arrow updates utility preference');
  check(await page.eval(`(() => { const input = document.activeElement, face = input.closest('label'), style = getComputedStyle(face); return input.checked && input.matches(':focus-visible') && style.outlineStyle === 'solid' && parseFloat(style.outlineWidth) === 2 && style.outlineOffset === '-2px' && style.boxShadow === 'none' && getComputedStyle(face, '::after').opacity === '1'; })()`), 'native keyboard focus paints one flush 2px perimeter independently of the selected check');
  await page.key('ArrowLeft');
  await page.waitFor(`${PROFILE_OPEN} && document.querySelector('#overlay-theme input:checked')?.value === 'system'`, 'native ArrowLeft keeps panel open');
  await page.eval(`document.querySelector('#overlay-theme input[value=dark]').focus()`);
  await page.key('Space');
  await page.waitFor(`${PROFILE_OPEN} && document.querySelector('#overlay-theme input:checked')?.value === 'dark'`, 'native Space selects the focused choice');
  await page.eval(`(() => { for (const value of ['light', 'system', 'dark']) document.querySelector('#overlay-theme input[value="' + value + '"]').click(); })()`);
  await page.waitFor(`${PROFILE_OPEN} && document.querySelector('#overlay-theme input:checked')?.value === 'dark' && document.documentElement.dataset.theme === 'dark'`, 'rapid theme intents settle on the final preference');
  check(await page.eval(`(() => { const close = document.querySelector('#profile-close'), r = close.getBoundingClientRect(), dialog = document.querySelector('#profile-menu'); return r.width >= 48 && r.height >= 48 && close.getAttribute('aria-label') === 'Đóng Giao diện' && close.querySelector('[data-icon=close]') && getComputedStyle(dialog).backgroundColor.startsWith('rgb(') && getComputedStyle(dialog, '::backdrop').backgroundColor !== 'rgba(0, 0, 0, 0)' && [...dialog.querySelectorAll('*')].every(el => !['auto','scroll'].includes(getComputedStyle(el).overflowY)); })()`), 'opaque panel, visible scrim, one scroller and named close icon target >=48px');
  await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM, { name: 'forced-colors', value: 'active' }, { name: 'prefers-reduced-motion', value: 'reduce' }] });
  await page.eval(`document.querySelector('#overlay-theme input:checked').focus()`);
  await page.key('ArrowRight');
  check(await page.eval(`(() => { const input = document.activeElement, face = input.closest('label'), style = getComputedStyle(face); return input.checked && input.matches(':focus-visible') && style.forcedColorAdjust === 'none' && style.outlineStyle === 'solid' && style.outlineWidth === '2px' && parseFloat(style.transitionDuration) <= 0.001 && getComputedStyle(face, '::after').opacity === '1' && [...document.querySelectorAll('#overlay-theme label')].every(el => getComputedStyle(el, '::before').forcedColorAdjust === 'none'); })()`), 'forced-colors retains selected check and keyboard focus; reduced-motion reduces transitions below 1ms');
  await appearanceFocusContrast(page, 'forced-colors');
  await page.capture('appearance-forced-colors-reduced-motion');
  await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM] });
  await page.key('Escape');

  for (const { width, rail, height } of [
    { width: 1440, rail: 'expanded', height: 900 },
    { width: 1440, rail: 'compact', height: 900 },
    { width: 1024, rail: 'compact', height: 900 },
    { width: 834, rail: 'compact', height: 900 },
    { width: 834, rail: 'expanded', height: 900 },
    { width: 1440, rail: 'expanded', height: 360 },
    { width: 1440, rail: 'compact', height: 360 },
    { width: 834, rail: 'expanded', height: 360 },
    { width: 834, rail: 'compact', height: 360 },
  ]) {
    await page.viewport(width, height);
    if ((await page.eval(RAIL_WIDTH)) !== (rail === 'expanded' ? 220 : 96)) await page.click('#navigation-toggle');
    for (const theme of ['system', 'light', 'dark']) {
      for (const scale of [100, 200]) {
        await page.eval(`document.documentElement.style.fontSize = '${scale}%'`);
        await openProfile(page);
        await chooseTheme(page, theme);
        const scenario = `${width}-${height}-${rail}-${theme}-text${scale}`;
        check(await page.eval(`(() => { const rail = document.querySelector('.vk-ui-rail'); return rail.scrollWidth <= rail.clientWidth; })()`), `${scenario}: the rail needs no horizontal scrolling`);
        check(await page.eval(`(() => {
          const dialog = document.querySelector('#profile-menu'), r = dialog.getBoundingClientRect();
          return dialog.matches(':modal') && r.left >= document.querySelector('.vk-ui-rail').getBoundingClientRect().right &&
            r.right <= innerWidth && r.top >= 0 && r.bottom <= innerHeight &&
            dialog.scrollWidth <= dialog.clientWidth && document.documentElement.scrollWidth <= innerWidth &&
            document.querySelector('#profile-trigger').getAttribute('aria-expanded') === 'true';
        })()`), `${scenario}: modal stays beside the rail, within the viewport, without internal horizontal overflow`);
        if (height === 900 && width === 1440 && scale === 100 && theme !== 'system') await textContrast(page, theme, 'account');
        await page.eval(`document.querySelector('#profile-close').focus()`);
        await page.key('Tab', SHIFT);
        check(await page.eval(`(() => {
          const el = document.activeElement, r = el.getBoundingClientRect(), d = document.querySelector('#profile-menu').getBoundingClientRect();
          return el.id === 'open-me' && r.top >= d.top && r.bottom <= d.bottom && r.height >= 44;
        })()`), `${scenario}: Shift+Tab reaches and reveals the final action`);
        if (await page.eval(`document.querySelector('#profile-menu').scrollHeight > document.querySelector('#profile-menu').clientHeight`)) await page.capture(`account-overlay-${scenario}-bottom`);
        await page.key('Tab');
        check(await page.eval(`(() => {
          const el = document.activeElement, r = el.getBoundingClientRect(), d = document.querySelector('#profile-menu').getBoundingClientRect();
          return el.id === 'profile-close' && r.top >= d.top && r.bottom <= d.bottom;
        })()`), `${scenario}: Tab reveals the close action at the start`);
        await page.eval(`document.querySelector('#profile-menu').scrollTop = 0`);
        check(await appearanceRows(page, 'overlay'), `${scenario}: compact panel groups keep whole labels and touch targets`);
        accountLayoutGeometry(await page.capture(`account-overlay-${scenario}`), scenario);
        await page.key('Escape');
        check(await page.eval(`!${PROFILE_OPEN} && document.activeElement?.id === 'profile-trigger'`), `${scenario}: Escape restores the account utility focus`);
      }
    }
  }
  await page.eval(`document.documentElement.style.fontSize = ''; localStorage.removeItem('vink.ui.rail')`);
  await page.viewport(1440);
  await page.goto(`${args.base}/toi`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'canonical Me restored');
}

// Fragment history still belongs to the shell overlay; with no overlay it remains an
// ordinary in-page traversal and must not ask the dirty editor's leave guard.
async function shellFragmentHistory(page) {
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'fragment history hydration');
  await page.eval(`location.hash = 'shell-fragment'`);
  await page.waitFor(`location.hash === '#shell-fragment'`, 'fragment history entry');
  await sleep(100);
  await openProfile(page);
  await page.capture('1440-fragment-overlay-before-back');
  await page.eval(`history.back()`);
  await page.waitFor(`!${PROFILE_OPEN}`, 'fragment Back closes Tôi');
  await sleep(100);
  check(await page.eval(`location.pathname === '/ban-viet' && location.hash === '#shell-fragment' && document.activeElement?.id === 'profile-trigger'`), 'fragment Back closes the overlay, retains the current URL and restores trigger focus');
  await page.capture('1440-fragment-overlay-after-back');
  await page.eval(`history.back()`);
  await page.waitFor(`location.hash === ''`, 'next Back traverses the fragment');
  await page.eval(`history.forward()`);
  await page.waitFor(`location.hash === '#shell-fragment'`, 'Forward traverses the fragment without an overlay');
  check(await page.eval(`location.pathname === '/ban-viet' && !${PROFILE_OPEN}`), 'plain fragment Back/Forward leaves the route and overlay unchanged');
  await page.eval(`history.back()`);
  await page.waitFor(`location.hash === ''`, 'prepare fragment Forward with overlay');
  await openProfile(page);
  await page.eval(`history.forward()`);
  await page.waitFor(`!${PROFILE_OPEN}`, 'fragment Forward closes Tôi');
  await sleep(100);
  check(await page.eval(`location.pathname === '/ban-viet' && location.hash === '' && document.activeElement?.id === 'profile-trigger'`), 'fragment Forward also closes the overlay before changing the current URL');

  await page.viewport(1440, 360);
  await page.eval(`document.documentElement.style.fontSize = '200%'; location.hash = 'shell-scroll'`);
  await sleep(100);
  await page.eval(`scrollTo(0, 500)`);
  const scroll = await page.eval(`scrollY`);
  check(scroll > 0, 'fragment scroll probe starts below the top of the page');
  await openProfile(page);
  await page.eval(`history.back()`);
  await page.waitFor(`!${PROFILE_OPEN}`, 'scrolled fragment Back closes Tôi');
  await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
  check(await page.eval(`scrollY === ${scroll} && location.hash === '#shell-scroll' && document.activeElement?.id === 'profile-trigger'`), 'held fragment Back preserves nonzero page scroll after native history restoration');
  await page.capture('1440x360-fragment-scroll-text200');
  await page.eval(`document.documentElement.style.fontSize = ''; scrollTo(0, 0)`);
  await page.viewport(1440);
}

async function railReview(page) {
  const cases = [
    [1440, 900, 100, 'light', 220], [1440, 900, 100, 'light', 96],
    [1440, 900, 100, 'dark', 220], [1440, 900, 100, 'dark', 96],
    [1024, 900, 200, 'light', 96], [1024, 900, 200, 'light', 220],
    [834, 360, 100, 'light', 96], [834, 360, 200, 'light', 96],
  ];
  for (const [width, height, scale, theme, railWidth] of cases) {
    await page.viewport(width, height);
    await page.eval(`document.documentElement.dataset.theme = '${theme}'; document.documentElement.style.fontSize = '${scale}%'`);
    if ((await page.eval(RAIL_WIDTH)) !== railWidth) await page.click('#navigation-toggle');
    await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: width - 10, y: 10 });
    await page.eval(`document.activeElement?.blur(); document.querySelector('#navigation').scrollTop = 0`);
    const name = `rail-r1-${width}x${height}-${railWidth}-${theme}-text${scale}`;
    await assertExploreHeader(page, name);
    await page.capture(name);
    check(await page.eval(`(() => {
      const rail = document.querySelector('#navigation'), toggle = document.querySelector('#navigation-toggle');
      const centerX = rect => rect.left + rect.width / 2;
      const r = toggle.getBoundingClientRect(), glyph = toggle.querySelector('svg').getBoundingClientRect();
      const navIcons = [...rail.querySelectorAll('.vk-ui-nav-item .vk-ui-icon')];
      const aligned = navIcons.length === 3 && navIcons.every(icon =>
        Math.abs(centerX(glyph) - centerX(icon.getBoundingClientRect())) <= 1);
      const wordsFit = [...rail.querySelectorAll('.vk-ui-nav-item__label')].every(label =>
        [...label.textContent.matchAll(/\\S+/g)].every(word => {
          const range = document.createRange();
          range.setStart(label.firstChild, word.index);
          range.setEnd(label.firstChild, word.index + word[0].length);
          return range.getClientRects().length === 1;
        }));
      return location.pathname === '/' && rail.firstElementChild === toggle &&
        rail.getBoundingClientRect().width === ${railWidth} && r.width >= 48 && r.height >= 48 &&
        aligned && glyph.width < navIcons[0].getBoundingClientRect().width && rail.scrollWidth <= rail.clientWidth &&
        document.documentElement.scrollWidth <= innerWidth && wordsFit &&
        document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.closest('button') === toggle;
    })()`), `${name}: toggle is first, icon axes align, target is reachable and words fit`);
    if (height === 360) {
      await page.eval(`document.querySelector('.vk-ui-skip-link').focus()`);
      await page.key('Tab');
      check(await page.eval(`(() => {
        const tip = document.querySelector('#navigation-tooltip');
        const r = tip.getBoundingClientRect();
        return document.activeElement?.id === 'navigation-toggle' && !tip.hidden &&
          r.left >= document.querySelector('#navigation').getBoundingClientRect().right &&
          r.top >= 0 && r.bottom <= innerHeight;
      })()`), `${name}: the focus tooltip clears the short rail`);
      await page.capture(`${name}-tooltip`);
      await page.key('Escape');
      for (let i = 0; i < 3; i++) await page.key('Tab');
      check(await page.eval(`(() => {
        const r = document.querySelector('#profile-trigger').getBoundingClientRect();
        return document.activeElement?.id === 'profile-trigger' && r.top >= 0 && r.bottom <= innerHeight;
      })()`), `${name}: Tab reaches and reveals Tôi at the foot`);
      await page.capture(`${name}-account`);
    }
  }
  await page.viewport(1440);
  await page.eval(`localStorage.removeItem('vink.ui.rail')`);
  await page.reload();
}

async function adaptiveShell(page) {
  console.log('adaptive navigation and appearance:');
  await page.viewport(1440);
  await page.goto(`${args.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'shell hydration');
  await brandAssets(page);
  await railReview(page);
  check(await page.eval(`location.pathname === '/' && [...document.querySelectorAll('#navigation-primary a')].map(a => a.textContent).join('|') === 'Khám phá|Bàn viết'`), 'launch route stays /, Khám phá precedes Bàn viết');
  check((await page.eval(RAIL_WIDTH)) === 220, 'desktop defaults to a 220px rail');
  check(await page.eval(`(() => { const b = document.querySelector('#navigation-toggle'); return b.getAttribute('aria-expanded') === 'true' && b.getAttribute('aria-label') === 'Thu gọn điều hướng' && b.getAttribute('aria-controls') === 'navigation'; })()`), 'expanded rail toggle names the collapse it performs');
  check(await page.eval(`document.querySelectorAll('[role=tablist], [role=tab]').length === 0 && [...document.querySelectorAll('#navigation-primary a, .vk-ui-bottom-nav a')].every(a => a.getAttribute('href')?.startsWith('/'))`), 'route destinations are real links, not tabs');
  await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM,
    { name: 'forced-colors', value: 'active' }, { name: 'prefers-reduced-motion', value: 'reduce' },
  ] });
  await page.eval(`window.__railToggle = document.querySelector('#navigation-toggle'); document.querySelector('.vk-ui-skip-link').focus()`);
  await page.key('Tab');
  const forcedFocus = await page.eval(`(() => {
    const style = getComputedStyle(window.__railToggle);
    return { focused: document.activeElement === window.__railToggle,
      visible: window.__railToggle.matches(':focus-visible'), outline: style.outlineStyle,
      width: parseFloat(style.outlineWidth),
      forcedColors: matchMedia('(forced-colors: active)').matches,
      reducedMotion: matchMedia('(prefers-reduced-motion: reduce)').matches,
      transition: getComputedStyle(document.querySelector('#navigation-primary a')).transitionDuration };
  })()`);
  check(forcedFocus.focused && forcedFocus.visible && forcedFocus.outline === 'solid' && forcedFocus.width >= 2 &&
    forcedFocus.forcedColors && forcedFocus.reducedMotion && parseFloat(forcedFocus.transition) <= 0.001,
  `Tab reaches a visible toggle focus ring in forced colors and reduced motion: ${JSON.stringify(forcedFocus)}`);
  await page.capture('rail-r1-1440-forced-colors-focus');
  await page.eval(`window.__exploreRailProbe = {
    brand: document.querySelector('.vk-brand'), input: document.querySelector('#public-query')
  }; window.__exploreRailProbe.right = window.__exploreRailProbe.brand.getBoundingClientRect().right;
  window.__exploreRailProbe.input.focus()`);
  await page.send('Input.insertText', { text: 'Mưa trên phố' });
  await page.eval(`document.querySelector('#navigation-toggle').focus()`);
  await page.key('Enter');
  check((await page.eval(RAIL_WIDTH)) === 96 && await page.eval(`document.activeElement === window.__railToggle`), 'Enter collapses once and retains the toggle');
  check(await page.eval(`(() => {
    const { brand, input, right } = window.__exploreRailProbe;
    return brand === document.querySelector('.vk-brand') && brand.getBoundingClientRect().right === right &&
      input === document.querySelector('#public-query') && input.value === 'Mưa trên phố' && !location.search;
  })()`), 'collapsing the rail keeps the brand aligned and retains the unsubmitted search');
  await page.key('Space');
  check((await page.eval(RAIL_WIDTH)) === 220 && await page.eval(`document.activeElement === window.__railToggle`), 'Space expands once and retains the toggle');
  await page.click('#clear-public-search');
  await page.waitFor(`document.querySelector('#public-query').value === ''`, 'clear rail probe text');
  await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM] });
  await page.click('#navigation-toggle');
  check((await page.eval(RAIL_WIDTH)) === 96, 'manual compact rail is 96px');
  check(await page.eval(`(() => { const b = document.querySelector('#navigation-toggle'); const r = b.getBoundingClientRect(); const rail = document.querySelector('.vk-ui-rail').getBoundingClientRect(); return location.pathname === '/' && b.getAttribute('aria-expanded') === 'false' && b.getAttribute('aria-label') === 'Mở rộng điều hướng' && r.width >= 44 && r.height >= 44 && r.left >= rail.left && r.right <= rail.right && document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.closest('#navigation-toggle'); })()`), 'compact rail keeps a visible, unobstructed >=44px expand control on the same page');
  check(await page.eval(`getComputedStyle(document.querySelector('#navigation-primary a')).flexDirection === 'column'`), 'compact rail places icons above labels');
  await page.key('Escape');
  check(await page.eval(`document.querySelector('[role=tooltip]').hidden`), 'Escape dismisses the rail tooltip');
  await page.eval(`document.querySelector('#navigation-toggle').blur()`);
  await page.eval(`document.querySelector('#navigation-toggle').focus()`);
  check(await page.eval(`!document.querySelector('[role=tooltip]').hidden`), 'focus exposes the rail tooltip');
  await page.key('Escape');
  await page.eval(`document.querySelector('#navigation-toggle').blur()`);
  await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1000, y: 20 });
  const hover = await page.eval(`(() => { const r = document.querySelector('#navigation-toggle').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...hover });
  await page.waitFor(`!document.querySelector('[role=tooltip]').hidden`, 'hover tooltip');
  check(await page.eval(`(() => { const r = document.querySelector('[role=tooltip]').getBoundingClientRect(); return r.left >= document.querySelector('.vk-ui-rail').getBoundingClientRect().right && r.right <= innerWidth; })()`), 'hover tooltip is outside the scrolling rail and within the viewport');
  await page.capture('1440-rail-tooltip-light');
  await page.key('Escape');
  check(await page.eval(`document.querySelector('[role=tooltip]').hidden`), 'Escape also dismisses a hover tooltip without trigger focus');
  await page.reload();
  check((await page.eval(RAIL_WIDTH)) === 96, 'rail preference survives reload');
  await page.viewport(1024);
  check((await page.eval(RAIL_WIDTH)) === 96, 'manual compact preference survives resize');
  await page.click('#navigation-toggle');
  check((await page.eval(RAIL_WIDTH)) === 220, 'manual expanded preference overrides tablet default');
  await page.eval(`localStorage.removeItem('vink.ui.rail')`);
  await page.reload();
  check((await page.eval(RAIL_WIDTH)) === 96, 'tablet defaults to a compact rail');
  await page.viewport(1440);
  await page.waitFor(`${RAIL_WIDTH} === 220`, 'desktop contextual default');
  await page.eval(`document.querySelector('.vk-ui-skip-link').focus()`);
  await page.key('Enter');
  check(await page.eval(`location.pathname === '/' && document.activeElement?.id === 'noi-dung'`), 'skip link moves focus to main without a route change');
  check(await page.eval(`(() => {
    let defaultPrevented = null;
    const record = (event) => { defaultPrevented = event.defaultPrevented; event.preventDefault(); };
    addEventListener('click', record, { once: true });
    document.querySelector('#navigation-primary a[href="/ban-viet"]').dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, ctrlKey: true, metaKey: true, button: 0 }));
    return defaultPrevented === false && location.pathname === '/';
  })()`), 'modifier click on a destination is left to the browser (new tab/window), not routed');
  for (const path of ['/ban-vietnam', '/tac-pham/a/b', '/toi/khac']) {
    const response = await fetch(`${args.base}${path}`);
    const missingHtml = await response.text();
    check(response.status === 404 && missingHtml.includes('Không tìm thấy trang này.'), `${path} is a 404 page on the server`);
    check(!missingHtml.includes('class="vk-brand"'), `${path} omits the brand before hydration`);
    await page.goto(`${args.base}${path}`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('h1')?.textContent === 'Không tìm thấy trang này.'`, `${path} NotFound`);
    check(await page.eval(`document.querySelectorAll('[aria-current=page]').length === 0`), `${path} marks no destination active`);
    await assertBrandPlacement(page);
  }
  await page.goto(`${args.base}/`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'shell hydration after unknown routes');
  await page.eval(`(() => { for (const href of ['/ban-viet', '/', '/ban-viet']) document.querySelector('#navigation-primary a[href="' + href + '"]').click(); })()`);
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('#create-draft')`, 'rapid navigations settle on the last intent');
  check(await page.eval(`document.querySelector('#navigation-primary a[href="/ban-viet"]').getAttribute('aria-current') === 'page' && [...document.querySelectorAll('[aria-current=page]')].filter(el => el.getClientRects().length).length === 1`), 'rapid navigations leave exactly the final destination active');
  await assertBrandPlacement(page);
  await page.click('#navigation-primary a[href="/"]');
  await page.waitFor(`location.pathname === '/' && document.querySelector('#discovery-tools .vk-brand')`, 'back to Khám phá with its brand');
  await assertBrandPlacement(page);

  await openProfile(page);
  check(await page.eval(`document.activeElement?.name === 'overlay-theme' && document.activeElement.value === 'system'`), 'overlay initially focuses the selected appearance option');
  check(await page.eval(`document.querySelector('#profile-title').textContent === 'Giao diện' && !document.querySelector('#profile-menu .vk-account-status')`), 'appearance utility contains preferences');
  await page.eval(`document.querySelector('#profile-close').focus()`);
  await page.key('Tab', SHIFT);
  check(await page.eval(`document.activeElement?.id === 'open-me'`), 'Shift+Tab is contained at the beginning of the modal');
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.id === 'profile-close'`), 'Tab is contained at the end of the modal');
  await chooseTheme(page, 'dark');
  await textContrast(page, 'dark');
  await page.capture('1440-overlay-dark');
  await page.key('Escape');
  check(await page.eval(`!${PROFILE_OPEN} && document.activeElement?.id === 'profile-trigger'`), 'Escape closes Tôi and restores trigger focus');
  await page.reload();
  check(await page.eval(`document.documentElement.dataset.theme === 'dark'`), 'theme preference survives reload');
  await assertBrand(page, 'dark');
  await openProfile(page);
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: 1400, y: 20, button: 'left', clickCount: 1 });
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 1400, y: 20, button: 'left', clickCount: 1 });
  check(await page.eval(`!${PROFILE_OPEN} && document.activeElement?.id === 'profile-trigger'`), 'outside click closes Tôi and restores focus');
  await openProfile(page);
  await page.click('#profile-close');
  check(await page.eval(`!${PROFILE_OPEN}`), 'explicit close works');
  await page.eval(`(() => {
    // Close and reopen in the same task, before the native queued close event is dispatched.
    document.querySelector('#profile-trigger').click();
    document.querySelector('#profile-close').click();
    document.querySelector('#profile-trigger').click();
  })()`);
  await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
  check(await page.eval(`${PROFILE_OPEN} && document.querySelector('#profile-trigger').getAttribute('aria-expanded') === 'true'`), 'stale close events leave the reopened overlay and its expanded state intact');
  await page.key('Escape');
  check(await page.eval(`!${PROFILE_OPEN} && document.activeElement?.id === 'profile-trigger' && document.querySelector('#profile-trigger').getAttribute('aria-expanded') === 'false'`), 'reopened overlay still dismisses and restores focus');
  await openProfile(page);
  await page.click('#open-me');
  await page.waitFor(`location.pathname === '/toi' && document.querySelector('#me-title')`, 'real Tôi destination');
  check(await page.eval(`document.querySelector('#profile-trigger').getAttribute('aria-current') === 'page'`), 'Tôi route marks the rail utility active');
  await page.eval(`document.querySelector('#profile-trigger').focus()`);
  await page.key('Enter');
  check(await page.eval(`${PROFILE_OPEN} && document.activeElement?.name === 'overlay-theme' && !document.querySelector('#open-me')`), '/toi can open the appearance panel without a self-link');
  await page.key('Escape');
  await page.click('#navigation-primary a[href="/"]');
  await page.waitFor(`location.pathname === '/'`, 'overlay history host');
  await openProfile(page);
  await page.eval(`history.back()`);
  await page.waitFor(`!${PROFILE_OPEN}`, 'Back closes utility');
  await sleep(200);
  check(await page.eval(`location.pathname === '/' && document.activeElement?.id === 'profile-trigger'`), 'Back closes panel before leaving its host route and restores focus');
  await page.eval(`history.back()`);
  await page.waitFor(`location.pathname === '/toi'`, 'next Back leaves host');
  await page.eval(`history.forward()`);
  await page.waitFor(`location.pathname === '/'`, 'Forward returns to host');
  await shellFragmentHistory(page);
  await openProfile(page);
  await page.viewport(390);
  await page.waitFor(`!${PROFILE_OPEN}`, 'resize closes overlay');
  check(await page.eval(`location.pathname === '/ban-viet' && document.activeElement?.id === 'noi-dung'`), 'resize retains host route and focuses visible main');
  check(await page.eval(`[...document.querySelectorAll('.vk-ui-bottom-nav a')].map(a => a.querySelector('.vk-ui-nav-item__label').textContent).join('|') === 'Khám phá|Bàn viết|Tôi'`), 'mobile destination order is Khám phá, Bàn viết, Tôi');
  await page.click('.vk-ui-bottom-nav a[href="/toi"]');
  await page.waitFor(`location.pathname === '/toi' && document.querySelector('#page-theme')`, 'compact Tôi goes to canonical route');

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
  await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM] });
  await chooseTheme(page, 'system', 'page-theme');

  // Bootstrap and interactive writes must both tolerate browser privacy/storage denial.
  const { identifier } = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `for (const method of ['getItem','setItem','removeItem']) Storage.prototype[method] = () => { throw new DOMException('Storage denied', 'SecurityError'); };` });
  await page.reload();
  await chooseTheme(page, 'dark', 'page-theme');
  await page.viewport(1440);
  await page.click('#navigation-toggle');
  check(await page.eval(`document.documentElement.dataset.theme === 'dark' && document.documentElement.dataset.rail === 'compact'`), 'storage-denied theme and rail still work in memory');
  await assertBrand(page, 'dark');
  // Client navigation retains the session preference even while storage is unavailable.
  await page.click('#navigation-primary a[href="/"]');
  await page.waitFor(`location.pathname === '/'`, 'storage-denied panel host');
  await openProfile(page);
  check(await page.eval(`document.activeElement?.value === 'dark'`), 'storage denial keeps utility on the session-local preference');
  await page.capture('appearance-storage-denied-overlay-dark');
  await chooseTheme(page, 'system');
  await page.click('#open-me');
  await page.waitFor(`location.pathname === '/toi' && document.querySelector('#page-theme input:checked')?.value === 'system'`, 'storage-denied System survives client navigation');
  await page.capture('appearance-storage-denied-page-system');
  await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier });
  await page.reload();
  await page.eval(`localStorage.removeItem('vink.ui.rail'); localStorage.removeItem('vink.ui.theme')`);
  await page.reload();

  await accountAppearance(page);
  await preferenceSurfaces(page);

  for (const width of [1440, 1280, 1024, 834, 390, 320]) {
    await page.viewport(width, width === 390 ? 844 : 900);
    for (const theme of ['system', 'light', 'dark']) {
      await chooseTheme(page, theme, 'page-theme');
      if (width === 1440 && theme !== 'system') await textContrast(page, theme, 'me');
      for (const scale of [100, 200]) {
        await page.eval(`document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
        await bottomNavigationAlignment(page, `shell ${width}px/${scale}%/${theme}`);
        // Layout can update before glyphs repaint after a root-text change. The
        // generic capture helper also serves actual no-JS pages, so wait only here.
        await page.eval(`new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))`);
        accountLayoutGeometry(await page.capture(`${width}-me-${theme}-text${scale}`), `${width}px/${scale}% ${theme}`);
        check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), `${width}px/${scale}% ${theme}: no horizontal overflow`);
        check(await appearanceRows(page, 'page'), `${width}px/${scale}% ${theme}: appearance choices keep whole labels and touch targets`);
        if (width < 760 && scale === 200) {
          await page.eval(`document.querySelector('#page-theme-title').scrollIntoView({ block: 'start' })`);
          await page.capture(`${width}-me-${theme}-text${scale}-appearance`);
          await page.eval(`scrollTo(0, document.documentElement.scrollHeight)`);
          check(await page.eval(`(() => {
            const footer = document.querySelector('.vk-footer').getBoundingClientRect();
            const bar = document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect();
            return footer.bottom <= bar.top + 1;
          })()`), `${width}px/${scale}% ${theme}: final disclosure remains reachable above navigation`);
          await page.capture(`${width}-me-${theme}-text${scale}-end`);
          await page.eval(`scrollTo(0, 0)`);
        }
      }
      await page.eval(`document.documentElement.style.fontSize = ''`);
    }
  }
  await page.viewport(1024);
  await page.eval(`document.documentElement.style.fontSize = '200%'`);
  const compactWords = await page.eval(`(() => {
    const labels = [...document.querySelectorAll('.vk-ui-rail .vk-ui-nav-item__label')].filter((el) => el.textContent === 'Khám phá');
    return { count: labels.length, intact: labels.every((el) => [[0,4],[5,8]].every(([a,b]) => {
      const range = document.createRange(); range.setStart(el.firstChild,a); range.setEnd(el.firstChild,b);
      return range.getClientRects().length === 1;
    })) };
  })()`);
  check(compactWords.count > 0 && compactWords.intact, '1024px / 200% text keeps compact rail Vietnamese words intact');
  await page.capture('1024-compact-words-text200');
  await page.viewport(320);
  await page.capture('320-me-dark-text200');
  check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), '320px / 200% text reflows without horizontal overflow');
  await page.eval(`scrollTo(0, document.documentElement.scrollHeight)`);
  await page.waitFor(`parseFloat(getComputedStyle(document.querySelector('.vk-ui-scaffold')).getPropertyValue('--vk-ui-bar-height')) === document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().height`, 'measured bottom clearance');
  check(await page.eval(`document.querySelector('.vk-footer').getBoundingClientRect().bottom <= document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().top`), 'bottom bar clears the end of page content at 320px / 200% text');
  await page.capture('320-me-dark-text200-bottom');
  await page.click('.vk-ui-bottom-nav a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('#create-draft')`, 'mobile desk destination');
  await page.capture('320-desk-dark-text200');
  await page.click('.vk-ui-bottom-nav a[href="/"]');
  await page.waitFor(`location.pathname === '/' && document.querySelector('#discovery-tools .vk-brand')`, 'mobile discovery destination');
  await assertBrandPlacement(page);
  await page.eval(`document.querySelector('.vk-ui-skip-link').focus()`);
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('.vk-brand:focus-visible') && getComputedStyle(document.activeElement).outlineStyle === 'solid'`), 'at 320px / 200% text, Tab reaches the Explore brand with a visible focus ring');
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('#public-search button[type=submit]:focus-visible')`), 'the next Tab reaches the search action below the brand');
  await page.click('.vk-ui-bottom-nav a[href="/toi"]');
  await page.waitFor(`location.pathname === '/toi'`, 'mobile Tôi screen');
  await chooseTheme(page, 'system', 'page-theme');
  await page.eval(`document.documentElement.style.fontSize = ''; scrollTo(0, 0)`);
  await page.viewport(1280);
  await appearanceFirstPaint(page);
}

/** Keep first-paint selection evidence separate from hydrated interaction evidence. */
async function appearanceFirstPaint(page) {
  console.log('appearance preference before hydration:');
  const errorsBefore = page.errors.length;
  const mismatches = [];
  let navigations = 0;
  const { identifier } = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `window.__appearancePaints = [];
    const sample = () => {
      const choice = document.querySelector('#page-theme input:checked');
      if (choice) { const signature = (document.documentElement.dataset.theme || 'system') + ':' + choice.value; if (!window.__appearancePaints.includes(signature)) window.__appearancePaints.push(signature); }
      if (document.documentElement.dataset.hydrated !== 'true') requestAnimationFrame(sample);
    }; requestAnimationFrame(sample);` });
  for (const [theme, os] of [['light', 'dark'], ['dark', 'light'], ['system', 'light'], ['system', 'dark']]) {
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: os }] });
    await page.eval(theme === 'system' ? `localStorage.removeItem('vink.ui.theme')` : `localStorage.setItem('vink.ui.theme', '${theme}')`);
    for (const width of [1440, 320]) {
      await page.viewport(width);
      await page.send('Network.setBlockedURLs', { urls: [`${args.base}/pkg/vink.js*`, `${args.base}/pkg/vink_bg.wasm*`] });
      await page.goto(`${args.base}/toi`);
      navigations++;
      for (const scale of [100, 200]) {
        await page.eval(`document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
        const name = `appearance-first-paint-${theme}-os-${os}-${width}-text${scale}`;
        await page.capture(name, { scenario: 'UI-REVIEW-21-first-paint', fixture: 'local-fixture-unknown-account' });
        await page.eval(`document.querySelector('#page-theme').scrollIntoView({ block: 'end' })`);
        await page.capture(`${name}-appearance`, { scenario: 'UI-REVIEW-21-first-paint', fixture: 'local-fixture-unknown-account' });
        const state = await page.eval(`({ hydrated: document.documentElement.dataset.hydrated === 'true', theme: document.documentElement.dataset.theme || 'system', choices: ['page-theme', 'overlay-theme'].map(id => document.querySelector('#' + id + ' input:checked')?.value) })`);
        if (state.hydrated || state.theme !== theme || state.choices.some(value => value !== theme)) mismatches.push({ name, expected: theme, ...state });
      }
      await page.send('Network.setBlockedURLs', { urls: [] });
      await page.reload();
      const hydrated = await page.eval(`({ theme: document.documentElement.dataset.theme || 'system', choices: ['page-theme', 'overlay-theme'].map(id => document.querySelector('#' + id + ' input:checked')?.value), paints: window.__appearancePaints })`);
      if (hydrated.theme !== theme || hydrated.choices.some(value => value !== theme) || hydrated.paints.some(value => value !== `${theme}:${theme}`)) mismatches.push({ name: 'hydration-' + theme + '-' + width, expected: theme, ...hydrated });
    }
  }
  await page.send('Network.setBlockedURLs', { urls: [] });
  for (const deadline = Date.now() + 5000; page.errors.length < errorsBefore + navigations && Date.now() < deadline;) await sleep(50);
  const blocked = page.errors.splice(errorsBefore);
  check(blocked.length === navigations && blocked.every(error => error === `TypeError: Failed to fetch dynamically imported module: ${args.base}/pkg/vink.js`), 'blocked hydration produces only the expected import failures');
  await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier });
  await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM] });
  check(mismatches.length === 0, `first paint and both native selections agree with the preference before hydration: ${JSON.stringify(mismatches)}`);

  // Another tab can write a preference after the bootstrap has applied this document's snapshot.
  await page.viewport(1440);
  await page.eval(`localStorage.removeItem('vink.ui.theme');
    localStorage.setItem('vink.ui.rail', 'compact'); localStorage.setItem('vink.ui.text', '200');`);
  const { identifier: changeRailStorage } = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: `
    const observer = new MutationObserver(() => {
      if (document.documentElement?.dataset.rail !== 'compact') return;
      observer.disconnect();
      localStorage.setItem('vink.ui.rail', 'expanded');
    });
    observer.observe(document, { subtree: true, attributes: true, attributeFilter: ['data-rail'] });` });
  try {
    await page.reload();
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true' &&
      localStorage.getItem('vink.ui.rail') === 'expanded' && ${RAIL_WIDTH} === 96 &&
      getComputedStyle(document.documentElement).fontSize === '32px' &&
      ['page', 'overlay'].every(prefix =>
        document.querySelector('#' + prefix + '-rail input:checked')?.value === 'compact' &&
        document.querySelector('#' + prefix + '-text input:checked')?.value === '200')`, 'hydration keeps the bootstrapped rail and text preferences even if storage changes');
  } finally {
    await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: changeRailStorage });
    await page.eval(`localStorage.removeItem('vink.ui.rail'); localStorage.removeItem('vink.ui.text');`);
    await page.reload();
  }
}

async function shellWithDirtyEditor(page, editorPath, written, view, selected) {
  const workId = editorPath.split('/').at(-1);
  const serverVersion = (await page.draft(workId)).version;
  console.log('shell utilities preserve the live dirty editor:');
  let writes = 0;
  const listener = (event) => {
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent' && ['PUT', 'POST'].includes(event.params.request.method)) writes++;
  };
  page.cdp.listeners.push(listener);
  check((await page.eval(RAIL_WIDTH)) === 96, 'desktop editor defaults to compact');
  await page.click('#navigation-toggle');
  check((await page.eval(RAIL_WIDTH)) === 220, 'manual preference can expand the editor rail');
  await page.click('#navigation-toggle');
  await page.tabToBody();
  await page.waitFor(`${selected} === ${JSON.stringify(LAST_LINE)}`, 'selection after rail changes');
  await openProfile(page);
  check(!(await page.dialogOpen('leave-editor')), 'opening Tôi utility does not invoke the leave guard');
  await page.click('#overlay-text label:has([value="200"])');
  await page.waitFor(`getComputedStyle(document.documentElement).fontSize === '32px'`, 'panel enlarges text in the dirty editor');
  await page.click('#overlay-rail label:has([value="expanded"])');
  await page.waitFor(`${RAIL_WIDTH} === 220`, 'explicit rail wins in the dirty editor');
  await page.click('#overlay-rail label:has([value="auto"])');
  await page.waitFor(`${RAIL_WIDTH} === 96`, 'automatic rail restores the focused editor default');
  await page.click('#overlay-text label:has([value="100"])');
  await page.waitFor(`getComputedStyle(document.documentElement).fontSize === '16px'`, 'panel restores the text size');
  for (const theme of ['light', 'system', 'dark']) {
    await chooseTheme(page, theme);
    await page.key('Escape');
    await openProfile(page);
  }
  await chooseTheme(page, 'dark');
  await page.capture('1280-editor-dirty-overlay-dark');
  await page.key('Escape');
  await page.eval(`location.hash = 'editor-fragment'`);
  await page.waitFor(`location.hash === '#editor-fragment'`, 'dirty editor fragment entry');
  await sleep(100);
  await openProfile(page);
  await page.eval(`history.back()`);
  await page.waitFor(`!${PROFILE_OPEN}`, 'Back closes appearance before dirty editor navigation');
  check(await page.eval(`location.pathname === '${editorPath}' && location.hash === '#editor-fragment' && !document.querySelector('[data-testid=leave-editor]').open`), 'fragment Back closes appearance and keeps the dirty editor URL without opening a second dialog');
  await page.eval(`history.back()`);
  await page.waitFor(`location.hash === ''`, 'plain fragment Back in dirty editor');
  check(await page.eval(`location.pathname === '${editorPath}' && !document.querySelector('[data-testid=leave-editor]').open`), 'plain fragment Back does not invoke the dirty editor leave guard');
  await openProfile(page);
  await page.key('Escape');
  await page.tabToBody();
  await page.waitFor(`${selected} === ${JSON.stringify(LAST_LINE)}`, 'selection after appearance utility');
  await page.eval(`(() => { const content = document.querySelector('.cm-content'); content.blur(); content.focus(); })()`);
  check((await page.eval(selected)) === LAST_LINE, 'native content focus immediately restores the CodeMirror selection');
  check(await page.eval(view) && (await page.eval(selected)) === LAST_LINE && (await page.body()) === written && (await page.status()) === 'unsaved' && (await page.eval(`document.querySelector('input[name=title]').value`)) === TITLE, 'rail, text size and theme preserve EditorView, selection, text and dirty state');

  const focusedSelection = `document.activeElement?.matches('.cm-content') && ${selected} === ${JSON.stringify(LAST_LINE)}`;
  const selectionAfterEscape = `!document.querySelector('[data-testid=leave-editor]').open && ${focusedSelection}`;
  for (const destination of ['/', '/ban-viet', '/toi']) {
    if (destination === '/toi') {
      await openProfile(page);
      await page.click('#open-me');
    } else {
      await page.click(`#navigation-primary a[href="${destination}"]`);
    }
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, `dirty guard to ${destination}`);
    check(await page.eval(`!${PROFILE_OPEN} && location.pathname === '${editorPath}'`), `real ${destination} navigation consults the existing guard`);
    await page.key('Escape');
    await page.waitFor(selectionAfterEscape, `selection after cancelling navigation to ${destination}`);
  }
  await openProfile(page);
  await page.viewport(390);
  await page.waitFor(`!${PROFILE_OPEN}`, 'editor overlay closes on mobile resize');
  check(await page.eval(view) && (await page.body()) === written && await page.eval(`location.pathname === '${editorPath}' && getComputedStyle(document.querySelector('.vk-ui-bottom-nav')).display === 'none'`), 'mobile resize retains editor and hides bottom bar');
  await page.capture('390-editor-dirty-dark');
  await assertBrandPlacement(page);
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`location.pathname === '${editorPath}' && document.querySelector('[data-testid=leave-editor]').open`, 'mobile editor’s Bàn viết link uses the same leave guard');
  await page.key('Escape');
  await page.waitFor(selectionAfterEscape, 'selection restored after mobile Escape');
  await page.viewport(1280);
  await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
  await page.waitFor(`innerWidth === 1280 && ${focusedSelection}`, 'selection after mobile resize');
  check((await page.eval(selected)) === LAST_LINE && writes === 0, 'utility changes sent no draft writes and retained the caret selection');
  check((await page.draft(workId)).version === serverVersion, 'shell and fragment traversal preserve the confirmed draft version');
  page.cdp.listeners.splice(page.cdp.listeners.indexOf(listener), 1);
  await openProfile(page);
  await chooseTheme(page, 'system');
  await page.key('Escape');
  await page.tabToBody();
  await page.waitFor(`${selected} === ${JSON.stringify(LAST_LINE)}`, 'selection after returning to system appearance');
}

// State anatomy and recovery targets at the same fixture, theme and CSS text scale.
// Focus each actual control so 200% content below the fold is inspected as well as the header.
async function captureRecovery(page, state, selector) {
  const before = await page.eval(`({width:innerWidth,height:innerHeight,theme:document.documentElement.dataset.theme,
    font:document.documentElement.style.fontSize,x:scrollX,y:scrollY})`);
  try {
    for (const theme of ['light', 'dark']) {
      await page.eval(`document.documentElement.dataset.theme='${theme}'`);
      for (const width of [1440, 1280, 1024, 834, 390, 320]) {
        await page.viewport(width);
        for (const scale of [100, 200]) {
          await page.eval(`document.documentElement.style.fontSize='${scale}%'`);
          await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
          const count = await page.eval(`document.querySelector(${JSON.stringify(selector)}).querySelectorAll('button,a[href]').length`);
          for (const index of new Set([0, count - 1])) {
            if (index < 0) continue;
            await page.eval(`(() => { const el=document.querySelector(${JSON.stringify(selector)}).querySelectorAll('button,a[href]')[${index}];
              if(document.activeElement===el) el.blur(); el?.focus(); })()`);
            const geometry = await page.eval(`(() => {
              const el=document.activeElement,r=el.getBoundingClientRect();
              const bar=document.querySelector('.vk-ui-bottom-nav');
              const limit=bar?.getClientRects().length ? bar.getBoundingClientRect().top : innerHeight;
              const label=el.querySelector('.vk-ui-button__label') || el;
              const walker=document.createTreeWalker(label,NodeFilter.SHOW_TEXT),brokenWords=[];
              const segmenter=new Intl.Segmenter('vi',{granularity:'word'});let wordCount=0;
              for(let node=walker.nextNode();node;node=walker.nextNode()) for(const word of segmenter.segment(node.textContent)) {
                if(!word.isWordLike) continue; wordCount++;
                const range=document.createRange();range.setStart(node,word.index);range.setEnd(node,word.index+word.segment.length);
                if(new Set([...range.getClientRects()].map(rect=>Math.round(rect.top))).size>1) brokenWords.push(word.segment);
              }
              // The first recovery carries the panel's primary emphasis, link or button alike.
              const background=getComputedStyle(el).backgroundColor;
              const primary=${index}!==0 || !/^rgba\(0, 0, 0, 0\)$|^transparent$/.test(background);
              // Native scroll positions round to CSS pixels; use the same 1px tolerance at both edges.
              return {ok:!!el.closest(${JSON.stringify(selector)}) && r.height>=44 && r.top>=-1 && r.bottom<=limit+1
                && document.documentElement.scrollWidth<=innerWidth && wordCount>0 && !brokenWords.length && primary,
                rect:r.toJSON(),limit,scrollWidth:document.documentElement.scrollWidth,wordCount,brokenWords,background};
            })()`);
            if (!geometry.ok) await page.capture(`recovery-failure-${state}-${width}-${scale}-${theme}`);
            check(geometry.ok, `${state} ${width}/${scale}/${theme}: recovery ${index + 1} is reachable above navigation without horizontal overflow${index === 0 ? ' and carries primary emphasis' : ''}${geometry.ok ? '' : JSON.stringify(geometry)}`);
          }
          await page.capture(`recovery-${state}-${width}-${scale}-${theme}`);
        }
      }
    }
  } finally {
    await page.eval(`document.documentElement.style.fontSize=${JSON.stringify(before.font)};
      ${before.theme ? `document.documentElement.dataset.theme=${JSON.stringify(before.theme)}` : 'delete document.documentElement.dataset.theme'}`);
    await page.viewport(before.width, before.height);
    await page.eval(`scrollTo(${before.x},${before.y})`);
  }
}

async function selectChevron(page) {
  console.log('Select #135: shared geometry and native keyboard in the desk:');
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.documentElement.dataset.hydrated==='true' && document.querySelector('#draft-sort')`, 'native sort');
  for (const theme of ['light','dark']) for (const width of [320,390,1440]) for (const scale of [100,200]) {
    await page.viewport(width);
    await page.eval(`document.documentElement.dataset.theme='${theme}';document.documentElement.style.fontSize='${scale}%';document.querySelector('#draft-sort').scrollIntoView({block:'center'})`);
    await page.eval('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))');
    const scenario=`select-desk-${theme}-${width}-${scale}`;
    await checkSelectGeometry(page,'#draft-sort',check,scenario);
    // Real native key dispatch, rather than changing .value or emitting a synthetic change.
    await page.eval(`document.querySelector('#draft-sort').focus()`);
    await selectCharacter(page,'t');
    await page.waitFor(`document.querySelector('#draft-sort').value==='title'`, 'keyboard A–Z');
    await checkSelectGeometry(page,'#draft-sort',check,`${scenario} A–Z`);
    await selectCharacter(page,'m');
    await page.waitFor(`document.querySelector('#draft-sort').value==='recent'`, 'keyboard recent');
    await page.capture(scenario);
  }
  await page.send('Emulation.setEmulatedMedia',{features:[LIGHT_SYSTEM,{name:'forced-colors',value:'active'}]});
  await page.eval(`document.querySelector('#draft-sort').focus()`);
  const focus=await page.eval(`(() => {const s=document.querySelector('#draft-sort'),c=getComputedStyle(s);return {native:document.activeElement===s,outline:c.outlineWidth,offset:c.outlineOffset,icon:getComputedStyle(s.parentElement.querySelector('svg')).color,system:(()=>{const p=document.createElement('span');p.style.color='ButtonText';document.body.append(p);const c=getComputedStyle(p).color;p.remove();return c})()}})()`);
  check(focus.native && focus.outline==='2px' && focus.offset==='-2px' && focus.icon===focus.system,'forced-colors: native sort keeps focus and one inset perimeter');
  await checkSelectGeometry(page,'#draft-sort',check,'desk forced-colors');
  await page.capture('select-desk-forced-colors-1440-200');
  await page.send('Emulation.setEmulatedMedia',{features:[LIGHT_SYSTEM]});
  await page.eval(`document.documentElement.style.fontSize='';delete document.documentElement.dataset.theme`);
  await page.viewport(1280);
}

async function draftR6Hierarchy(page) {
  console.log('Desk R6: same-fixture geometry, complete row targets and primary action:');
  const title = 'Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ tiếng Việt và những khổ thơ còn đang viết dở'.normalize('NFD');
  const cards = [title, 'Hiên nhà còn nắng', 'Bên kia ô cửa, nơi tiếng gió giữ lại những khổ thơ chưa kịp đặt tên'].map((title, i) => ({
    work_id: `88888888-8888-4888-8888-${String(i).padStart(12, '0')}`, title, version: 3-i, updated_at: '2026-11-09T08:42:00Z',
  }));
  const gate = new DraftGate(page);
  gate.list = {body:{drafts:cards}};
  await gate.enable();
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length===3 && document.querySelector('#draft-scope').textContent==='Tìm trong 3 bản thảo đã tải.'`, 'R6 fixture with settled inventory');
  for (const [width, scale, theme] of [[390,100,'light'], [320,200,'dark'], [1440,100,'light'], [1280,100,'light'], [1440,200,'dark'], [1280,200,'dark']]) {
    await page.viewport(width);
    await page.eval(`document.documentElement.dataset.theme='${theme}';document.documentElement.style.fontSize='${scale}%';scrollTo(0,0)`);
    await page.eval(`Promise.all([...document.fonts].map(f=>f.load().catch(()=>{})))`);
    await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
    await page.capture(`r6-${width}-${scale}-${theme}`);
    check(await page.eval(`(() => {
      const rows=[...document.querySelectorAll('.vk-draft-row__link')], list=document.querySelector('#draft-results'), cta=document.querySelector('[data-testid=resume-draft]');
      const style=getComputedStyle(cta), listStyle=getComputedStyle(list);
      return document.documentElement.scrollWidth<=innerWidth && cta.getAttribute('href')==='/ban-viet/${cards[0].work_id}' &&
        style.backgroundColor!==getComputedStyle(cta.closest('.vk-ui-surface')).backgroundColor && listStyle.backgroundColor!=='rgba(0, 0, 0, 0)' &&
        rows.every((row,i)=>row.getAttribute('href')==='/ban-viet/'+${JSON.stringify(cards)}[i].work_id && row.querySelector('.vk-draft-row__title').textContent===${JSON.stringify(cards)}[i].title &&
          getComputedStyle(row).paddingLeft===getComputedStyle(row).paddingRight && row.querySelectorAll('a,button,input').length===0 && !row.querySelector('.vk-draft-row__mark'));
    })()`), `R6 ${width}/${scale}/${theme}: primary resume, one list surface, equal insets, complete titles and exact IDs`);
    if (width===390 && scale===100) {
      check(await page.eval(`(() => {
        const rect=s=>document.querySelector(s).getBoundingClientRect(), resume=rect('#resume'), search=rect('#draft-search'), scope=rect('#draft-scope'), sort=rect('#draft-sort'), refresh=rect('#refresh-drafts'), label=rect('#draft-sort-label');
        const range=document.createRange();range.selectNodeContents(document.querySelector('#draft-scope'));
        return resume.height<=250 && search.top<=466.765625-48 && new Set([...range.getClientRects()].map(r=>Math.round(r.top))).size===1 &&
          scope.top>=search.bottom && scope.bottom<=sort.top && Math.abs(sort.bottom-refresh.bottom)<1 && label.bottom>=sort.top &&
          Math.abs(rect('[data-testid=resume-draft]').top-rect('#resume-title-toggle').top)<1;
      })()`), 'R6 390/100: resume <=250px, search moves up >=48px, one scope line and compact action/tool rows');
    }
    await page.eval(`document.querySelector('#drafts').scrollIntoView({block:'start'})`);
    await page.capture(`r6-list-${width}-${scale}-${theme}`);
    await textContrast(page, theme, 'desk');
    if (width===320 && scale===200) {
      await page.eval(`(() => {const sort=document.querySelector('#draft-sort');sort.value='title';sort.dispatchEvent(new Event('change',{bubbles:true}))})()`);
      check(await page.eval(`document.querySelector('.vk-draft-sort-help')?.textContent==='Bỏ qua dấu khi sắp xếp.' && document.documentElement.scrollWidth<=innerWidth && document.querySelector('[data-testid=resume-draft]').getAttribute('href')==='/ban-viet/${cards[0].work_id}' && ['#draft-sort','#refresh-drafts'].every(s=>{const r=document.querySelector(s).getBoundingClientRect();return r.left>=0 && r.right<=innerWidth})`), 'R6 320/200 A–Z: tools and accent-folding caption reflow without changing resume');
      await page.capture('r6-sort-320-200-dark');
      await page.eval(`(() => {const sort=document.querySelector('#draft-sort');sort.value='recent';sort.dispatchEvent(new Event('change',{bubbles:true}))})()`);
    }
  }
  await page.viewport(390);
  await page.eval(`document.documentElement.style.fontSize='100%'`);
  for (const forced of [false,true]) {
    await page.send('Emulation.setEmulatedMedia',{features:[{name:'forced-colors',value:forced?'active':'none'}]});
    await page.eval(`document.querySelector('[data-testid=resume-draft]').focus()`);
    await page.key('Tab');
    check(await page.eval(`document.activeElement.id==='resume-title-toggle' && document.activeElement.matches(':focus-visible')`), 'R6 primary link leads to disclosure in keyboard order');
    await page.key('Enter');
    check(await page.eval(`document.activeElement.id==='resume-title-toggle' && document.querySelector('#resume h2').textContent===${JSON.stringify(title)} && getComputedStyle(document.querySelector('#resume h2')).overflow==='visible'`), 'R6 disclosure keeps focus and full NFD title');
    await page.key('Enter');
    await page.key('Tab', SHIFT);
    check(await page.eval(`document.activeElement.matches('[data-testid=resume-draft]:focus-visible') && parseFloat(getComputedStyle(document.activeElement).outlineWidth)>=2 && document.activeElement.getBoundingClientRect().bottom<=document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().top`), `R6 ${forced?'forced-colors':'normal'}: primary link focus is visible above bottom navigation`);
    await page.capture(`r6-focus-${forced?'forced-colors':'normal'}`);
  }
  await page.send('Emulation.setEmulatedMedia',{features:[LIGHT_SYSTEM]});
  await page.eval(`document.documentElement.style.fontSize=''`);
  await gate.disable();
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
  cards[43].title = cards[42].title; // Repeated titles still open their own work IDs.
  gate.list = { body: { drafts: cards } };
  gate.holdGets = true;
  await gate.enable();
  await page.viewport(1440);
  await page.goto(`${args.base}/ban-viet`);
  const initial = await gate.next('gets');
  await page.waitFor(`document.querySelector('[data-testid=draft-loading]')`, 'initial dashboard loading');
  check(await page.eval(`document.querySelector('[data-testid=draft-content]').getAttribute('aria-busy') === 'true' && !document.querySelector('[data-testid=draft-empty]') && !document.querySelector('#create-draft')`), 'loading is distinct from empty and cannot create');
  gate.holdGets = false;
  await gate.answerGet(initial);
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 100`, '100 loaded rows');
  const resumeHref = `/ban-viet/${cards[0].work_id}`;
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resumeHref)} && document.querySelector('#resume h2').textContent === ${JSON.stringify(cards[0].title)}`), 'resume opens exactly the first authorized card and preserves its full combining title');
  check(await page.eval(`document.querySelector('#draft-scope').textContent.includes('Tìm trong 100 bản thảo gần đây đã tải') && document.querySelector('#draft-search-help').textContent === 'Trong danh sách đã tải'`), '100 loaded rows are not claimed as a global total');
  check(await page.eval(`[...document.querySelectorAll('[data-testid=draft-row]')].every((row, i) => row.querySelector('a').getAttribute('href') === '/ban-viet/' + ${JSON.stringify(cards)}[i].work_id && row.textContent.includes('Phiên bản ' + (i + 1)) && row.querySelector('time').dateTime === '2026-11-01T08:30:00Z')`), 'every row shows the provider work_id, version and update timestamp');
  check(manuscriptRequests === 0, 'list/resume never fetch full manuscripts (no N+1)');
  check(await page.eval(`!document.querySelector('#desk').textContent.match(/Chưa đăng|Đã đăng|góp ý|thể loại|D-05|VINK_DEV_PRINCIPAL/)`), 'no invented publication/profile/feedback state or implementation copy');
  await page.eval(`document.querySelector('[data-testid=resume-draft]').focus()`);
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('.vk-draft-search button[type=submit]:focus-visible')`), 'keyboard order moves from Viết tiếp to the leading search submit');
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.id === 'draft-search' && document.activeElement.matches(':focus-visible')`), 'keyboard continues to the stable search input with visible focus');
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.id === 'draft-sort'`), 'empty search has no clear target; sort follows in keyboard order');
  await page.eval(`document.querySelector('#refresh-drafts').focus()`);
  await page.waitFor(`!document.querySelector('#refresh-drafts-tooltip').hidden`, 'refresh tooltip on keyboard focus');
  check(await page.eval(`document.activeElement.id === 'refresh-drafts' && document.querySelector('#refresh-drafts').getAttribute('aria-label') === 'Tải lại' && !document.querySelector('#refresh-drafts-tooltip :is(button,a,input,[tabindex])')`), 'refresh has its own name and the shared tooltip never takes focus');
  await page.key('Escape');
  check(await page.eval(`document.querySelector('#refresh-drafts-tooltip').hidden && document.activeElement.id === 'refresh-drafts'`), 'Escape dismisses only the tooltip and keeps trigger focus');
  await page.viewport(390);
  const toggleTitle = '#resume-title-toggle';
  await page.eval(`document.querySelector('${toggleTitle}').focus()`);
  await page.key('Enter');
  check(await page.eval(`document.querySelector('${toggleTitle}').getAttribute('aria-expanded') === 'true' && getComputedStyle(document.querySelector('#resume h2')).webkitLineClamp === 'none' && getComputedStyle(document.querySelector('#resume h2')).overflow === 'visible' && document.querySelector('#resume h2').textContent === ${JSON.stringify(cards[0].title)} && document.activeElement.matches('${toggleTitle}:focus-visible')`), 'Enter reveals the entire combining title in place and keeps visible keyboard focus');
  await page.send('Input.dispatchKeyEvent', { type: 'keyDown', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 });
  await page.send('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space', windowsVirtualKeyCode: 32 });
  check(await page.eval(`document.querySelector('${toggleTitle}').getAttribute('aria-expanded') === 'false' && document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resumeHref)}`), 'Space collapses the title without changing the resume work ID');
  const { nodes: deskAX } = await page.send('Accessibility.getFullAXTree');
  check(deskAX.some(node => node.role?.value==='searchbox' && node.name?.value==='Tìm bản thảo'), 'hidden native Desk label remains the searchbox accessible name');
  check(deskAX.some(node => node.role?.value === 'link' && node.name?.value.includes(cards[0].title)) && deskAX.some(node => node.role?.value === 'heading' && node.name?.value === cards[0].title) && deskAX.some(node => node.role?.value === 'button' && node.name?.value === 'Xem đủ tên bản thảo' && node.properties?.some(p => p.name === 'expanded' && p.value.value === false)) && deskAX.some(node => node.role?.value === 'link' && node.name?.value === 'Viết tiếp'), 'accessibility tree keeps the full title, named primary link and collapsed disclosure button');
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme = '${theme}'`);
    await page.eval(`Promise.all(document.getAnimations().map(animation => animation.finished.catch(() => {})))`);
    await textContrast(page, theme, 'desk');
    for (const scale of [100, 200]) {
      await page.eval(`document.documentElement.style.fontSize = '${scale}%'`);
      for (const width of [1440, 1280, 1024, 834, 390, 320]) {
        await page.viewport(width);
        await page.eval(`scrollTo(0, 0)`);
        await page.capture(`desk-long-${width}-${scale}-${theme}`);
        check(await page.eval(`(() => {
          const title = document.querySelector('#resume h2'), cta = document.querySelector('[data-testid=resume-draft]');
          const t = title.getBoundingClientRect(), c = cta.getBoundingClientRect();
          const stacked = getComputedStyle(title).webkitLineClamp === '3';
          return document.documentElement.scrollWidth <= innerWidth && (!stacked || t.height <= parseFloat(getComputedStyle(title).lineHeight) * 3 + 1) &&
            c.width >= 48 && c.height >= 48 && c.left >= 0 && c.right <= innerWidth && (c.top >= t.bottom || c.left >= t.right) &&
            c.bottom <= document.querySelector('#drafts').getBoundingClientRect().top && cta.textContent === 'Viết tiếp' &&
            document.querySelector('#resume h2').textContent === ${JSON.stringify(cards[0].title)};
        })()`), `${theme} ${width}px / ${scale}%: compact title or full wide title, primary link remains separate and no horizontal overflow`);
        if (width === 390 && scale === 100) {
          check(await page.eval(`document.querySelector('#draft-tools').getBoundingClientRect().top < document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().top && document.querySelector('[data-testid=resume-draft]').getBoundingClientRect().bottom < document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().top`), `${theme} 390px first viewport exposes Viết tiếp and the list tools`);
        }
        await page.eval(`document.querySelector('#drafts').scrollIntoView({ block: 'start' })`);
        await searchBarGeometry(page, '#draft-search', `Desk ${width}/${scale}/${theme}`);
        await quietListSurfaces(page, '.vk-draft-row__link', `Desk ${width}/${scale}/${theme}`);
        await page.capture(`desk-list-${width}-${scale}-${theme}`);
        if (width === 320 && scale === 200) {
          await page.eval(`document.querySelector('.vk-draft-row__title').scrollIntoView({block:'start'})`);
          await page.capture(`desk-row-start-${width}-${scale}-${theme}`);
          await page.eval(`document.querySelector('.vk-draft-row__open').scrollIntoView({block:'center'})`);
          await page.capture(`desk-row-end-${width}-${scale}-${theme}`);
          await page.eval(`document.querySelector('#drafts').scrollIntoView({block:'start'})`);
        }
        check(await page.eval(`['#draft-search', '#draft-sort', '#refresh-drafts'].every(s => { const r = document.querySelector(s).getBoundingClientRect(); return r.width > 0 && r.left >= 0 && r.right <= innerWidth; })`), `${theme} ${width}px / ${scale}%: search, sort and refresh reflow within the viewport`);
        check(await page.eval(`(() => {
          const row = document.querySelector('.vk-draft-row__link'), title = row.querySelector('.vk-draft-row__title');
          const t = title.getBoundingClientRect(), r = row.getBoundingClientRect(), meta = row.querySelector('.vk-draft-meta').getBoundingClientRect();
          const range = document.createRange(); range.selectNodeContents(title);
          return title.textContent === ${JSON.stringify(cards[0].title)} && getComputedStyle(title).overflow === 'visible' &&
            [...range.getClientRects()].every(line => line.left >= t.left - 1 && line.right <= t.right + 1 && line.top >= r.top - 1 && line.bottom <= r.bottom + 1) &&
            meta.top >= t.bottom && meta.bottom <= r.bottom && row.getAttribute('href') === ${JSON.stringify(resumeHref)} &&
            row.querySelectorAll('button, input, a').length === 0;
        })()`), `${theme} ${width}px / ${scale}%: recent row keeps every title line, metadata and one navigation target`);
        if (width <= 390) {
          check(await page.eval(`(() => {
            const row = document.querySelector('.vk-draft-row__link'), r = row.getBoundingClientRect(), style = getComputedStyle(row);
            const text = row.querySelector('.vk-draft-row__text').getBoundingClientRect(), open = row.querySelector('.vk-draft-row__open').getBoundingClientRect();
            const available = r.width - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
            return text.width >= available - 1 && open.top >= text.bottom && open.left >= text.left - 1 && open.right <= text.right + 1 &&
              row.querySelector('.vk-draft-row__open').getAttribute('aria-hidden') === 'true' && row.querySelector('.vk-draft-row__open .vk-ui-icon').getBoundingClientRect().width >= 18 && !row.textContent.includes('Mở bản thảo');
          })()`), `${theme} ${width}px / ${scale}%: recent title uses the whole inner measure, with a readable open affordance below`);
        }
        check(await page.eval(`(() => {
          const rect = s => document.querySelector(s).getBoundingClientRect();
          const heading = rect('#heading'), resume = rect('#resume'), list = rect('#drafts'), controls = rect('.vk-draft-controls'), scope = rect('#draft-scope'), results = rect('#draft-results');
          const search = rect('.vk-draft-search .vk-ui-search__shell'), sort = rect('#draft-sort'), reload = rect('#refresh-drafts');
          return [resume, list, controls, scope, results, search].every(r => Math.abs(r.left - heading.left) < 1) &&
            Math.abs(resume.right - list.right) < 1 && Math.abs(results.right - controls.right) < 1 &&
            ((sort.left >= search.right && Math.abs(sort.bottom - reload.bottom) < 1) ||
             (sort.top >= search.bottom && Math.abs(sort.bottom - reload.bottom) < 1) || reload.top >= sort.bottom);
        })()`), `${theme} ${width}px / ${scale}%: heading, resume, search, scope and list share an axis; tools align or stack`);
      }
    }
    const wholeWords = await page.eval(`(() => {
      const title = document.querySelector('#resume h2');
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
    await page.eval(`document.querySelector('${toggleTitle}').focus()`);
    await page.key('Enter');
    await page.eval(`document.querySelector('#resume h2').scrollIntoView({ block: 'start' })`);
    await page.capture(`desk-title-expanded-320-200-${theme}`);
    check(await page.eval(`getComputedStyle(document.querySelector('#resume h2')).webkitLineClamp === 'none' && getComputedStyle(document.querySelector('#resume h2')).overflow === 'visible'`), `${theme} 320px / 200%: full title is visible after disclosure`);
    await page.key('Enter');
    await page.key('Tab', SHIFT);
    check(await page.eval(`document.activeElement.matches('[data-testid=resume-draft]:focus-visible') && document.activeElement.getBoundingClientRect().bottom <= document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().top`), `${theme} 320px / 200%: keyboard reveals the primary link above the bottom bar`);
    await page.capture(`desk-continue-320-200-${theme}`);
    check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), `${theme} dashboard keeps 320px width at 200% text`);
    await page.eval(`document.documentElement.style.fontSize = ''`);
  }
  await page.viewport(1440);
  // One actual summary, then an unbroken Unicode title: disclosure never changes the DTO.
  for (const [variant, title] of [['blank', ''], ['word', '[Fixture] ' + 'Đườngỹỷữợ'.repeat(20)]]) {
    gate.list = { body: { drafts: [{ ...cards[0], title }] } };
    await page.click('#refresh-drafts');
    await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 1 && document.querySelector('#resume h2').textContent === ${JSON.stringify(title || 'Chưa đặt tên')} && document.querySelector('[data-testid=draft-content]').getAttribute('aria-busy') === 'false'`, `one ${variant} draft refreshed`);
    await page.waitFor(`document.querySelector('#draft-scope').textContent.includes('Tìm trong 1 bản thảo đã tải')`, 'settled one-row inventory');
    check(await page.eval(`document.querySelector('#resume h2').textContent === ${JSON.stringify(title || 'Chưa đặt tên')} && document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resumeHref)} && document.querySelector('#draft-scope').textContent.includes('Tìm trong 1 bản thảo đã tải') && document.querySelector('.vk-draft-row__title').textContent === ${JSON.stringify(title || 'Chưa đặt tên')}`), `one ${variant} summary preserves title, scope and resume identity`);
    for (const scale of [100, 200]) {
      await page.viewport(320);
      await page.eval(`document.documentElement.style.fontSize = '${scale}%'`);
      if (variant === 'word') await page.click(toggleTitle);
      await page.eval(`document.querySelector('#resume').scrollIntoView({ block: 'start' })`);
      await page.capture(`desk-one-${variant}-320-${scale}`);
      check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), `one ${variant} title reflows at 320px / ${scale}%`);
      if (variant === 'word') {
        check(await page.eval(`getComputedStyle(document.querySelector('#resume h2')).webkitLineClamp === 'none' && getComputedStyle(document.querySelector('#resume h2')).overflow === 'visible'`), 'an expanded unbroken title is fully available');
        await page.click(toggleTitle);
      }
    }
  }
  await page.eval(`document.documentElement.style.fontSize = ''`);
  await page.viewport(1440);
  gate.list = { body: { drafts: cards } };
  await page.click('#refresh-drafts');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 100`, 'restore 100 loaded rows');
  const search = async text => {
    await page.eval(`document.querySelector('#draft-search').focus(); document.querySelector('#draft-search').select()`);
    await page.send('Input.insertText', { text });
  };
  await page.waitFor(`document.querySelector('[data-testid=draft-result-count]').textContent.includes('Tìm trong 100')`, 'settled inventory before typing');
  await page.eval(`window.__resultAnnouncements=[]; window.__resultObserver = new MutationObserver(() => window.__resultAnnouncements.push(document.querySelector('[data-testid=draft-result-count]').textContent)); window.__resultObserver.observe(document.querySelector('[data-testid=draft-result-count]'), {childList:true,subtree:true,characterData:true})`);
  await search('duong ve');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 2`, 'accentless Vietnamese results');
  await search('ĐƯỜNG VỀ CÓ NẮNG'.normalize('NFD'));
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 1`, 'combining accented results');
  await search('không có tên này');
  await page.waitFor(`document.querySelector('[data-testid=draft-no-results]')`, 'no results');
  check(await page.eval(`window.__resultAnnouncements.length===0 && document.activeElement.id==='draft-search'`), 'filtering successive input does not announce every keystroke or move focus');
  await page.waitFor(`document.querySelector('[data-testid=draft-result-count]').textContent==='0 trong 100 bản thảo gần đây đã tải.'`, 'one settled no-result announcement');
  check(await page.eval(`window.__resultAnnouncements.length===1 && document.querySelectorAll('#drafts [role=status]').length===1`), 'only one live result status announces the settled query');
  await page.eval(`window.__resultObserver.disconnect()`);
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resumeHref)}`), 'search never changes the most recent resume target');
  await page.viewport(390);
  await page.eval(`document.querySelector('#drafts').scrollIntoView({ block: 'start' })`);
  await page.capture('desk-no-results-390');
  check(await page.eval(`!!document.querySelector('[data-testid=draft-no-results] .vk-ui-panel__actions #clear-draft-search')`), 'draft clear belongs to the no-results panel');
  await captureRecovery(page, 'desk-no-results', '[data-testid=draft-no-results]');
  await page.click('#clear-draft-search');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 100`, 'clear query');
  await page.waitFor(`document.activeElement?.id === 'draft-search'`, 'clearing draft results restores search focus');
  await page.eval(`const sort = document.querySelector('#draft-sort'); sort.value = 'title'; sort.dispatchEvent(new Event('change', { bubbles: true }))`);
  await page.waitFor(`document.querySelector('[data-testid=draft-row]').textContent.includes('[Fixture] Bản thảo 10')`, 'title sort');
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === ${JSON.stringify(resumeHref)} && document.querySelector('#resume .vk-draft-meta').textContent.includes('Phiên bản 1') && document.querySelector('#resume time').dateTime === '2026-11-01T08:30:00Z'`), 'sorting preserves the newest resume work, version and timestamp');
  await search('duong ve');
  await page.eval(`window.__draftSearchInput = document.querySelector('#draft-search'); window.__draftSubmits=0; document.querySelector('.vk-draft-search').addEventListener('submit',()=>window.__draftSubmits++)`);
  await page.key('Enter');
  await page.click('.vk-draft-search button[type=submit]');
  check(await page.eval(`!location.search && window.__draftSubmits===2 && document.querySelectorAll('[data-testid=draft-row]').length===2 && document.querySelector('[data-testid=resume-draft]').getAttribute('href')===${JSON.stringify(resumeHref)}`), 'Desk Enter and click submit stay local and preserve sort/resume/scope');
  await page.eval(`window.__draftSearchInput.dispatchEvent(new CompositionEvent('compositionstart',{bubbles:true}))`);
  await page.eval(`window.__draftSearchInput.focus()`);
  await page.key('Enter');
  check(await page.eval(`window.__draftSubmits===2 && !location.search && window.__draftSearchInput.value==='duong ve'`), 'Desk synthetic composition Enter never submits or clears the private query');
  await page.eval(`window.__draftSearchInput.dispatchEvent(new CompositionEvent('compositionend',{bubbles:true}))`);
  gate.list = { status: 503, body: { error: { code: 'dependency_unavailable', message: 'test fixture' } } };
  gate.holdGets = true;
  await page.click('#refresh-drafts');
  const pendingRefresh = await gate.next('gets');
  check(await page.eval(`document.querySelector('[data-testid=draft-content]').getAttribute('aria-busy') === 'true' && document.querySelectorAll('[data-testid=draft-row]').length === 2 && window.__draftSearchInput === document.querySelector('#draft-search') && document.querySelector('#draft-search').value === 'duong ve' && !location.search`), 'pending refresh keeps loaded rows and private query/input without URL persistence');
  gate.holdGets = false;
  await gate.answerGet(pendingRefresh);
  await page.waitFor(`document.querySelector('[data-testid=desk-error]')`, 'dependency failure');
  check(await page.eval(`!document.querySelector('[data-testid=draft-empty]') && document.querySelectorAll('[data-testid=draft-row]').length === 2 && document.querySelector('#draft-search').value === 'duong ve' && document.querySelector('[data-testid=desk-error]').textContent.includes('tải thành công gần nhất') && window.__draftSearchInput === document.querySelector('#draft-search')`), 'temporary refresh failure preserves the labelled last-success list, query and input identity');
  check(await page.eval(`!!document.querySelector('[data-testid=desk-error] .vk-ui-panel__actions #retry-drafts')`), 'draft retry belongs to the error panel');
  await captureRecovery(page, 'desk-unavailable', '[data-testid=desk-error]');
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme = '${theme}'`);
    await page.eval(`Promise.all(document.getAnimations().filter(a => Number.isFinite(a.effect.getComputedTiming().endTime)).map(a => a.finished.catch(() => {})))`);
    for (const width of [1440, 320]) {
      await page.viewport(width);
      await page.eval(`scrollTo(0, 0)`);
      await page.capture(`desk-error-${width}-${theme}`);
    }
  }
  gate.list = { body: { drafts: cards } };
  await page.click('#retry-drafts');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 2`, 'retry with preserved query');
  check(await page.eval(`document.querySelector('#draft-search').value === 'duong ve' && document.querySelector('#draft-sort').value === 'title'`), 'query and sort survive failed refresh and retry');
  gate.list = { status: 403, body: { error: { code: 'forbidden', message: 'test fixture' } } };
  await page.click('#refresh-drafts');
  await page.waitFor(`document.querySelector('[data-testid=desk-error]')?.textContent.includes('không có quyền')`, 'forbidden dashboard');
  check(await page.eval(`!document.querySelector('[data-testid=draft-empty]') && !document.querySelector('[data-testid=draft-resume]') && !document.querySelector('#create-draft')`), 'forbidden remains distinct from empty and never exposes resume/create');
  await captureRecovery(page, 'desk-forbidden', '[data-testid=desk-error]');
  gate.list = { status: 401, body: { error: { code: 'unauthenticated', message: 'test fixture' } } };
  await page.click('#retry-drafts');
  await page.waitFor(`document.querySelector('[data-testid=desk-error]')?.textContent.includes('chưa đăng nhập')`, 'unauthenticated dashboard');
  await captureRecovery(page, 'desk-unauthenticated', '[data-testid=desk-error]');
  check(await page.eval(`!document.querySelector('#create-draft') && document.querySelector('[data-testid=desk-error] a').getAttribute('href') === '/'`), '401 has an honest public-reading action and no fake login/create');
  gate.list = { body: { drafts: [] } };
  await page.click('#retry-drafts');
  await page.waitFor(`document.querySelector('[data-testid=draft-empty]')`, 'empty dashboard');
  check(await page.eval(`document.querySelectorAll('#create-draft').length === 1 && document.querySelector('#create-draft').textContent === 'Viết bản thảo đầu tiên' && !document.querySelector('[data-testid=draft-resume]')`), 'empty has exactly one primary create action');
  check(await page.eval(`!!document.querySelector('[data-testid=draft-empty] .vk-ui-panel__actions #create-draft')`), 'first create belongs to the empty panel');
  await captureRecovery(page, 'desk-empty', '[data-testid=draft-empty]');
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme = '${theme}'`);
    await page.eval(`Promise.all(document.getAnimations().filter(a => Number.isFinite(a.effect.getComputedTiming().endTime)).map(a => a.finished.catch(() => {})))`);
    for (const width of [1440, 320]) {
      await page.viewport(width);
      await page.eval(`scrollTo(0, 0)`);
      await page.capture(`desk-empty-${width}-${theme}`);
    }
  }
  check(manuscriptRequests === 0, 'search, sort and refresh never prefetch manuscripts');
  page.cdp.listeners = page.cdp.listeners.filter(listener => listener !== countManuscripts);
  await page.click('#create-draft');
  const rejected = await gate.next('posts');
  await gate.fulfill(rejected, 403, { error: { code: 'forbidden', message: 'test fixture' } });
  await page.waitFor(`document.querySelector('[data-testid=create-notice]')?.textContent.includes('bị từ chối')`, 'explicit create rejection');
  check(await page.eval(`!document.querySelector('[data-testid=create-notice]').textContent.includes('có thể đã')`), 'pre-write rejection stays distinct from an uncertain outcome');
  check(await page.eval(`document.querySelector('#create-draft').getAttribute('aria-disabled') === 'true'`), 'permission rejection requires an access recheck before another create');
  // Return to the real store for the commit + lost response case.
  gate.list = null;
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.querySelector('[data-testid=resume-draft]')`, 'real authorized list');
  const before = await page.eval(`fetch('/api/v1/drafts').then(r => r.json()).then(list => list.drafts.map(d => d.work_id))`);
  await page.click('#create-draft');
  const lost = await gate.next('posts');
  const postsBefore = gate.postCount;
  await page.eval(`for (let i = 0; i < 5; i++) document.querySelector('#create-draft').click()`);
  check(await page.eval(`document.querySelector('#create-draft').getAttribute('aria-disabled') === 'true' && document.querySelector('#refresh-drafts').getAttribute('aria-disabled') === 'true'`), 'create and refresh are disabled during the single POST');
  gate.dropAck = true;
  await page.send('Fetch.continueRequest', { requestId: lost });
  await page.waitFor(`document.querySelector('[data-testid=create-notice]')?.textContent.includes('có thể đã được tạo')`, 'uncertain create after lost 201');
  check(gate.droppedStatus === 201 && gate.postCount === postsBefore, 'one real POST committed and its ack was dropped without duplicate submissions');
  check(await page.eval(`document.querySelector('#create-draft').getAttribute('aria-disabled') === 'true' && location.pathname === '/ban-viet'`), 'uncertain creation stays on desk and blocks another POST');
  check(await page.eval(`!!document.querySelector('[data-testid=create-notice] [data-panel="error:uncertain"] .vk-ui-panel__actions #reconcile-drafts')`), 'uncertain creation offers GET reconciliation in its own warning panel');
  await captureRecovery(page, 'desk-uncertain-create', '[data-testid=create-notice]');
  // Failed reconciliation must keep creation blocked.
  gate.list = { status: 503, body: { error: { code: 'dependency_unavailable', message: 'test fixture' } } };
  await page.click('#reconcile-drafts');
  await page.waitFor(`document.querySelector('[data-testid=desk-error]')`, 'failed reconciliation');
  check(await page.eval(`document.querySelector('[data-testid=create-notice]').textContent.includes('có thể đã được tạo')`), 'failed reconciliation preserves the unknown outcome');
  gate.list = null;
  await page.click('#reconcile-drafts');
  await page.waitFor(`document.querySelector('#create-draft')?.textContent === 'Tạo thêm bản thảo'`, 'successful list reconciliation');
  await page.waitFor(`document.activeElement?.id === 'draft-search'`, 'GET reconciliation restores focus to the loaded search');
  const after = await page.eval(`fetch('/api/v1/drafts').then(r => r.json()).then(list => list.drafts.map(d => d.work_id))`);
  const created = after.filter(id => !before.includes(id));
  check(created.length === 1 && gate.postCount === postsBefore, 'GET reconciliation found the committed draft without retrying POST');
  check(await page.eval(`document.querySelector('[data-testid=resume-draft]').getAttribute('href') === '/ban-viet/' + ${JSON.stringify(created[0])}`), 'newly reconciled draft is the resume target');
  await page.click('[data-testid=resume-draft]');
  await page.waitFor(`${EDITOR_READY} && location.pathname === '/ban-viet/${created[0]}'`, 'resume opens the real draft');
  check((await page.status()) === 'saved', 'reconciled draft opens at its real saved version');
  const renamed = '[Fixture] Tiêu đề vừa sửa — tiếng Việt ỹ ỷ ữ ợ';
  await page.typeAtEnd('input[name=title]', renamed);
  await page.key('s', page.mod);
  await page.waitStatus('saved', 'renamed title acknowledged by the real server');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('[data-testid=resume-draft]')`, 'return to dashboard');
  check(await page.eval(`document.querySelector('#resume h2').textContent === ${JSON.stringify(renamed)} && document.querySelector('[data-testid=resume-draft]').getAttribute('href') === '/ban-viet/' + ${JSON.stringify(created[0])} && document.querySelector('#resume .vk-draft-meta').textContent.includes('Phiên bản 2')`), 'return after a real title save shows the same work with its acknowledged title and version');
  // The same real row opens by keyboard and by a browser touch event, without a
  // second nested target or a tooltip/disclosure needed to read its full title.
  await page.viewport(390);
  const realRow = '.vk-draft-row__link[href="/ban-viet/' + created[0] + '"]';
  await page.eval(`document.querySelector(${JSON.stringify(realRow)}).focus()`);
  await page.key('Tab'); await page.key('Tab', SHIFT);
  check(await page.eval(`document.activeElement.matches(${JSON.stringify(realRow)} + ':focus-visible')`), 'recent row has visible keyboard focus');
  const { nodes: rowAX } = await page.send('Accessibility.getFullAXTree');
  check(rowAX.some(node => node.role?.value === 'link' && node.name?.value.includes(renamed)), 'recent row accessibility name retains the complete title');
  await page.key('Enter');
  await page.waitFor(`${EDITOR_READY} && location.pathname === '/ban-viet/${created[0]}'`, 'keyboard opens the same real row');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector(${JSON.stringify(realRow)})`, 'return to the same recent row');
  const point = await page.eval(`(() => {
    const title = document.querySelector(${JSON.stringify(realRow)} + ' .vk-draft-row__title');
    title.scrollIntoView({block:'center'}); const r = title.getBoundingClientRect();
    return {x:r.left + r.width / 2, y:r.top + r.height / 2};
  })()`);
  await page.send('Emulation.setTouchEmulationEnabled', { enabled: true });
  await page.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ ...point, id: 1 }] });
  await page.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  await page.waitFor(`${EDITOR_READY} && location.pathname === '/ban-viet/${created[0]}'`, 'touch opens the same real row');
  await page.send('Emulation.setTouchEmulationEnabled', { enabled: false });
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector(${JSON.stringify(realRow)})`, 'return after touch navigation');
  await page.viewport(1440);
  await page.click('#create-draft');
  const late = await gate.next('posts');
  const finishedBefore = gate.finishedPosts;
  await page.click('#navigation-primary a[href="/"]');
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

// Native browser submission is a separate boundary from the hydrated router's Form handler.
async function nativeExploreSearch(page) {
  console.log('Explore native GET with page JavaScript disabled:');
  const enter = async () => {
    // A native GET replaces this execution context. Wait for its SSR result below,
    // rather than Page.key's animation-frame evaluation in the departing document.
    for (const type of ['keyDown', 'keyUp']) {
      await page.send('Input.dispatchKeyEvent', { type, key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13, ...(type === 'keyDown' ? { text: '\r' } : {}) });
    }
  };
  // A no-JS document cannot consume HydrationScripts' WASM preload. Block both
  // assets here, as in the bootstrap checks, so the next hydrated document cannot
  // wait behind a stranded same-URL preload (observed ~20s in cold debug builds).
  await page.send('Network.setBlockedURLs', { urls: [`${args.base}/pkg/vink.js*`, `${args.base}/pkg/vink_bg.wasm*`] });
  await page.send('Emulation.setScriptExecutionDisabled', { value: true });
  try {
    await page.goto(`${args.base}/?q=old&q=duplicate&unused=1`);
    check(await page.eval(`document.documentElement.dataset.hydrated !== 'true' && document.querySelector('#public-search').method === 'get' && document.querySelector('#public-query').name === 'q'`), 'unhydrated SSR retains the real GET form and named input');
    await page.eval(`window.__nativeSearchDocument = true; document.querySelector('#public-query').focus(); document.querySelector('#public-query').select()`);
    await page.send('Input.insertText', { text: 'mua tren pho cu' });
    await enter();
    await page.waitFor(`location.search === '?q=mua+tren+pho+cu' && document.querySelectorAll('.vk-public-row').length === 1 && window.__nativeSearchDocument === undefined`, 'native Enter performs a new SSR GET with one q');
    check(await page.eval(`document.documentElement.dataset.hydrated !== 'true' && JSON.stringify([...new URL(location.href).searchParams.keys()]) === '["q"]'`), 'native GET replaces duplicate/obsolete parameters without an empty submitter pair');
    await page.eval(`document.querySelector('#clear-public-search').focus()`);
    await enter();
    await page.waitFor(`!location.search && document.querySelector('#public-query')?.value === '' && document.querySelectorAll('.vk-public-row').length === 1`, 'native clear anchor navigates to / without submitting');
    check(true, 'keyboard clear navigates to / with page JavaScript still disabled');
    await page.eval(`document.querySelector('#public-query').focus()`);
    await page.send('Input.insertText', { text: 'không có tác phẩm này' });
    // The usual click helper waits for page rAF callbacks, which disabled page JS cannot run.
    const point = await page.eval(`(() => {
      const button = document.querySelector('#public-search button[type=submit]');
      button.scrollIntoView({ block: 'center' });
      const r = button.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    })()`);
    for (const type of ['mousePressed', 'mouseReleased']) {
      await page.send('Input.dispatchMouseEvent', { type, ...point, button: 'left', clickCount: 1 });
    }
    await page.waitFor(`document.querySelector('[data-testid=public-no-results]') && new URL(location.href).searchParams.get('q') === 'không có tác phẩm này'`, 'native submit button renders the SSR no-results state');
    check(true, 'native submit button returns the correct SSR no-results state');
  } finally {
    await page.send('Emulation.setScriptExecutionDisabled', { value: false });
    await page.send('Network.setBlockedURLs', { urls: [] });
  }
}

async function publicReading(page) { await discoveryReading(page, { args, check, textContrast }); }

async function editorChrome(page) {
  console.log('editor chrome: reflow, save races and recovery:');
  await page.viewport(1440);
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.querySelector('#create-draft') && document.documentElement.dataset.hydrated === 'true'`, 'desk for editor chrome');
  await page.click('#create-draft');
  await page.waitFor(EDITOR_READY, 'chrome draft');
  const workId = await page.eval(`location.pathname.split('/').pop()`);
  const title = 'Những ngày mưa ở khuỷu đường cũ — '.repeat(6).slice(0, 200);
  const poem = Array.from({ length: 8 }, (_, i) => `Khổ ${i + 1} — Sáng nay trời đổ mưa rào,\n  ngõ nhỏ ướt mềm từng viên gạch cũ.\nNgười về còn nhớ tiếng Việt không?`).join('\n\n');
  await page.eval(`window.__titleInput = document.querySelector('#draft-title'); window.__titleInput.focus()`);
  check(await page.eval(`window.__titleInput.closest('.vk-ui-field--floating') && window.__titleInput.name === 'title' && window.__titleInput.maxLength === 200 && window.__titleInput.labels[0].textContent === 'Tiêu đề'`), 'editor title consumes the shared FloatingTonal contract with the same native name, limit and label');
  await page.send('Input.imeSetComposition', { text: 'd', selectionStart: 1, selectionEnd: 1 });
  await page.send('Input.imeSetComposition', { text: 'đ', selectionStart: 1, selectionEnd: 1 });
  await page.send('Input.insertText', { text: 'đ' });
  check(await page.eval(`window.__titleInput === document.querySelector('#draft-title') && window.__titleInput.value === 'đ' && window.__titleInput.selectionStart === 1 && window.__titleInput.selectionEnd === 1`), 'title composition commits once with a stable input and caret (CDP simulation)');
  // CDP renderer key events do not invoke macOS browser-menu accelerators for native inputs.
  // Exercise the browser's real editing history through its undo/redo editing commands.
  for (const [command, modifiers, expected] of [['undo', page.mod, ''], ['redo', page.mod | SHIFT, 'đ']]) {
    await page.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'z', code: 'KeyZ', modifiers, commands: [command] });
    await page.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'z', code: 'KeyZ', modifiers });
    check(await page.eval(`window.__titleInput.value === ${JSON.stringify(expected)}`), `native title ${command} restores the correct composition history (CDP editing command)`);
  }
  await page.eval(`window.__titleInput.select()`);
  await page.send('Input.insertText', { text: title });
  check(await page.eval(`window.__titleInput === document.querySelector('#draft-title') && window.__titleInput.value.length === 200 && document.activeElement === window.__titleInput && window.__titleInput.selectionStart === 200`), 'filled 200-character title keeps native input identity, focus and horizontal caret position');
  await page.typeInBody(poem);
  await page.click('#save');
  await page.waitStatus('saved');
  const live = `window.__chromeEditor === document.querySelector('.cm-editor') && document.querySelectorAll('.cm-editor').length === 1 && window.__titleInput === document.querySelector('#draft-title') && window.__titleInput.value === ${JSON.stringify(title)}`;
  await page.eval(`window.__chromeEditor = document.querySelector('.cm-editor')`);
  const geometry = async state => {
    await page.eval(`document.activeElement?.blur()`);
    for (const width of [1440, 1024, 834, 390, 320]) {
      for (const scale of [100, 200]) {
        for (const theme of ['light', 'dark']) {
          await page.viewport(width);
          await page.eval(`document.documentElement.style.fontSize = '${scale}%'; document.documentElement.dataset.theme = '${theme}'; scrollTo(0, 0)`);
          await page.eval(`document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))`);
          // Closed native details can leave chip transitions idle with unresolved finished
          // promises. Poll current visible transitions instead of holding stale promises.
          await page.waitFor(`!document.getAnimations().some(a => a.playState === 'running' && a.effect.target?.checkVisibility() && Number.isFinite(a.effect.getComputedTiming().endTime))`, 'visible editor transitions settled');
          const scenario = `editor-${state}-${width}-${scale}-${theme}`;
          if (width === 1440 && scale === 100) await textContrast(page, theme, `editor-${state}`);
          check(await page.eval(`document.documentElement.scrollWidth <= innerWidth && ${live}`), `${scenario}: no overflow and one live EditorView`);
          check(await page.eval(`(() => {
            const toolbar = document.querySelector('#editor-toolbar'), t = toolbar.getBoundingClientRect();
            return [...toolbar.querySelectorAll('button')].every(button => {
              const r = button.getBoundingClientRect(), label = button.querySelector('.vk-ui-button__label');
              return r.width >= 48 && r.height >= 48 && r.left >= t.left && r.right <= t.right && r.top >= t.top && r.bottom <= t.bottom && (!label || label.scrollWidth <= label.clientWidth + 1);
            }) && document.querySelector('[data-testid=save-status]').textContent.trim().length > 0;
          })()`), `${scenario}: status text and all toolbar targets fit`);
          if (width <= 834 || scale === 200) check(await page.eval(`getComputedStyle(document.querySelector('#editor-toolbar')).position === 'static'`), `${scenario}: toolbar stays in document flow`);
          check(await page.eval(`(() => {
            const nodes = ['.vk-editor-paper', '.vk-editor-paper .vk-ui-field__control', '#draft-title', '[data-testid=editor-body]', '#editor-toolbar', '.vk-editor-recovery'].flatMap(s => [...document.querySelectorAll(s)]);
            const flat = nodes.every(el => {
              const s = getComputedStyle(el);
              return ['Top','Right','Bottom','Left'].every(side => parseFloat(s['border'+side+'Width']) === 0) && s.boxShadow === 'none' && s.backgroundImage === 'none' && (s.outlineStyle === 'none' || parseFloat(s.outlineWidth) === 0);
            });
            const expected = { clean:'Đã lưu', dirty:'Chưa lưu', saving:'Đang lưu…', failed:'Chưa xác nhận lưu.', conflict:'Có bản mới trên máy chủ.' };
            return flat && document.querySelector('[data-testid=save-status]').textContent === expected['${state}'] &&
              getComputedStyle(document.querySelector('[data-testid=editor-body]')).backgroundColor === 'rgba(0, 0, 0, 0)' &&
              getComputedStyle(document.querySelector('.vk-editor-paper .vk-ui-field__control')).backgroundColor === 'rgba(0, 0, 0, 0)' &&
              document.querySelectorAll('#editor-toolbar [role=status],.vk-editor-recovery [role=status],.vk-editor-recovery [role=alert]').length === 1 &&
              [...document.querySelectorAll('.vk-editor [aria-describedby]')].every(el => el.getAttribute('aria-describedby').split(/\\s+/).every(id => document.getElementById(id))) &&
              !document.querySelector('.vk-editor-paper [aria-invalid=true]') &&
              getComputedStyle(document.querySelector('#editor-body-hint')).position === 'absolute';
          })()`), `${scenario}: one uninterrupted writing sheet, flat resting chrome, complete descriptions and one live status owner`);

          await page.capture(scenario);
          if (state === 'clean') {
            await page.waitFor(TAGS_READY, 'metadata loaded');
            check(await page.eval(`(() => {
              const paper=document.querySelector('.vk-editor-paper').getBoundingClientRect(), toolbar=document.querySelector('#editor-toolbar').getBoundingClientRect(), info=document.querySelector('[data-testid=work-info]').getBoundingClientRect();
              const trigger=document.querySelector('#work-tags-edit > summary').getBoundingClientRect();
              return Math.abs(paper.left-toolbar.left)<1 && Math.abs(paper.width-toolbar.width)<1 && Math.abs(info.left-paper.left)<1 && Math.abs(info.width-paper.width)<1 && info.top>=paper.bottom && trigger.width>=48 && trigger.height>=48 && !document.querySelector('#work-tags-edit').open;
            })()`), `${scenario}: one aligned column with compact metadata after the manuscript`);
            await page.eval(`document.querySelector('#work-tags-edit > summary').focus(); window.__metadataQuery=document.querySelector('#work-tags-query')`);
            await page.key('Enter');
            check(await page.eval(`document.querySelector('#work-tags-edit').open && window.__metadataQuery===document.querySelector('#work-tags-query') && ${live} && document.documentElement.scrollWidth<=innerWidth`), `${scenario}: metadata opens in place without overflow or remount`);
            await page.key('Space');
            check(await page.eval(`!document.querySelector('#work-tags-edit').open && document.activeElement.matches('#work-tags-edit > summary')`), `${scenario}: metadata closes with visible trigger focus`);
            if (width===1440 || width<=390) {
              await page.eval(`document.querySelector('[data-testid=work-info]').scrollIntoView({block:'center'})`);
              await page.capture(`${scenario}-metadata`);
            }
          }
          if (width <= 390 && scale === 200) {
            await page.eval(`document.querySelector('#draft-title').scrollIntoView({block:'start'})`);
            await page.capture(`${scenario}-title`);
            await page.eval(`document.querySelector('[data-testid=editor-body]').scrollIntoView({block:'start'})`);
            await page.capture(`${scenario}-writing`);
          }
          if (state === 'failed' || state === 'conflict') {
            const recovery = state === 'failed' ? '[data-testid=save-failed]' : '[data-testid=conflict]';
            await page.eval(`document.querySelector('${recovery}').scrollIntoView({block:'start'})`);
            check(await page.eval(`(() => {
              const r = document.querySelector('${recovery}');
              return [...r.querySelectorAll('button')].every(b => { const t = b.getBoundingClientRect(); return t.width >= 44 && t.height >= 44 && t.left >= 0 && t.right <= innerWidth; });
            })()`), `${scenario}: persistent recovery remains reachable`);
            await page.capture(`${scenario}-recovery`);
          }
        }
      }
    }
    await page.viewport(1440);
    await page.eval(`document.documentElement.style.fontSize = ''; document.documentElement.dataset.theme = 'light'; scrollTo(0,0)`);
  };
  await geometry('clean');
  await page.eval(`document.querySelector('#editor-save-details > summary').focus()`);
  await page.key('Enter');
  check(await page.eval(`document.querySelector('#editor-save-details').open && document.querySelector('#editor-save-details').textContent.includes('phiên bản 2') && getComputedStyle(document.querySelector('#editor-toolbar')).position==='static' && ${live}`), 'expanded save detail releases sticky positioning and keeps the acknowledged version and editor');
  await page.key('Enter');
  for (const id of ['preview', 'export', 'save']) {
    await page.eval(`document.querySelector('#${id}').focus()`);
    await page.waitFor(`!document.querySelector('#${id}-tooltip').hidden && getComputedStyle(document.querySelector('#${id}-tooltip')).visibility === 'visible'`, `${id} focus tooltip`);
    check(await page.eval(`document.querySelector('#${id}').getAttribute('aria-describedby').includes('${id}-tooltip') && document.querySelector('#${id}-tooltip').getAttribute('role') === 'tooltip' && document.activeElement.id === '${id}'`), `${id} has its own native description and focus tooltip`);
    await page.key('Escape');
    check(await page.eval(`document.querySelector('#${id}-tooltip').hidden && document.activeElement.id === '${id}'`), `${id} Escape dismisses only the tooltip`);
  }
  check(await page.eval(`document.querySelector('#save-tooltip').textContent.includes(${JSON.stringify(page.mac ? '⌘ + S' : 'Ctrl + S')})`), 'save shortcut tooltip matches the browser platform');
  for (const forced of [false, true]) {
    await page.send('Emulation.setEmulatedMedia', {features:[LIGHT_SYSTEM,{name:'forced-colors',value:forced?'active':'none'}]});
    await page.eval(`document.querySelector('#draft-title').focus()`);
    check(await page.eval(`(() => { const s=getComputedStyle(document.querySelector('#draft-title').closest('.vk-ui-field__control')); return s.outlineStyle==='solid' && parseFloat(s.outlineWidth)===2; })()`), `title focus remains visible with forced-colors=${forced}`);
    await page.eval(`document.querySelector('.cm-content').focus()`);
    check(await page.eval(`(() => { const s=getComputedStyle(document.querySelector('[data-testid=editor-body]')), cm=getComputedStyle(document.querySelector('.cm-editor')); return s.outlineStyle==='solid' && parseFloat(s.outlineWidth)===2 && (cm.outlineStyle==='none'||parseFloat(cm.outlineWidth)===0); })()`), `body has one visible focus perimeter with forced-colors=${forced}`);
  }
  await page.send('Emulation.setEmulatedMedia', {features:[LIGHT_SYSTEM,{name:'forced-colors',value:'none'}]});
  await page.typeInBody('\n\nChữ mới e\u0302\u0301 — chưa lưu.');
  // Give this input its own native history group before the observer's edit/undo pair.
  await sleep(700);
  const dirtyText = await page.body();
  await page.eval(`window.__statusMutations = 0; window.__statusObserver = new MutationObserver(records => window.__statusMutations += records.length); window.__statusObserver.observe(document.querySelector('[data-testid=save-status]'), { subtree:true, childList:true, characterData:true });`);
  await page.typeInBody('x');
  await page.key('z', page.mod);
  await page.eval(`new Promise(r => requestAnimationFrame(r))`);
  check(await page.eval(`window.__statusMutations === 0`), 'typing and undo within Unsaved do not repeatedly mutate the polite status');
  check((await page.body()) === dirtyText, 'announcement probe restores the same dirty fixture after undo');
  await page.eval(`window.__statusObserver.disconnect()`);
  await geometry('dirty');
  // shellWithDirtyEditor in the main phase checks native selection and unsaved text
  // through actual Giao diện choices; its short fixture avoids DOM virtualization.
  await page.eval(`document.querySelector('.cm-content').dispatchEvent(new KeyboardEvent('keydown', {key:'s',ctrlKey:true,isComposing:true,bubbles:true,cancelable:true}))`);
  check((await page.status()) === 'unsaved', 'composing shortcut never saves a partial syllable (synthetic guard check)');
  await page.eval(`document.querySelector('.vk-editor-help summary').focus()`);
  await page.key('Enter');
  check(await page.eval(`document.querySelector('.vk-editor-help').open && ${live}`), 'keyboard opens writing help without remounting the text');
  await page.key('Enter');
  check(await page.eval(`!document.querySelector('.vk-editor-help').open`), 'keyboard closes optional writing help');
  // Native Tab order follows the visible secondary actions, Save, title and body.
  await page.eval(`document.querySelector('#preview').focus()`);
  for (const id of ['export', 'save', 'draft-title']) {
    await page.key('Tab');
    check(await page.eval(`document.activeElement.id === '${id}'`), `editor Tab reaches ${id}`);
  }
  await page.key('Tab');
  check(await page.eval(`(() => {
    const body = document.querySelector('[data-testid=editor-body]'), style = getComputedStyle(body);
    return document.activeElement.classList.contains('cm-content') &&
      parseFloat(style.borderTopWidth) === 0 &&
      style.outlineStyle === 'solid' &&
      parseFloat(style.outlineWidth) === 2 &&
      style.outlineOffset === '-2px';
  })()`), 'editor Tab reaches the text box with a borderless tonal surface and one flush 2px focus perimeter');
  const gate = new SaveGate(page);
  await gate.enable();
  await page.click('#save');
  const first = await gate.next();
  await page.waitStatus('saving');
  await page.eval(`document.querySelector('#save').click(); document.querySelector('#save').click()`);
  await page.key('s', page.mod);
  await page.typeInBody(' Gõ tiếp khi chờ.');
  await geometry('saving');
  check(gate.held.length === 0, 'double click and save shortcut cannot dispatch another pending PUT');
  await gate.release(first);
  await page.waitStatus('unsaved');
  check((await page.draft(workId)).document.blocks.at(-1).lines.at(-1) === 'Chữ mới ế — chưa lưu.', 'ack only persisted its snapshot, not newer typing');
  await page.click('#save');
  await gate.fail(await gate.next());
  await page.waitStatus('failed');
  await geometry('failed');
  check(await page.eval(`document.querySelector('[data-testid=save-failed]').textContent.includes('chưa rõ') && document.querySelector('[data-testid=save-failed] .vk-ui-notice__message').textContent === 'Tải bản sao trước khi rời trang.' && !document.querySelector('#save-failure-details').open && !document.querySelector('#save-failure-details').contains(document.querySelector('#retry-save'))`), 'unconfirmed notice keeps backup reminder and recovery actions outside optional detail');
  await page.click('#save-failure-details > summary');
  check(await page.eval(`document.querySelector('#save-failure-details').open && document.querySelector('#save-failure-details').textContent.includes('chưa có bản khôi phục tự động') && ${live}`), 'pointer opens truthful native failure detail without remounting the editor');
  await page.key('Space');
  check(await page.eval(`!document.querySelector('#save-failure-details').open && document.activeElement.matches('#save-failure-details > summary')`), 'Space closes native detail while retaining its trigger focus');
  await page.viewport(320);
  await page.eval(`document.documentElement.style.fontSize='200%';document.querySelector('#save-failure-details > summary').scrollIntoView({block:'center'})`);
  const detailPoint = await page.eval(`(() => {const r=document.querySelector('#save-failure-details > summary').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
  await page.send('Emulation.setTouchEmulationEnabled', {enabled:true});
  await page.send('Input.dispatchTouchEvent', {type:'touchStart',touchPoints:[{...detailPoint,id:1}]});
  await page.send('Input.dispatchTouchEvent', {type:'touchEnd',touchPoints:[]});
  await page.waitFor(`document.querySelector('#save-failure-details').open`, 'touch opens failure detail');
  check(await page.eval(`document.documentElement.scrollWidth<=innerWidth && ${live}`), 'native recovery detail supports touch and complete 200% text at 320px without losing the editor');
  await page.capture('editor-failure-detail-touch-320-200-light');
  await page.send('Emulation.setTouchEmulationEnabled', {enabled:false});
  await page.eval(`document.querySelector('#save-failure-details').open=false;document.documentElement.style.fontSize=''`);
  await page.viewport(1440);

  await page.click('#retry-save');
  await gate.release(await gate.next());
  await page.waitStatus('saved');
  check((await page.draft(workId)).document.blocks.at(-1).lines.at(-1).endsWith('Gõ tiếp khi chờ.'), 'deliberate retry saves the current text after reconciliation');
  await gate.disable();

  // Response stage: server has really committed. A retry must GET and enter conflict, with
  // no second PUT even when that first reconciliation itself fails (403 here).
  const responseGate = new SaveGate(page, 'Response');
  await responseGate.enable();
  await page.typeInBody(' Đã tới máy chủ.');
  await page.click('#save');
  const response = await responseGate.next();
  const committed = await page.draft(workId);
  check(committed.document.blocks.at(-1).lines.at(-1).endsWith('Đã tới máy chủ.'), 'held response belongs to a real committed save');
  await responseGate.fail(response);
  await responseGate.disable();
  await page.waitStatus('failed');
  let writes = 0, denyRead = true;
  const listener = message => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    const { requestId, request } = message.params;
    if (request.method === 'PUT') writes++;
    if (request.method === 'GET' && denyRead) {
      denyRead = false;
      page.send('Fetch.fulfillRequest', { requestId, responseCode: 403, body: Buffer.from(JSON.stringify({ error: { code: 'forbidden', message: 'fixture' } })).toString('base64'), responseHeaders: [{ name: 'content-type', value: 'application/json' }] }).catch(() => {});
    } else page.send('Fetch.continueRequest', { requestId }).catch(() => {});
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: '*/api/v1/drafts/*', requestStage: 'Request' }] });
  await page.click('#retry-save');
  await page.waitFor(`document.querySelector('[data-testid=save-failed]')?.textContent.includes('bị từ chối')`, 'failed reconciliation remains visible');
  check(writes === 0, 'failed reconciliation cannot write');
  await page.typeInBody(' Chữ còn trên trang.');
  await page.click('#retry-save');
  await page.waitStatus('conflict');
  check(writes === 0, 'retry after a failed reconciliation still reads before writing');
  await page.send('Fetch.disable');
  page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
  await geometry('conflict');
  check((await page.draft(workId)).version === committed.version, 'unknown outcome never blindly replays an already committed save');

  // Dialogs use current unsaved text and retain focus/scroll/body through close and reopen.
  for (const kind of ['preview', 'leave']) {
    await page.click(kind === 'preview' ? '#preview' : '.vk-editor-heading__back');
    const dialog = kind === 'preview' ? '#draft-preview' : '#leave-editor';
    await page.waitFor(`document.querySelector('${dialog}').open`, `${kind} dialog`);
    if (kind === 'preview') check(await page.eval(`document.querySelector('#draft-preview').textContent.includes('Chữ còn trên trang.')`), 'preview reads unsaved text during conflict');
    for (const width of [1440, 1024, 834, 390, 320]) for (const scale of [100, 200]) for (const theme of ['light', 'dark']) {
      await page.viewport(width);
      await page.eval(`document.documentElement.style.fontSize = '${scale}%'; document.documentElement.dataset.theme = '${theme}'; document.querySelector('${dialog}').scrollTop = 0`);
      await page.waitFor(`!document.getAnimations().some(a => a.playState === 'running' && a.effect.target?.checkVisibility() && Number.isFinite(a.effect.getComputedTiming().endTime))`, 'visible editor transitions settled');
      await page.capture(`editor-${kind}-${width}-${scale}-${theme}`);
      check(await page.eval(`(() => { const d = document.querySelector('${dialog}'), r = d.getBoundingClientRect(); return r.left >= 0 && r.right <= innerWidth && r.top >= 0 && r.bottom <= innerHeight && d.scrollWidth <= d.clientWidth + 1 && ${live}; })()`), `${kind} ${width}/${scale}/${theme}: dialog fits and text view survives`);
      await page.eval(`document.querySelector('${dialog} .vk-ui-dialog__title').focus()`);
      await page.key('Tab');
      check(await page.eval(`document.activeElement.id === '${kind === 'preview' ? 'preview-close' : 'leave-stay'}'`), `${kind}: Tab reaches the safe action`);
      await page.capture(`editor-${kind}-${width}-${scale}-${theme}-actions`);
      check(await page.eval(`(() => { const r = document.activeElement.getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight; })()`), `${kind} ${width}/${scale}/${theme}: keyboard reveals safe action`);
    }
    if (kind === 'leave') {
      await page.click('#leave-discard');
      await page.waitFor(`document.activeElement.id === 'discard-back'`, 'safe discard confirmation');
      await page.key('Enter');
      await page.waitFor(`document.activeElement.id === 'leave-discard'`, 'focus returns to discard trigger');
      check(await page.eval(`(() => { const r = document.activeElement.getBoundingClientRect(); return r.top >= 0 && r.bottom <= innerHeight; })()`), 'return from confirmation reveals the focused trigger');
      await page.capture('editor-discard-return-320-200-dark');
      await page.key('Tab');
      check(await page.eval(`document.activeElement === document.body || document.querySelector('#leave-editor').contains(document.activeElement)`), 'Tab cannot focus a background control after returning from confirmation');
    }
    await page.key('Escape');
    await page.waitFor(`!document.querySelector('${dialog}').open`, `${kind} closed`);
  }
  await page.viewport(1440, 360);
  await page.eval(`document.documentElement.style.fontSize=''; scrollTo(0,0)`);
  check(await page.eval(`getComputedStyle(document.querySelector('#editor-toolbar')).position === 'static'`), 'short desktop leaves writing viewport clear');
  await page.capture('editor-short-desktop-1440');
  await page.viewport(390, 420);
  await page.typeInBody('');
  await page.lineStart();
  check(await page.eval(`(() => { const r = getSelection().getRangeAt(0).getBoundingClientRect(); return getComputedStyle(document.querySelector('#editor-toolbar')).position === 'static' && r.top >= 0 && r.bottom <= innerHeight; })()`), 'reduced mobile viewport keeps the active caret visible without fixed controls');
  await page.capture('editor-short-mobile-390');
  await page.viewport(1440);
  await page.eval(`document.documentElement.dataset.theme = 'light'; document.querySelector('#draft-title').focus(); document.querySelector('#draft-title').scrollIntoView({block:'start'})`);
  check(await page.eval(`document.querySelector('#draft-title').getBoundingClientRect().top >= document.querySelector('#editor-toolbar').getBoundingClientRect().bottom`), 'sticky toolbar does not hide the focused title');
  await page.capture('editor-sticky-title-1440');
  await page.click('#keep-mine');
  await page.waitStatus('saved');
  await page.reload();
  await page.waitFor(EDITOR_READY, 'chrome editor reloaded');
  check((await page.draft(workId)).title === title && (await page.draft(workId)).document.blocks.length === 9, 'long title and poetry survive explicit recovery and reload');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('#create-draft')`, 'clean editor leaves before Forward scenario');
  await page.eval(`history.back()`);
  await page.waitFor(`location.pathname === '/ban-viet/${workId}' && ${EDITOR_READY}`, 'Back reopens the clean editor');
  await page.typeInBody(' Chữ mới trước nút Tiến.');
  await page.eval(`history.forward()`);
  await page.waitFor(`document.querySelector('#leave-editor').open`, 'Forward is held while dirty');
  check(await page.eval(`location.pathname === '/ban-viet/${workId}'`), 'Forward restores the editor URL before the router leaves');
  await page.click('#leave-stay');
  check((await page.body()).endsWith('Chữ mới trước nút Tiến.'), 'Stay after Forward keeps the local text');
  await page.click('#save');
  await page.waitStatus('saved');

  // A pre-write GET is not a PUT already in flight: discard cancels that future mutation.
  const beforeDiscard = await page.draft(workId);
  await gate.enable();
  await page.typeInBody(' Chữ bỏ khi đang đối chiếu.');
  await page.click('#save');
  await gate.fail(await gate.next());
  await page.waitStatus('failed');
  await gate.disable();
  let heldRead = null, postDiscardWrites = 0;
  const holdRead = message => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    const { requestId, request } = message.params;
    if (request.method === 'GET') heldRead = requestId;
    else {
      if (request.method === 'PUT') postDiscardWrites++;
      page.send('Fetch.continueRequest', { requestId }).catch(() => {});
    }
  };
  page.cdp.listeners.push(holdRead);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: `*/api/v1/drafts/${workId}`, requestStage: 'Request' }] });
  await page.click('#retry-save');
  const deadline = Date.now() + 10000;
  while (!heldRead && Date.now() < deadline) await sleep(50);
  check(heldRead !== null, 'reconciliation GET is pending before navigation');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('#leave-editor').open`, 'leave while reconciliation is pending');
  await page.click('#leave-save');
  await page.click('#leave-discard');
  await page.waitFor(`document.activeElement.id === 'discard-back'`, 'safe discard confirmation during reconciliation');
  await page.click('#discard-confirm');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('#create-draft')`, 'discard left the editor');
  await page.send('Fetch.continueRequest', { requestId: heldRead });
  await sleep(700);
  check(postDiscardWrites === 0 && await page.eval(`location.pathname === '/ban-viet'`), 'late reconciliation cannot start PUT or navigate after discard');
  await page.send('Fetch.disable');
  page.cdp.listeners = page.cdp.listeners.filter(item => item !== holdRead);
  check((await page.draft(workId)).version === beforeDiscard.version, 'discard before PUT leaves the server version unchanged');

  // The save can settle while the confirmation is open, removing its original trigger.
  await page.click('#create-draft');
  await page.waitFor(EDITOR_READY, 'draft for confirmation focus after ACK');
  await page.typeInBody('Chữ đang chờ xác nhận.');
  await gate.enable();
  await page.click('#save');
  const pendingAck = await gate.next();
  await page.waitStatus('saving');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('#leave-editor').open`, 'leave while save is pending');
  await page.click('#leave-discard');
  await page.waitFor(`document.activeElement.id === 'discard-back'`, 'confirmation before ACK');
  await gate.release(pendingAck);
  await page.waitStatus('saved');
  await page.key('Enter');
  await page.waitFor(`document.activeElement.id === 'leave-stay'`, 'focus falls back to Stay after ACK removes discard');
  check(await page.eval(`!document.querySelector('#leave-discard')`), 'settled save removes the discard action');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('#leave-editor').open`, 'stay after ACK');
  await gate.disable();
}

async function mainPhase(page) {
  await serverHtml();
  await conditionalPublicReads(page);
  await cacheFirstPublicReads(page);
  await expressionPilot(page, args, check);
  await discussionReadingAndWrites(page);
  await endorsementAsAuthor(page, { args, check });
  await adaptiveShell(page);
  await draftDashboard(page);
  await draftR6Hierarchy(page);
  await selectChevron(page);
  await publicReading(page);
  await editorChrome(page);

  console.log('reader in the browser:');
  await page.goto(`${args.base}/tac-pham/${SAMPLE_WORK}`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'hydration');
  check(true, 'page hydrated');
  check(await page.eval(`document.querySelector('#navigation-primary a[href="/"]').getAttribute('aria-current') === 'page'`), 'reader belongs to the Khám phá destination');
  const blocks = await page.eval(`[...document.querySelectorAll('.vk-block')].map((b) => b.textContent)`);
  check(blocks.length === 2, 'two stanzas render as two blocks');
  check(blocks[1].split('\n').length === 4 && blocks[1].includes(INDENTED_LINE), 'line breaks and indentation survive rendering');
  check(await page.eval(`[...document.querySelectorAll('.vk-discussion [role="status"]:empty')].every((el) => getComputedStyle(el).display === 'none')`), 'empty discussion status nodes collapse without adding dead grid gaps');

  console.log('self-hosted fonts:');
  const loaded = await page.eval(`document.fonts.ready.then(() => [...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family.replaceAll('"', '') + ' ' + f.weight))`);
  check(loaded.includes('Be Vietnam Pro 400') && loaded.includes('Literata 200 900'), `reader loaded the UI and reading faces (${loaded.join(', ')})`);
  const served = await page.eval(`fetch('/pkg/fonts/literata-variable.woff2').then((r) => r.status + ' ' + r.headers.get('content-type'))`);
  check(served === '200 font/woff2', 'font files are served as font/woff2');
  const editorOnReader = await page.eval(`performance.getEntriesByType('resource').some((e) => e.name.endsWith('/pkg/vink-editor.js'))`);
  check(!editorOnReader, 'the public reader does not load the editor runtime');

  console.log('client navigation and draft creation:');
  await page.eval(`window.__vinkNavigation = 'kept'`);
  await page.click('#navigation-primary a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('#create-draft')`, 'desk list');
  check(await page.eval(`window.__vinkNavigation === 'kept'`), 'navigation stayed in the hydrated app (no full reload)');
  await page.click('#create-draft');
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
  await page.click('#save');
  await page.waitStatus('saved', 'save confirmation');
  check((await page.eval(`document.querySelector('#editor-save-details').textContent`)).includes('phiên bản 2'), 'saved only after server confirmed version 2');

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
  await page.click('#save');
  await page.waitStatus('conflict', 'conflict state');
  check((await page.body()) === BODY + TAIL, 'conflict keeps the text being written');
  const serverAfterConflict = await page.draft(workId);
  check(serverAfterConflict.title === 'Tab khác' && serverAfterConflict.version === 3, 'stale save wrote nothing');
  await page.eval(`document.querySelector('.cm-editor').__appearanceConflictView = 'kept'`);
  await openProfile(page);
  await chooseTheme(page, 'dark');
  await page.click('#open-me');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'profile navigation from a conflicted editor uses the guard');
  check(await page.eval(`location.pathname === '/ban-viet/${workId}' && document.querySelector('.cm-editor').__appearanceConflictView === 'kept'`) && (await page.body()) === BODY + TAIL && (await page.status()) === 'conflict', 'appearance and guarded Tôi navigation retain the conflicted editor and text');
  await page.key('Escape');
  await page.click('#keep-mine');
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
  // R5: the shared writing sheet owns the theme fill; the body remains transparent.
  const surface = `getComputedStyle(document.querySelector('.vk-editor-paper')).backgroundColor`;
  const lightSurface = await page.eval(surface);
  await page.eval(`document.documentElement.dataset.theme = 'dark'`);
  await page.waitFor(`${surface} !== ${JSON.stringify(lightSurface)}`, 'dark theme applied');
  check(await page.eval(view), 'theme change kept the same EditorView');
  check((await page.eval(selected)) === LAST_LINE, 'theme change kept the selection');

  await page.eval(`scrollTo(0, document.documentElement.scrollHeight)`);
  const scrolled = await page.eval(`scrollY`);
  await page.eval(`document.querySelector('#preview').click()`);
  await page.waitFor(`document.querySelector('[data-testid=draft-preview]').open`, 'preview dialog');
  const preview = await page.eval(`[...document.querySelectorAll('[data-testid=draft-preview] .vk-block')].map((b) => b.textContent)`);
  check(preview.length === 3 && preview[2] === 'Khổ mới chưa lưu,\n  dòng thụt vào,\nlời cuối ở đây.', 'preview shows the unsaved stanza with its line breaks');
  check((await page.eval(`document.querySelector('[data-testid=draft-preview] .vk-preview h2').textContent`)) === TITLE, 'preview shows the current title');
  check((await page.status()) === 'unsaved' && (await page.draft(workId)).version === 4, 'preview neither saved nor marked saved');
  await page.capture('1280-preview-long-title-dark');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('[data-testid=draft-preview]').open`, 'preview closed by Escape');
  check(await page.eval(view), 'preview kept the same EditorView');
  check(scrolled > 0 && Math.abs((await page.eval(`scrollY`)) - scrolled) < 2, `closing the preview kept the page scroll (${scrolled}px)`);
  await page.eval(`document.querySelector('[data-testid=editor-body] .cm-content').focus()`);
  check((await page.eval(selected)) === LAST_LINE, 'selection is back when the text regains focus');

  await page.click('#navigation-primary a[href="/"]');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog from the Khám phá rail link');
  check((await page.eval(`location.pathname`)) === editorPath, 'Khám phá link held while text is unsaved');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog closed by Escape');
  check((await page.eval(`location.pathname`)) === editorPath, 'Escape stays in the editor');
  check(await page.eval(`document.activeElement?.classList.contains('cm-content')`), 'Escape returns focus to the text');
  check((await page.eval(selected)) === LAST_LINE && (await page.body()) === written, 'Escape keeps the text and selection');
  check(await page.eval(view), 'dialog kept the same EditorView');

  // Returning focus restores the saved selection over the next two frames. A key the editor's
  // keymap handles inside them must win: on Linux CI, End arrived ~20 ms after the dialog closed,
  // the late restore re-selected the line and the typed text replaced it. Hold animation frames
  // from that focus until the key is in, so the key lands inside the restore on every platform.
  await page.click('#navigation-primary a[href="/"]');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog again from the Khám phá rail link');
  await page.eval(`(() => {
    const raf = window.requestAnimationFrame.bind(window);
    window.__vinkFocused = new Promise((resolve) => {
      const hold = (event) => {
        if (!event.target.matches?.('[data-testid=editor-body] .cm-content')) return;
        window.removeEventListener('focus', hold, true);
        const held = [];
        window.requestAnimationFrame = (callback) => { held.push(callback); return 0; };
        window.__vinkReleaseFrames = () => {
          window.requestAnimationFrame = raf;
          held.forEach((callback) => raf(callback));
          return held.length;
        };
        resolve();
      };
      // Capture runs before CodeMirror's own focus handler schedules its restore.
      window.addEventListener('focus', hold, true);
    });
  })()`);
  await page.key('Escape', 0, { settle: false });
  await page.eval(`window.__vinkFocused`);
  await page.lineEnd({ settle: false });
  check((await page.eval(`window.__vinkReleaseFrames()`)) > 0, 'line end reached the editor while frames from the returning focus were held');
  await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(resolve))))`);
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
    await page.click('#leave-stay');
    check((await page.eval(`location.pathname`)) === editorPath && (await page.body()).endsWith(' Lui lại.'), '“Ở lại” keeps the route and the text');
    await page.eval(`history.back()`);
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog again');
    await page.eval(`document.querySelector('[data-testid=leave-editor]').dispatchEvent(new Event('close'))`);
    check(await page.eval(`document.querySelector('[data-testid=leave-editor]').open`), 'queued close from a previous dialog cannot dismiss the reopened leave intent');
    await page.click('#leave-save');
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
    await page.click('#open-me');
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog');
    check(!(await page.eval(`document.querySelector('[data-testid=leave-recovery]')`)), 'no discard offered while saving is still possible');
    await page.click('#leave-save');
    const held = await gate.next();
    await page.waitStatus('saving');
    await page.key('Escape');
    await page.waitFor(`!document.querySelector('[data-testid=leave-editor]').open`, 'stayed while saving');
    const pendingText = await page.body();
    await page.eval(`document.querySelector('.cm-editor').__appearancePendingView = 'kept'`);
    await openProfile(page);
    await chooseTheme(page, 'light');
    await page.click('#open-me');
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'profile navigation during a pending save uses the guard');
    check(await page.eval(`location.pathname === '${editorPath}' && document.querySelector('.cm-editor').__appearancePendingView === 'kept'`) && (await page.body()) === pendingText && (await page.status()) === 'saving' && gate.held.length === 0, 'appearance keeps the same pending editor protected without starting another save');
    await page.key('Escape');
    await gate.release(held);
    await page.waitStatus('saved', 'late acknowledgement');
    await sleep(700);
    check((await page.eval(`location.pathname`)) === editorPath, 'a late successful save did not navigate');

    console.log('persistent failure: export, explicit discard, late acknowledgement:');
    await page.typeInBody(' Mất kết nối.');
    await page.click('#navigation-primary a[href="/"]');
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog');
    await page.click('#leave-save');
    await gate.fail(await gate.next());
    await page.waitStatus('failed', 'failed save');
    check((await page.eval(`document.querySelector('[data-testid=leave-message]').textContent`)).includes('Chưa lưu được'), 'dialog explains the failed save');
    check((await page.eval(`location.pathname`)) === editorPath && (await page.body()).endsWith(' Mất kết nối.'), 'failure keeps the route and the text');
    await page.click('#leave-export');
    const rescue = await downloaded(downloads);
    check(rescue.text.endsWith(' Mất kết nối.\n'), `recovery export holds the unsaved text (${rescue.name})`);
    await page.click('#leave-save');
    const late = await gate.next();
    await page.click('#leave-discard');
    await page.waitFor(`document.querySelector('#discard-confirm')`, 'discard confirmation');
    await page.waitFor(`document.activeElement?.id === 'discard-back'`, 'confirmation focus after its animation frame');
    check(await page.eval(`document.activeElement?.id === 'discard-back'`), 'confirmation focuses “Quay lại”, not the loss');
    await page.click('#discard-confirm');
    await page.waitFor(`location.pathname === '/'`, 'left without saving');
    await page.click('#navigation-primary a[href="/ban-viet"]');
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

  await draftTags(page, workId);

  const kept = await page.draft(workId);
  const text = kept.document.blocks.map((block) => block.lines.join('\n')).join('\n\n');
  writeFileSync(args.state, JSON.stringify({ workId, title: kept.title, text, version: kept.version }));
}

const TAGS_READY = `document.querySelector('[data-testid=tag-status]') && document.querySelector('#work-tags-query')`;

function tagStatus(page) {
  return page.eval(`document.querySelector('[data-testid=tag-status]')?.dataset.status ?? null`);
}

function waitTagStatus(page, status, what = `tags ${status}`) {
  return page.waitFor(`document.querySelector('[data-testid=tag-status]')?.dataset.status === ${JSON.stringify(status)}`, what);
}

function draftTagsOf(page, workId) {
  return page.eval(`fetch('/api/v1/drafts/${workId}/tags').then((r) => r.json())`);
}

function chipLabels(page) {
  return page.eval(`[...document.querySelectorAll('#work-tags-themes .vk-tag-chip__label')].map((l) => l.textContent)`);
}

/** TAG-PR-05 against the labelled loopback LocalFixture and the scratch PostgreSQL catalog that
 * `verify web` seeded through `vink-app tag` (Tản văn, Thơ; Ký ức + alias Hồi ức, Gia đình, Mùa,
 * Đô thị). Tag saves are real writes; every outcome is read back from the server. */
async function draftTags(page, workId) {
  console.log('draft tags: real catalog, metadata-only leave guard, IME, save/reload, conflict, lost ACK:');
  const editorPath = `/ban-viet/${workId}`;
  // The previous step leaves another draft with unsaved text on purpose; accept only that
  // browser unload prompt to load this editor without writing the other draft.
  const acceptUnload = (message) => {
    if (message.sessionId === page.sessionId && message.method === 'Page.javascriptDialogOpening' && message.params.type === 'beforeunload') {
      page.send('Page.handleJavaScriptDialog', { accept: true }).catch(() => {});
    }
  };
  page.cdp.listeners.push(acceptUnload);
  await page.goto(`${args.base}${editorPath}`);
  page.cdp.listeners = page.cdp.listeners.filter((item) => item !== acceptUnload);
  await page.waitFor(`${EDITOR_READY} && ${TAGS_READY} && document.documentElement.dataset.hydrated === 'true'`, 'editor with tags');
  const textVersion = (await page.draft(workId)).version;
  check((await tagStatus(page)) === 'saved' && (await page.status()) === 'saved', 'a draft without tags starts saved');
  const html = await (await fetch(`${args.base}${editorPath}`)).text();
  check(!html.includes('Thông tin tác phẩm') && !html.includes('work-tags'), 'SSR never renders the private tag section or its data');
  await page.waitFor(`document.querySelectorAll('#work-tags-form .vk-ui-chip').length === 2`, 'form choices from the real catalog');
  await page.eval(`document.querySelector('.cm-editor').__vinkTagsView = 'kept'`);
  const view = `document.querySelector('.cm-editor')?.__vinkTagsView === 'kept' && document.querySelectorAll('.cm-editor').length === 1`;
  const bodyBefore = await page.body();
  check(await page.eval(`!document.querySelector('#work-tags-edit').open && document.querySelector('[data-testid=tag-selection-summary]').textContent.includes('không có thẻ')`), 'metadata starts compact with its current selection visible');
  await page.eval(`window.__tagQuery = document.querySelector('#work-tags-query'); document.querySelector('#work-tags-edit > summary').focus()`);
  await page.key('Enter');
  check(await page.eval(`document.querySelector('#work-tags-edit').open && window.__tagQuery===document.querySelector('#work-tags-query') && ${view}`), 'Enter opens the existing TagSelector in place');
  await page.key('Space');
  check(await page.eval(`!document.querySelector('#work-tags-edit').open && document.activeElement.matches('#work-tags-edit > summary') && ${view}`), 'Space closes metadata without losing focus, selector or EditorView');
  await page.key('Enter');

  // A metadata-only change is unsaved work: the toolbar says so and leaving is held.
  await page.click('#work-tags-form .vk-ui-chip');
  await waitTagStatus(page, 'unsaved');
  check((await page.status()) === 'unsaved', 'the toolbar status covers a tag-only change');
  await page.click('#work-tags-edit > summary');
  check(await page.eval(`!document.querySelector('#work-tags-edit').open && document.querySelector('[data-testid=tag-selection-summary]').textContent.includes('Tản văn')`), 'closing metadata keeps the unsaved selection visible');
  await page.click('#navigation-primary a[href="/"]');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog for a tag-only change');
  check((await page.eval(`document.querySelector('[data-testid=leave-message]').textContent`)).includes('Thẻ đã chọn'), 'the dialog names the unsaved tags');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('[data-testid=leave-editor]').open`, 'stayed');
  check((await page.eval(`location.pathname`)) === editorPath, 'a tag-only change held the navigation');
  check(await page.eval(`!document.querySelector('#work-tags-edit').open && document.activeElement.matches('#work-tags-edit > summary')`), 'staying returns focus to the visible closed metadata trigger');
  await page.key('Enter');

  // IME: Enter while composing neither chooses a theme nor submits the editor form.
  let puts = 0;
  const countPuts = (message) => {
    if (message.sessionId === page.sessionId && message.method === 'Network.requestWillBeSent' && message.params.request.method === 'PUT') puts++;
  };
  page.cdp.listeners.push(countPuts);
  await page.eval(`document.getElementById('work-tags-query').focus()`);
  await page.send('Input.imeSetComposition', { text: 'hồ', selectionStart: 2, selectionEnd: 2 });
  await page.key('Enter');
  check(puts === 0 && (await chipLabels(page)).length === 0, 'Enter during composition chose nothing and saved nothing');
  await page.send('Input.insertText', { text: 'hồi' });
  await page.waitFor(`document.getElementById('work-tags-status').textContent.includes('gợi ý cho “hồi”')`, 'suggestions for the committed query');
  const alias = await page.eval(`[...document.querySelectorAll('#work-tags-options .vk-ui-button')].find((b) => b.textContent.includes('Ký ức'))?.textContent ?? ''`);
  check(alias.includes('qua tên “Hồi ức”'), `the alias that matched is named (${alias})`);
  await page.eval(`[...document.querySelectorAll('#work-tags-options .vk-ui-button')].find((b) => b.textContent.includes('Ký ức')).id = 'tag-pick-ky-uc'`);
  await page.click('#tag-pick-ky-uc');
  await page.waitFor(`document.querySelector('#work-tags-themes .vk-tag-chip__label')?.textContent === 'Ký ức'`, 'theme chosen from a suggestion');
  await page.waitFor(`document.getElementById('work-tags-query').value === '' && document.activeElement.id === 'work-tags-query'`, 'the field clears and keeps focus after choosing');
  check(true, 'the field clears and keeps focus after choosing');
  await page.send('Input.insertText', { text: 'Đô thị' });
  await page.waitFor(`document.getElementById('work-tags-status').textContent.includes('“Đô thị”')`, 'exact suggestion');
  await page.key('Enter');
  await page.waitFor(`document.querySelectorAll('#work-tags-themes .vk-tag-chip').length === 2`, 'Enter chose the exact match');
  check(puts === 0, 'choosing tags does not save by itself');
  check(await page.eval(view) && (await page.body()) === bodyBefore, 'tag edits kept the same EditorView and text');

  await page.key('s', page.mod);
  await waitTagStatus(page, 'saved', 'tags saved by Ctrl/Cmd+S');
  await page.waitStatus('saved', 'toolbar saved');
  page.cdp.listeners = page.cdp.listeners.filter((item) => item !== countPuts);
  check(puts === 1, `only the tags were sent (${puts} PUT)`);
  let stored = await draftTagsOf(page, workId);
  check(stored.version === 2 && stored.form?.label === 'Tản văn' && stored.themes.map((t) => t.label).join() === 'Ký ức,Đô thị', `server stored the exact selection (v${stored.version})`);
  check((await page.draft(workId)).version === textVersion, 'a tag save does not change the text version');
  const publication = await fetch(`${args.base}/api/v1/works/${workId}/publication`);
  check(publication.status === 404, 'saving tags published nothing');

  await page.eval(`window.__vinkBeforeReload = true`);
  await page.send('Page.reload');
  await page.waitFor(`window.__vinkBeforeReload === undefined && ${EDITOR_READY} && ${TAGS_READY} && document.documentElement.dataset.hydrated === 'true'`, 'editor after reload');
  await page.waitFor(`document.querySelectorAll('#work-tags-themes .vk-tag-chip').length === 2`, 'tags restored');
  check((await chipLabels(page)).join() === 'Ký ức,Đô thị' && await page.eval(`(() => { const pressed = [...document.querySelectorAll('#work-tags-form .vk-ui-chip[aria-pressed=true]')]; return pressed.length === 1 && pressed[0].textContent.includes('Tản văn'); })()`), 'reload shows the confirmed tags');

  check(await page.eval(`!document.querySelector('#work-tags-edit').open && document.querySelector('[data-testid=tag-selection-summary]').textContent.includes('Tản văn, Ký ức, Đô thị')`), 'reloaded metadata shows acknowledged names while compact');
  await page.click('#work-tags-edit > summary');

  // Another tab saves first: the stale save writes nothing; the author chooses.
  const mua = (await page.eval(`fetch('/api/v1/tags?facet=theme').then((r) => r.json())`)).tags.find((t) => t.label === 'Mùa');
  const other = await page.eval(`fetch('/api/v1/drafts/${workId}/tags', { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ expected_version: 2, form: null, themes: [${JSON.stringify(mua.tag_id)}] }) }).then((r) => r.status)`);
  check(other === 200, 'another tab saved tags version 3');
  await page.click('#work-tags-theme-0-remove');
  await waitTagStatus(page, 'unsaved');
  await page.click('#save');
  await waitTagStatus(page, 'conflict', 'tag conflict');
  check((await page.eval(`document.querySelector('[data-testid=tag-conflict]').textContent`)).includes('Đang lưu: Mùa'), 'the conflict shows the stored tags by name');
  stored = await draftTagsOf(page, workId);
  check(stored.version === 3 && stored.themes.map((t) => t.label).join() === 'Mùa', 'the stale tag save wrote nothing');
  check((await chipLabels(page)).join() === 'Đô thị', 'the conflict keeps the author\'s selection on the page');
  await page.click('#work-tags-edit > summary');
  check(await page.eval(`!document.querySelector('#work-tags-edit').open && !document.querySelector('#work-tags-edit').contains(document.querySelector('[data-testid=tag-conflict]')) && document.querySelector('#tags-keep-mine').getClientRects().length>0`), 'tag conflict and deliberate recovery stay visible outside the closed disclosure');
  await page.click('#tags-keep-mine');
  await waitTagStatus(page, 'saved', 'explicit tag overwrite');
  await page.click('#work-tags-edit > summary');
  stored = await draftTagsOf(page, workId);
  check(stored.version === 4 && stored.form?.label === 'Tản văn' && stored.themes.map((t) => t.label).join() === 'Đô thị', 'the author chose their selection: version 4');

  // While the acknowledgement is held, undo the selection that the server already committed.
  const responses = new SaveGate(page, 'Response');
  await responses.enable();
  await page.eval(`document.getElementById('work-tags-query').focus()`);
  await page.send('Input.insertText', { text: 'Gia đình' });
  await page.waitFor(`document.getElementById('work-tags-status').textContent.includes('“Gia đình”')`, 'exact suggestion');
  await page.key('Enter');
  await page.waitFor(`document.querySelectorAll('#work-tags-themes .vk-tag-chip').length === 2`, 'second theme');
  await page.click('#save');
  const lost = await responses.next();
  await page.click('#work-tags-theme-1-remove');
  check((await chipLabels(page)).join() === 'Đô thị', 'the author restored the old selection while saving');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]')?.open`, 'the pending tag save holds navigation even after restoring the old selection');
  await page.click('#leave-save');
  await responses.fail(lost);
  await responses.disable();
  await waitTagStatus(page, 'failed', 'unconfirmed tag save');
  check((await page.eval(`document.querySelector('[data-testid=save-status]').textContent`)) === 'Chưa xác nhận lưu thẻ.', 'the toolbar says the tag save is unconfirmed, not saved or failed outright');
  stored = await draftTagsOf(page, workId);
  check(stored.version === 5, 'the dropped answer belonged to a committed save');
  check((await page.eval(`location.pathname`)) === editorPath, 'a lost tag acknowledgement keeps the author in the editor');
  await page.click('#leave-stay');
  await page.click('#tags-retry');
  await waitTagStatus(page, 'unsaved', 'reconciled version with a newer local selection');
  stored = await draftTagsOf(page, workId);
  check(stored.version === 5 && stored.themes.map((t) => t.label).join() === 'Đô thị,Gia đình', 'the retry reconciled through 409 without writing again');
  check((await chipLabels(page)).join() === 'Đô thị' && (await page.draft(workId)).version === textVersion, 'reconciliation keeps the newer selection and leaves the text untouched');

  // Text and tags together: leaving waits for both confirmations.
  await page.typeInBody(' Cùng thẻ.');
  await waitTagStatus(page, 'unsaved');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog for text and tags');
  check((await page.eval(`document.querySelector('[data-testid=leave-message]').textContent`)).includes('Chữ và thẻ'), 'the dialog names both parts');
  await page.click('#leave-save');
  await page.waitFor(`location.pathname === '/ban-viet'`, 'left after both saves');
  stored = await draftTagsOf(page, workId);
  const text = await page.draft(workId);
  check(stored.version === 6 && stored.themes.map((t) => t.label).join() === 'Đô thị' && text.version === textVersion + 1 && text.document.blocks.at(-1).lines.at(-1).endsWith(' Cùng thẻ.'), 'left only after the text and the tags were both confirmed');
}

/** After `verify web` retired Đô thị (successor Mùa) between the phases: the tag stays on the
 * draft, is written as retired and is replaced in place only by the author's action. */
async function retiredTagAfterRestart(page, workId) {
  console.log('a tag retired by the curator stays on the draft until the author replaces it:');
  await page.waitFor(`${TAGS_READY} && document.querySelector('#work-tags-theme-0')`, 'tags after restart');
  check(await page.eval(`(() => { const chip = document.getElementById('work-tags-theme-0'); return chip.dataset.status === 'retired' && chip.textContent.includes('Đô thị') && chip.textContent.includes('Đã ngừng dùng'); })()`), 'the retired tag is kept and written as retired');
  check((await tagStatus(page)) === 'saved' && (await page.status()) === 'saved', 'a kept retired tag is not an unsaved change');
  await page.click('#work-tags-edit > summary');
  await page.click('#work-tags-theme-0-replace');
  await waitTagStatus(page, 'unsaved');
  check((await chipLabels(page)).join() === 'Mùa', 'the curator\'s successor replaced it in place');
  await page.click('#save');
  await waitTagStatus(page, 'saved', 'replacement saved');
  const stored = await draftTagsOf(page, workId);
  check(stored.themes.map((t) => `${t.label}:${t.status}`).join() === 'Mùa:active', 'the server holds the successor');
}

/** Real local fixture identity + scratch PostgreSQL writes. Rendering fixtures used below
 * have a separate label and never substitute for persistence/auth/moderation evidence. */
async function discussionReadingAndWrites(page) {
  console.log('D2T-03: anonymous SSR, local discussion writes and exact keyed reconciliation:');
  const endpoint = `${args.base}/api/v1/works/${SAMPLE_WORK}/discussion`;
  const wirePattern = `${args.base}/api/v1/works/*/discussion`;
  await page.goto(`${args.base}/tac-pham/${SAMPLE_WORK}`);
  await page.waitFor(`document.documentElement.dataset.hydrated==='true' && document.querySelector('#discussion-mode-toggle') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'local fixture discussion readiness');
  check(await page.eval(`!!document.querySelector('meta[name="vink-local-discussion-fixture"]') && document.querySelector('#comments-mode').textContent.includes('Định danh thử trên thiết bị này')`), 'fixture readiness is labelled and supplied by the trusted host');
  const text = 'D2T-03 [Mẫu] Đọc khổ thơ\n\n  Tiếng Việt 👩‍💻 🌧️';
  const newer = ' Chữ mới sau khi gửi.';
  await page.typeAtEnd('#root-composer', text);
  const held = [];
  const waitHeld = async count => {
    const deadline = Date.now() + 10_000;
    while (held.length < count) {
      if (Date.now() > deadline) throw new Error(`timed out waiting for discussion response ${count}`);
      await sleep(50);
    }
  };
  const listener = message => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    if (decodeURIComponent(new URL(message.params.request.url).pathname) !== new URL(`${endpoint}/comments`).pathname) {
      page.send('Fetch.continueRequest', { requestId: message.params.requestId }).catch(() => {});
      return;
    }
    if (message.params.request.method === 'POST') held.push(message.params);
    else page.send('Fetch.continueRequest', { requestId: message.params.requestId }).catch(() => {});
  };
  page.cdp.listeners.push(listener);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: `${wirePattern}/comments`, requestStage: 'Response' }] });
  let commentId;
  try {
    await page.click('#root-composer-send');
    await page.waitFor('document.querySelector("#root-composer-send")?.getAttribute("aria-disabled")==="true"', 'pending send blocks a second activation');
    await waitHeld(1);
    check(held[0].responseStatusCode === 201, 'the real server committed a new pending comment before the response is lost');
    await page.typeAtEnd('#root-composer', newer);
    await page.click('#root-composer-send');
    check(held.length === 1, 'double activation starts only one command');
    await page.send('Fetch.failRequest', { requestId: held[0].requestId, errorReason: 'ConnectionClosed' });
    await page.waitFor(`document.querySelector('#feedback-region').textContent.includes('Kiểm tra lần gửi') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'lost ACK is uncertain, never falsely failed or public');
    await page.send('Emulation.setEmulatedMedia', {features:[{name:'prefers-reduced-motion',value:'reduce'}]});
    for (const [width, scale] of [[390,100],[320,200]]) {
      await page.viewport(width);
      await page.eval(`(async () => {
        document.documentElement.dataset.theme='dark'; document.documentElement.style.fontSize='${scale}%';
        await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
        const recovery=document.querySelector('.vk-discussion-delivery .vk-ui-button--filled');
        recovery.focus({preventScroll:true}); recovery.scrollIntoView({block:'center',behavior:'instant'});
        await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
        recovery.scrollIntoView({block:'center',behavior:'instant'});
      })()`);
      await page.waitFor(`getComputedStyle(document.querySelector('.vk-discussion-delivery p')).color===getComputedStyle(document.body).color`, 'recovery text resolves the final theme foreground');
      check(await page.eval(`(() => {
        const region=document.querySelector('#root-composer').closest('.vk-discussion-composer');
        const delivery=region.querySelector('.vk-discussion-delivery'), send=document.querySelector('#root-composer-send');
        return getComputedStyle(delivery).borderTopLeftRadius==='20px' &&
          [...region.querySelectorAll('.vk-ui-button--filled')].filter(b=>b.getAttribute('aria-disabled')!=='true').length===1 &&
          send.getAttribute('aria-disabled')==='true' && !!document.getElementById(send.getAttribute('aria-describedby')) &&
          region.textContent.split('Chưa rõ kết quả gửi').length===2 && document.documentElement.scrollWidth<=innerWidth;
      })()`), `uncertain/${width}/${scale}: one enabled primary recovery, disabled described Send and separate 20px status`);
      await page.capture(`discussion-local-uncertain-${width}-${scale}-dark`, {boundary:'real-local-postgresql-with-lost-response'});
    }
    await page.click('.vk-discussion-delivery .vk-ui-button--filled');
    check(true, '320/text200/reduced motion: the real recovery target is reachable above bottom navigation');
    await waitHeld(2);
    await page.eval(`document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme`);
    await page.viewport(1280);
    await page.send('Emulation.setEmulatedMedia', {features:[LIGHT_SYSTEM]});
    const first = JSON.parse(held[0].request.postData), replay = JSON.parse(held[1].request.postData);
    check(JSON.stringify(first) === JSON.stringify(replay) && held[1].responseStatusCode === 200, 'reconciliation replays exactly the same key/body/target; server returns the original receipt');
    await page.send('Fetch.continueRequest', { requestId: held[1].requestId });
    await page.waitFor(`document.querySelector('[data-testid=discussion-receipt]')?.textContent.includes('Chờ duyệt') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'accepted-for-review receipt');
    check(await page.eval(`document.querySelector('#root-composer').value===${JSON.stringify(text + newer)}`), 'late success keeps all newly typed text');
    const own = await (await fetch(endpoint)).json();
    const matches = own.roots.filter(root => root.root.kind === 'comment' && root.root.comment.body === text.trim());
    check(matches.length === 1 && matches[0].root.comment.visibility === 'own_pending', 'keyed replay persisted exactly one own-pending comment in real PostgreSQL');
    commentId = matches[0].root.comment.comment_id;
    const publicHtml = await (await fetch(`${args.base}/tac-pham/${SAMPLE_WORK}`)).text();
    check(!publicHtml.includes('D2T-03 [Mẫu] Đọc khổ thơ') && !publicHtml.includes('own_pending') && !publicHtml.includes('own_rejected'), 'public SSR and hydration payload exclude fixture viewer-private comments');
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
  }
  await page.click('#discussion-mode-toggle');
  await page.waitFor(`document.querySelector('#discussion-mode-toggle')?.textContent==='Mở bình luận' && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'author closes comments with generation CAS');
  check(await page.eval(`document.querySelector('#root-composer').value===${JSON.stringify(text + newer)} && document.querySelector('#root-composer-send').getAttribute('aria-disabled')==='true' && !!document.querySelector('[data-own-comment="${commentId}"]')`), 'closing preserves draft and readable own projection while blocking new sends');
  const closed = await (await fetch(endpoint)).json();
  check(!closed.settings.comments_open && closed.settings.generation > 1, 'closed mode is a durable provider fact');
  await captureDiscussionMatrix(page, 'local-postgres-closed', '#feedback-heading', 'real-local-postgresql-with-fixture-identity');
  await page.click('#discussion-mode-toggle');
  await page.waitFor(`document.querySelector('#discussion-mode-toggle')?.textContent==='Tắt bình luận' && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'author reopens the same thread');
  await page.click(`#remove-comment-${commentId}`);
  await page.waitFor(`!document.querySelector('[data-own-comment="${commentId}"] .vk-discussion-own-body')?.textContent && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'own removal refreshes the authorized projection');

  console.log('D2T-03: labelled provider fixture projection for flat contexts and responsive states:');
  const fixture = JSON.parse(readFileSync(new URL('../../../services/vink-app/contracts/http/v1/fixtures/discussion.public.get.json', import.meta.url), 'utf8'));
  fixture.work_id = SAMPLE_WORK;
  const rootId = fixture.roots[0].root.comment.comment_id;
  const replyId = fixture.roots[0].preview[0].comment_id;
  let currentFixture = structuredClone(fixture);
  let replyRequest;
  let failRead = true;
  let failNextPage = false;
  const readCursors = [];
  const nextRootId = '0192f3a1-0000-7000-8000-000000000098';
  const nextRoot = structuredClone(fixture.roots[0]);
  nextRoot.root.comment.comment_id = nextRootId;
  nextRoot.root.comment.body = '[Mẫu] Nhận xét ở trang kế.';
  nextRoot.preview = [];
  nextRoot.public_replies = 0;
  nextRoot.preview_next_cursor = null;
  const fixtureListener = message => {
    if (message.sessionId !== page.sessionId || message.method !== 'Fetch.requestPaused') return;
    const path = decodeURIComponent(new URL(message.params.request.url).pathname);
    if (message.params.request.method === 'POST' && path === new URL(`${endpoint}/comments`).pathname) { replyRequest = message.params.requestId; return; }
    if (message.params.request.method !== 'GET' || path !== new URL(`${endpoint}/public`).pathname) {
      page.send('Fetch.continueRequest', { requestId: message.params.requestId }).catch(() => {});
      return;
    }
    const cursor = new URL(message.params.request.url).searchParams.get('cursor');
    readCursors.push(cursor);
    if (failRead || (failNextPage && cursor !== null)) {
      page.send('Fetch.fulfillRequest', { requestId: message.params.requestId, responseCode: 503,
        responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
        body: Buffer.from(JSON.stringify({ error: { code: 'unavailable', message: 'fixture outage' } })).toString('base64'),
      }).catch(() => {});
      return;
    }
    page.send('Fetch.fulfillRequest', { requestId: message.params.requestId, responseCode: 200,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify(cursor === null ? currentFixture : { ...currentFixture, roots: [nextRoot], next_cursor: null })).toString('base64'),
    }).catch(() => {});
  };
  page.cdp.listeners.push(fixtureListener);
  await page.send('Fetch.enable', { patterns: [{ urlPattern: `${wirePattern}*`, requestStage: 'Request' }] });
  try {
    // Client navigation exercises the initial resource error without changing SSR or the DB.
    await page.goto(`${args.base}/`);
    await page.waitFor(`document.documentElement.dataset.hydrated==='true' && document.querySelector('a[href="/tac-pham/${SAMPLE_WORK}"]')`, 'reader link for initial-read failure');
    await page.click(`a[href="/tac-pham/${SAMPLE_WORK}"]`);
    await page.waitFor(`document.querySelector('#discussion-load-retry')`, 'initial discussion read failed');
    const failedInitialReads = readCursors.length;
    check(failedInitialReads === 1, 'one failed initial GET exposes the retry button');
    await page.capture('discussion-contract-fixture-initial-error', { scenario: 'D2T-03-load-retry', boundary: 'intercepted-provider-contract-fixture-no-persistence' });
    failRead = false;
    await page.click('#discussion-load-retry');
    await page.waitFor(`document.querySelector('#reply-to-${replyId}') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'initial retry completes with contract fixture projection');
    check(readCursors.length > failedInitialReads && !await page.eval(`!!document.querySelector('#discussion-load-retry')`), 'initial retry issues a new GET and replaces the error with loaded comments');
    await page.click(`#reply-to-${replyId}`);
    await page.waitFor(`document.activeElement?.id==='reply-composer-${rootId}'`, 'reply moves focus to its own stable field');
    await page.typeAtEnd(`#reply-composer-${rootId}`, 'Bản trả lời có ngữ cảnh\n\n  Giữ nguyên 👩‍💻');
    check(await page.eval(`document.querySelectorAll('.vk-discussion-replies .vk-discussion-replies').length===0 && document.querySelectorAll('#reply-composer-${rootId}').length===1`), 'reply-to-reply mounts one composer in its root and no third indentation level');
    await captureDiscussionMatrix(page, 'contract-fixture-flat', `#comment-${rootId}`, 'intercepted-provider-contract-fixture-no-persistence');
    await page.click(`#reply-composer-${rootId}-send`);
    const deadline = Date.now() + 10_000;
    while (!replyRequest) {
      if (Date.now() > deadline) throw new Error('timed out waiting for held fixture reply intent');
      await sleep(50);
    }
    await page.typeAtEnd(`#reply-composer-${rootId}`, ' Gõ tiếp khi đang gửi.');
    await page.eval(`[...document.querySelectorAll('.vk-discussion-reply-composer button')].find(b=>b.textContent==='Thu ô trả lời · Giữ chữ').click()`);
    await page.waitFor(`!document.querySelector('#reply-composer-${rootId}') && document.activeElement?.id==='reply-to-${replyId}'`, 'reply composer collapsed and restored its trigger focus');
    await page.click(`#reply-to-${replyId}`);
    await page.waitFor(`document.querySelector('#reply-composer-${rootId}-send')?.dataset.state==='pending'`, 'feature-owned transport survives composer collapse');
    await page.send('Fetch.fulfillRequest', { requestId: replyRequest, responseCode: 201,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      body: Buffer.from(JSON.stringify({ comment_id: '0192f3a1-0000-7000-8000-000000000099', status: 'pending', replayed: false })).toString('base64'),
    });
    await page.waitFor(`document.querySelector('#reply-composer-${rootId}')?.value.includes('Gõ tiếp khi đang gửi.') && document.querySelector('#reply-composer-${rootId}-send')?.dataset.state!=='pending'`, 'held fixture reply acknowledgment completes after collapse without losing newer text');
    check(true, 'intercepted fixture only: collapse/reopen does not cancel the feature transport or strand Sending');
    currentFixture = structuredClone(fixture);
    currentFixture.roots[0].preview = currentFixture.roots[0].preview.filter(comment => comment.comment_id !== replyId);
    currentFixture.roots[0].preview[0].reply_to = { kind: 'unavailable' };
    await page.click('#discussion-refresh');
    await page.waitFor(`!document.querySelector('#comment-${replyId}') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'context revocation fixture refresh');
    check(await page.eval(`document.querySelector('#reply-composer-${rootId}')?.value.includes('Giữ nguyên') && document.querySelector('#reply-composer-${rootId}-send')?.getAttribute('aria-disabled')==='true' && document.querySelectorAll('.vk-discussion-replies .vk-discussion-comment').length===3`), 'revoked context keeps peer replies and draft, neutralizes cached target identity and blocks stale send');
    await page.eval(`document.querySelector('.vk-discussion-reply-composer').scrollIntoView({ block: 'center', behavior: 'instant' })`);
    await page.capture('discussion-contract-fixture-context-revoked', { scenario: 'D2T-03-context', boundary: 'intercepted-provider-contract-fixture-no-persistence' });
    failRead = true;
    await page.click('#discussion-refresh');
    await page.waitFor(`document.querySelector('#comments-mode').textContent.includes('Chưa làm mới được') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'refresh failure uses read-specific copy');
    check(await page.eval(`!document.querySelector('#comments-mode').textContent.includes('chưa rõ kết quả') && !!document.querySelector('#comment-${rootId}') && document.querySelector('#reply-composer-${rootId}')?.value.includes('Giữ nguyên')`), 'failed refresh preserves loaded comments and draft without implying an unknown write');
    failRead = false;
    currentFixture = structuredClone(fixture);
    currentFixture.next_cursor = 'next-root-cursor';
    await page.click('#discussion-refresh');
    await page.waitFor(`document.querySelector('#root-pagination button') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'root pagination control');
    failNextPage = true;
    const beforePaging = readCursors.length;
    await page.click('#root-pagination button');
    await page.waitFor(`document.querySelector('#root-pagination [role=status]')?.textContent.includes('Chưa tải thêm được')`, 'next-page failure is reported at its control');
    check(JSON.stringify(readCursors.slice(beforePaging)) === JSON.stringify(['next-root-cursor']), 'the failed cursor GET is intercepted as 503 rather than reaching the real server with a fixture cursor');
    check(await page.eval(`!document.querySelector('#comments-mode').textContent.includes('Chưa tải thêm được') && !document.querySelector('#comments-mode').textContent.includes('Chưa rõ kết quả') && document.querySelectorAll('.vk-discussion-comment').length>=3`), 'next-page failure stays beside its control, keeps loaded comments and does not claim an unknown write');
    await page.eval(`document.querySelector('#root-pagination').scrollIntoView({ block: 'center', behavior: 'instant' })`);
    await page.capture('discussion-contract-fixture-next-page-error', { scenario: 'D2T-03-paging', boundary: 'intercepted-provider-contract-fixture-no-persistence' });
    failNextPage = false;
    await page.click('#root-pagination button');
    await page.waitFor(`document.querySelector('#comment-${nextRootId}') && !document.querySelector('#root-pagination') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'retry appends the returned page and reaches the authoritative end');
    check(JSON.stringify(readCursors.slice(beforePaging)) === JSON.stringify(['next-root-cursor', 'next-root-cursor']), 'failed page and its retry each issue one GET with the same cursor');
    check(await page.eval(`!!document.querySelector('#comment-${rootId}') && document.querySelector('#reply-composer-${rootId}')?.value.includes('Giữ nguyên')`), 'successful paging keeps prior comments and draft');
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== fixtureListener);
    await page.eval(`document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme`);
    await page.viewport(1280);
  }
}

async function discussionR31Geometry(page, width, scale, scenario) {
  const geometry = await page.eval(`(() => {
    const thread = document.querySelector('.vk-discussion-root'), pattern = thread.querySelector('.vk-discussion-thread');
    const root = pattern.querySelector(':scope > .vk-discussion-comment'), group = pattern.querySelector('.vk-discussion-reply-group');
    const composer = thread.querySelector('.vk-feedback-composer'), body = root.querySelector('.vk-discussion-body');
    const rect = e => { const r=e.getBoundingClientRect(); return {left:r.left, right:r.right}; };
    const controls = [...pattern.querySelectorAll('.vk-ui-toggle__control')];
    const luminance = color => {
      const rgb=color.match(/[0-9.]+/g).slice(0,3).map(Number).map(v=>v/255).map(v=>v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4);
      return rgb[0]*0.2126+rgb[1]*0.7152+rgb[2]*0.0722;
    };
    const ratio=(a,b)=>{const x=luminance(a),y=luminance(b);return (Math.max(x,y)+0.05)/(Math.min(x,y)+0.05);};
    const surface=getComputedStyle(group).backgroundColor, tonal=getComputedStyle(composer).backgroundColor;
    const ratios=[ratio(getComputedStyle(group.querySelector('.vk-discussion-body')).color,surface), ratio(getComputedStyle(group.querySelector('time')).color,surface), ratio(getComputedStyle(composer.querySelector('label')).color,tonal)];
    const reasons = [...pattern.querySelectorAll('.vk-discussion-shared-reason')].filter(e=>!e.hidden);
    return {root:rect(root), group:rect(group), composer:rect(composer),
      inset:rect(body).left-rect(root).left, insetRight:rect(root).right-rect(body).right,
      aligned: Math.abs(rect(root.querySelector('.vk-discussion-name')).left-rect(body).left)<=1 && Math.abs(rect(root.querySelector('.vk-discussion-actions .vk-ui-button__label')).left-rect(body).left)<=1,
      composerInset:rect(composer.querySelector('textarea')).left+parseFloat(getComputedStyle(composer.querySelector('textarea')).paddingLeft)-rect(composer).left,
      gap:parseFloat(getComputedStyle(group.querySelector('ul')).rowGap),
      contrast:Math.min(...ratios),
      reasons:reasons.length, targetReasons:pattern.querySelectorAll('.vk-ui-action__note').length,
      descriptions:controls.every(b=>(b.getAttribute('aria-describedby')||'').split(' ').filter(Boolean).every(id=>!!document.getElementById(id))),
      shared:controls.length>=2 && controls.every(b=>(b.getAttribute('aria-describedby')||'').includes(reasons[0]?.id)),
      field:composer.querySelectorAll('textarea').length===1 && !!composer.querySelector('label[for]'),
      radius:getComputedStyle(composer).borderTopLeftRadius,
      buttons:[...thread.querySelectorAll('button')].every(b=>{const r=b.getBoundingClientRect();return r.width>=48 && r.height>=48 && r.left>=0 && r.right<=innerWidth+1;})};
  })()`);
  const close = (a,b)=>Math.abs(a-b)<=1;
  check(['group','composer'].every(k=>close(geometry.root.left,geometry[k].left)&&close(geometry.root.right,geometry[k].right)) && geometry.aligned, `${scenario}: root/group/composer share both edges and text/action axis (${JSON.stringify(geometry)})`);
  check(close(geometry.inset, width < 360 ? 12 : 16) && close(geometry.inset,geometry.insetRight) && close(geometry.composerInset,geometry.inset) && geometry.gap===16, `${scenario}: balanced fixed inset and 16px reply spacing`);
  check(geometry.reasons===1 && geometry.targetReasons===0 && geometry.descriptions && geometry.shared, `${scenario}: one uniform branch reason describes every separate target`);
  check(geometry.field && geometry.radius==='28px' && geometry.buttons, `${scenario}: one native field, 28px tonal surface and reachable 48px targets`);
  check(geometry.contrast>=4.5, `${scenario}: resolved body/metadata/label contrast >=4.5 (${geometry.contrast.toFixed(2)})`);
  if (width === 390 || width === 1440 || (width === 320 && scale === 200)) {
    const scroll = await page.eval('({x:scrollX,y:scrollY})');
    await page.eval(`document.querySelector('.vk-discussion-reply-composer').scrollIntoView({block:'start'})`);
    await page.capture(`discussion-composer-${width}-${scale}-${await page.eval('document.documentElement.dataset.theme')}`, {boundary:'intercepted-provider-contract-fixture-no-persistence'});
    await page.eval(`window.scrollTo({left:${scroll.x},top:${scroll.y},behavior:'instant'})`);
  }
}

async function captureDiscussionMatrix(page, name, anchor, boundary) {
  for (const theme of ['light', 'dark']) {
    await page.eval(`document.documentElement.dataset.theme=${JSON.stringify(theme)}`);
    for (const width of [320, 390, 834, 1024, 1440]) {
      await page.viewport(width);
      await page.eval(`document.documentElement.style.fontSize=''; document.querySelector(${JSON.stringify(anchor)}).scrollIntoView({block:'start'})`);
      check(await page.eval(`document.documentElement.scrollWidth<=innerWidth+1 && document.querySelectorAll('.vk-discussion-replies .vk-discussion-replies').length===0`), `${name}/${theme}/${width}: reflows without a third reply tier`);
      await page.capture(`discussion-${name}-${width}-${theme}`, { scenario: 'D2T-03-responsive', boundary, build: BUILD_SHA, textScale: 100 });
      if (name === 'contract-fixture-flat') await discussionR31Geometry(page, width, 100, `${name}/${theme}/${width}`);
      if (width <= 390 || (width === 1440 && name === 'contract-fixture-flat')) {
        await page.eval(`document.documentElement.style.fontSize='200%'; document.querySelector(${JSON.stringify(anchor)}).scrollIntoView({block:'start'})`);
        check(await page.eval(`document.documentElement.scrollWidth<=innerWidth+1`), `${name}/${theme}/${width}/200%: text reflows`);
        if (name === 'contract-fixture-flat') await discussionR31Geometry(page, width, 200, `${name}/${theme}/${width}/200%`);
        await page.capture(`discussion-${name}-${width}-200-${theme}`, { scenario: 'D2T-03-large-text', boundary, build: BUILD_SHA, textScale: 200 });
      }
    }
  }
  await page.eval(`document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme`);
  await page.viewport(1280);
}

async function afterRestartPhase(page) {
  const state = JSON.parse(readFileSync(args.state, 'utf8'));
  console.log('after server restart:');
  await page.goto(`${args.base}/ban-viet/${state.workId}`);
  await page.waitFor(`${EDITOR_READY} && document.documentElement.dataset.hydrated === 'true'`, 'editor');
  check((await page.body()) === state.text, 'confirmed text survived the restart');
  check((await page.eval(`document.querySelector('input[name=title]').value`)) === state.title, 'title survived the restart');
  check((await page.eval(`document.querySelector('#editor-save-details').textContent`)).includes(`phiên bản ${state.version}`), 'version survived the restart');
  await retiredTagAfterRestart(page, state.workId);

  console.log('export keeps the text being written:');
  const downloads = mkdtempSync(join(tmpdir(), 'vink-downloads-'));
  try {
    await page.cdp.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads, eventsEnabled: true });
    await page.typeInBody(STANZA);
    const current = await page.body();
    check(current === state.text + STANZA, 'unsaved stanza typed after the restart');
    await page.click('#export');
    const file = await downloaded(downloads);
    // HFS+ returns decomposed file names; only the name comparison is canonicalized.
    check(file.name.normalize('NFC') === EXPORT_NAME, `export file uses the capped title name (${file.name})`);
    check(file.text === `${state.title}\n\n${current}\n`, 'export holds the title and the unsaved text with lines and stanzas');
    check((await page.status()) === 'unsaved', 'export is not a save');
  } finally {
    rmSync(downloads, { recursive: true, force: true });
  }
}

mkdirSync(dirname(args.state), { recursive: true });
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
  if (args.phase === 'discussion-only') await discussionReadingAndWrites(page);
  else if (args.phase === 'select-chevron') await selectChevron(page);
  else if (args.phase === 'editor-chrome') await editorChrome(page);
  else if (args.phase === 'main') await mainPhase(page);
  else if (args.phase === 'after-restart') await afterRestartPhase(page);
  else if (args.phase === 'endorsement') await endorsementAsReader(page, { args, check, captureMatrix: captureDiscussionMatrix });
  else if (args.phase === 'review-withdrawn') await reviewWithdrawnPage(page, { args, check });
  else if (args.phase === 'auth') await authWeb(page, { args, check, openPage: () => openPage(cdp) });
  else if (args.phase === 'discovery-corpus') await discoveryCorpus(page, { args, check });
  else if (args.phase === 'public-search') await publicReading(page);
  else if (args.phase === 'public-cache') await cacheFirstPublicReads(page);
  else if (args.phase === 'desk-layout') { await draftDashboard(page); await draftR6Hierarchy(page); await publicReading(page); }
  else if (args.phase === 'bottom-navigation') await bottomNavigationRegression(page);
  else if (args.phase === 'appearance') await adaptiveShell(page);
  else if (args.phase === 'first-paint') {
    await page.goto(`${args.base}/toi`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'appearance hydration');
    await appearanceFirstPaint(page);
  }
  else throw new Error(`unknown phase ${args.phase}`);
  check(page.errors.length === 0, `no console errors or exceptions${page.errors.length ? `: ${page.errors.join(' | ')}` : ''}`);
  console.log(`browser-smoke: PASS (${args.phase})`);
} catch (error) {
  if (activePage) {
    try {
      await activePage.capture('failure');
      console.error('browser context:', await activePage.eval(`({ path: location.pathname, preferences: { ...document.documentElement.dataset }, active: { tag: document.activeElement?.tagName, id: document.activeElement?.id }, selectionLength: getSelection().toString().length, view: document.querySelector('.cm-editor')?.__vinkView, status: document.querySelector('[data-testid=save-status]')?.dataset.status, errors: ${JSON.stringify(activePage.errors)} })`));
    } catch { /* Preserve the original failure if the page itself crashed. */ }
  }
  console.error(`browser-smoke: FAIL ${error.message}`);
  exitCode = 1;
} finally {
  // Chrome may still be writing its profile; removing it must not replace the smoke's result.
  const exited = chrome.child.exitCode !== null ? null : new Promise((resolve) => chrome.child.once('exit', resolve));
  chrome.child.kill();
  await Promise.race([exited, sleep(5000)]);
  rmSync(chrome.profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}
process.exit(exitCode);
