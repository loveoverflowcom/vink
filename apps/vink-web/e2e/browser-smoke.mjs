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

import { defaultDocument, paragraphs } from './document.mjs';
import { execFileSync, spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { checkSelectGeometry, selectCharacter } from '../../../packages/ui-system/leptos/preview/e2e/select-geometry.mjs';
import { discoveryReading, discoveryCorpus } from './discovery-smoke.mjs';
import { discoveryChipGeometry } from './discovery-chip-geometry.mjs';
import { expressionPilot } from './expressions-smoke.mjs';
import { endorsementAsAuthor, endorsementAsReader } from './endorsement-smoke.mjs';
import { authWeb } from './auth-smoke.mjs';
import { readerAuthClosed } from './reader-auth-smoke.mjs';
import { passwordAuth } from './auth-password-smoke.mjs';
import { publicationAuthor } from './publication-author-smoke.mjs';
import { reviewWithdrawnPage } from './review-withdrawn-smoke.mjs';
import { markdownReader } from './markdown-reader-smoke.mjs';
import { moderationQueues } from './moderation-queue-smoke.mjs';
import { publicStorage, storageReopened } from './public-storage-smoke.mjs';
import { draftPagination } from './draft-pagination-smoke.mjs';
import { authorWorks } from './author-works-smoke.mjs';
import { editorWriting } from './editor-writing-smoke.mjs';
import { editorPaperFirst } from './editor-paper-first-smoke.mjs';
import { editorDeskRegressions, firstDraftFormat } from './editor-desk-regressions.mjs';
import { markdownBudget } from './editor-markdown-budget.mjs';
import { documentBudget } from './document-budget.mjs';
import { readerIntentCost } from './reader-intent-cost.mjs';
import { webPerfBaseline } from './web-perf-baseline.mjs';
import { readerContainment } from './reader-containment.mjs';
import { discoveryReturnPosition } from './discovery-return-position.mjs';
import { recordTransport } from './web-request-ledger.mjs';
import { readerBeforeHydration, readerGuestHistory } from './reader-ssr-boundary.mjs';
import { deskEmptyState, deskGuest } from './desk-empty-smoke.mjs';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import {
  MANIFEST_SCHEMA_VERSION,
  buildLaneSummary,
  buildModalInspectionScript,
  buildNavIndicatorInspectionScript,
  buildNavInspectionScript,
  buildOverflowInspectionScript,
  buildTargetGeometryScript,
  classifyTextScaleLane,
  collectElementAccessibility,
  collectSourceIdentity,
  evaluateAccessibleElement,
  evaluateLeaveModalBranch,
  evaluateMePanelReservation,
  evaluateNavAlignment,
  evaluateOverflowReport,
  evaluateTargetGeometry,
  parsePngMetadata,
  validateCaptureScene,
  validateManifestRecord,
} from './visual-evidence.mjs';

const SAMPLE_WORK = '33333333-3333-4333-8333-333333333333';
const BUILD_SHA = process.env.GITHUB_SHA ?? execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
const BUILD_DIRTY = execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim();
const SOURCE_IDENTITY = collectSourceIdentity({ buildSha: BUILD_SHA });
const EVIDENCE_STATE = {
  lanes: {
    appAppearanceControl: 'NOT_RUN',
    domOverrideGeometryOnly: 'NOT_RUN',
  },
  negativeControls: {},
};
const PUBLISHED_LINE = 'Phố cũ mưa giăng sợi mỏng,';
const INDENTED_LINE = '    đã theo mưa về rất lâu.';
const DRAFT_ONLY_LINE = 'Khổ đang viết dở — chưa công khai.';

const TITLE = 'Thử nghiệm Bàn viết — Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ tiếng Việt và những khổ thơ còn đang viết dở';
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
  const hydrated = await page.eval(`document.documentElement.dataset.hydrated === 'true'`);
  if (hydrated) {
    await page.eval(`document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))`);
    await page.waitFor(`Math.abs(parseFloat(getComputedStyle(document.querySelector('.vk-ui-scaffold')).getPropertyValue('--vk-ui-bar-height')) - document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().height) < 1`, 'bottom bar resize clearance');
  } else {
    await page.eval(`document.fonts.ready`);
  }
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
  const inspected = await page.eval(buildNavInspectionScript('.vk-ui-bottom-nav'));
  const evalResult = evaluateNavAlignment(inspected.items, { tolerancePx: 1, maxLines: 2, checkSameRow: true, expectedCount: 3 });
  check(evalResult.ok, `${scenario}: equal targets, icon centres, first label lines and label font align within 1px: ${evalResult.reasons.join('; ') || JSON.stringify({ iconDelta: evalResult.maxIconCenterDelta, firstLineDelta: evalResult.maxFirstLineDelta, lineCounts: evalResult.lineCounts })}`);
  const toiAx = await collectElementAccessibility(page, '.vk-ui-bottom-nav a[href="/toi"]');
  const routePath = await page.eval(`location.pathname`);
  const toiAxEval = evaluateAccessibleElement(toiAx, {
    role: 'link',
    name: 'Tôi',
    excludeSubstrings: ['TT', '•'],
    ariaCurrent: routePath === '/toi' ? 'page' : null,
  });
  check(toiAxEval.ok, `${scenario}: Tôi AX name excludes decorative initials (${JSON.stringify({ axName: toiAx.ax.name, rawText: toiAx.rawTextContent, ariaCurrent: toiAx.dom.ariaCurrent })})${toiAxEval.reasons.length ? ' -> ' + toiAxEval.reasons.join('; ') : ''}`);
  const overflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({ containers: ['.vk-ui-bottom-nav'] })));
  check(overflow.ok, `${scenario}: bottom bar and page have no unintended/hidden overflow${overflow.reasons.length ? ': ' + overflow.reasons.join('; ') : ''}`);
  return inspected.items.map((item) => ({
    name: item.labelText,
    x: item.x,
    y: item.y,
    width: item.width,
    height: item.height,
    iconCenterY: item.iconCenterY,
    firstLineY: item.firstLineY,
    font: item.labelFont,
    lineHeight: item.labelLineHeight,
    lineCount: item.lineCount,
  }));
}

async function bottomNavigationAnatomy(page, route, scenario) {
  const items = await page.eval(buildNavIndicatorInspectionScript());
  check(items.map(item => item.href).join('|') === '/|/ban-viet|/toi' &&
    items.every(item => item.current === (item.href === route ? 'page' : null)),
  `${scenario}: route order and exactly one matching aria-current`);
  check(items.every(item => item.flatTarget), `${scenario}: selected background must not cover the whole destination`);
  const isolated = items.every(item => item.fill === item.expectedFill && item.leadingInside && item.labelOutside) && items[2].avatar;
  check(isolated, `${scenario}: selected Lavender indicator contains only the icon/avatar; labels sit below` +
    (isolated ? '' : ` (${JSON.stringify(items)})`));
}

async function bottomNavigationStates(page) {
  const selected = '.vk-ui-bottom-nav a[aria-current="page"]';
  const styles = () => page.eval(`(async () => {
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const link = document.querySelector('${selected}'), indicator = link.querySelector('.vk-ui-icon').parentElement;
    const style = getComputedStyle(indicator);
    return { fill: style.backgroundColor, layer: style.boxShadow,
      hovered: link.matches(':hover'), pressed: link.matches(':active'),
      hoverMedia: matchMedia('(hover: hover)').matches,
      hasOutline: style.outlineStyle === 'solid',
      hasFocus: link.matches(':focus-visible') && getComputedStyle(link).outlineStyle === 'solid',
      motion: style.transitionDuration, transition: style.transitionProperty,
      bounds: indicator.getBoundingClientRect().toJSON() };
  })()`);
  const reduce = { name: 'prefers-reduced-motion', value: 'reduce' };
  await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM, reduce] });
  try {
    check(await page.eval(`matchMedia('(hover: hover)').matches`),
      'Chrome reports a hover-capable pointer for the navigation mouse probe');
    await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1, y: 1 });
    const rest = await styles();
    const point = { x: rest.bounds.x + rest.bounds.width / 2, y: rest.bounds.y + rest.bounds.height / 2 };
    await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point });
    const hover = await styles();
    await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
    const press = await styles();
    await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 1, y: 1, button: 'left', clickCount: 1 });
    check(!rest.hovered && !rest.pressed && hover.hovered && !hover.pressed && press.hovered && press.pressed,
      'real pointer input reaches the selected bottom navigation link');
    const distinctLayers = new Set([rest.layer, hover.layer, press.layer]).size === 3;
    check(distinctLayers, 'rest, hover and pressed use distinct state layers' +
      (distinctLayers ? '' : ` (${JSON.stringify({ rest, hover, press })})`));
    const stableIndicator = [hover, press].every(state =>
      state.fill === rest.fill && JSON.stringify(state.bounds) === JSON.stringify(rest.bounds));
    check(stableIndicator, 'hover and pressed preserve selected fill and geometry' +
      (stableIndicator ? '' : ` (${JSON.stringify({ rest, hover, press })})`));
    check(await page.eval(`matchMedia('(prefers-reduced-motion: reduce)').matches`) &&
      rest.motion.split(',').every(duration => parseFloat(duration) <= 0.001),
    `bottom navigation honors reduced motion (${rest.motion})`);

    await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM, reduce, { name: 'forced-colors', value: 'active' }] });
    await page.eval(`document.querySelector('.vk-ui-bottom-nav a[href="/ban-viet"]').focus()`);
    await page.key('Tab');
    const forced = await styles();
    check(forced.hasOutline && forced.hasFocus, 'forced colors preserves the selected indicator and keyboard focus');
    await page.capture('nav-toi-360x800-200-forced-colors', { requestedScene: { width: 360, height: 800, scale: 200 } });
  } finally {
    await page.send('Emulation.setEmulatedMedia', { features: [LIGHT_SYSTEM] });
  }
}

async function bottomNavigationSafeArea(page) {
  await page.viewport(390);
  await page.eval(`document.documentElement.style.fontSize = ''`);
  const bar = `document.querySelector('.vk-ui-bottom-nav')`;
  const before = await page.eval(`({ height: ${bar}.getBoundingClientRect().height, padding: parseFloat(getComputedStyle(${bar}).paddingBottom) })`);
  try {
    await page.send('Emulation.setSafeAreaInsetsOverride', { insets: { bottom: 24, left: 20, right: 20 } });
  } catch (error) {
    if (!String(error).includes("wasn't found")) throw error;
    console.log('  UNSUPPORTED  Chrome CDP safe-area inset overrides');
    return;
  }
  try {
    await page.waitFor(`${bar}.getBoundingClientRect().height > ${before.height} &&
      parseFloat(getComputedStyle(${bar}).paddingBottom) === ${before.padding + 24}`, 'bar grows around the safe area');
    await bottomNavigationAlignment(page, '390px with emulated safe area');
    check(await page.eval(`(() => {
      const bar = ${bar}, links = bar.querySelectorAll('a');
      const lastChoice = document.querySelector('#page-rail input:checked');
      lastChoice.focus();
      scrollTo(0, document.documentElement.scrollHeight);
      const r = bar.getBoundingClientRect();
      return links[0].getBoundingClientRect().left >= r.left + 20 &&
        links[2].getBoundingClientRect().right <= r.right - 20 &&
        links[2].getBoundingClientRect().bottom <= r.bottom - 24 &&
        document.activeElement === lastChoice &&
        lastChoice.closest('label').getBoundingClientRect().bottom <= r.top;
    })()`), 'safe-area padding and measured clearance leave the last focused control reachable');
  } finally {
    await page.send('Emulation.setSafeAreaInsetsOverride', { insets: { bottom: 0, left: 0, right: 0 } });
  }
}

async function bottomNavigationLinks(page) {
  const link = href => `.vk-ui-bottom-nav a[href="${href}"]`;
  const selected = href => page.waitFor(
    `location.pathname === '${href}' && document.querySelector('${link(href)}').getAttribute('aria-current') === 'page'`,
    `mobile Router and current destination for ${href}`,
  );
  for (const href of ['/', '/ban-viet', '/toi']) {
    await page.click(link(href));
    await selected(href);
  }
  await page.eval('history.back()');
  await selected('/ban-viet');
  await page.eval('history.forward()');
  await selected('/toi');
  const point = await page.eval(`(() => {
    const r = document.querySelector('${link('/')}').getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  })()`);
  for (const button of ['modifier', 'middle']) {
    const prior = (await page.cdp.send('Target.getTargets')).targetInfos.map(target => target.targetId);
    if (button === 'modifier') {
      await page.click(link('/'), page.mod);
    } else {
      for (const type of ['mousePressed', 'mouseReleased']) {
        await page.send('Input.dispatchMouseEvent', { type, ...point, button, clickCount: 1 });
      }
    }
    let opened;
    for (let attempt = 0; attempt < 50 && !opened; attempt++) {
      const { targetInfos } = await page.cdp.send('Target.getTargets');
      opened = targetInfos.find(target => target.type === 'page' && target.url === `${args.base}/` && !prior.includes(target.targetId));
      if (!opened) await sleep(100);
    }
    check(opened && await page.eval("location.pathname === '/toi'"), `${button} click opens a native new tab while keeping the current route`);
    await page.cdp.send('Target.closeTarget', { targetId: opened.targetId });
    await page.send('Page.bringToFront');
  }
}

async function bottomNavigationRegression(page) {
  for (const theme of ['dark', 'light']) {
    await page.viewport(320);
    await page.goto(`${args.base}/toi`);
    await chooseTheme(page, theme, 'page-theme');
    for (const width of [320, 360, 390, 480, 759]) {
      await page.viewport(width);
      for (const scale of [100, 200]) {
        let reference;
        for (const route of ['/toi', '/', '/ban-viet']) {
          await page.goto(`${args.base}${route}`);
          await page.waitFor(
            `document.documentElement.dataset.hydrated === 'true'` +
              (route === '/ban-viet'
                ? ` && Boolean(document.querySelector('#draft-scope')) && document.querySelector('#draft-scope').textContent !== 'Đang tìm tác phẩm…'`
                : ''),
            'navigation route hydration',
          );
          await page.eval(`document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
          const slug = route === '/' ? 'explore' : route.slice(1);
          const name = `nav-${slug}-${width}-${scale}-${theme}`;
          await page.capture(name, {
            scenario: `bottom-nav-${width}x900`,
            requestedScene: { width, height: 900, scale, theme },
            requiredTargets: ['.vk-ui-bottom-nav', '.vk-ui-bottom-nav a[href="/toi"]'],
            boundary: 'Chrome-CDP-dom-override-geometry',
          });
          await bottomNavigationAnatomy(page, route, name);
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
            const content = document.querySelector('.vk-ui-scaffold__content'), bar = document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect();
            return parseFloat(getComputedStyle(content).paddingBottom) >= bar.height + 27 &&
              document.querySelector('#noi-dung').getBoundingClientRect().bottom <= bar.top;
          })()`), `${name}: measured content clearance follows the bar`);
        }
      }
    }
    // Explicit 800-height mobile nav verification: never silently substitute 900-height captures.
    for (const width of [320, 360, 390]) {
      await page.viewport(width, 800);
      await page.goto(`${args.base}/ban-viet`);
      await page.waitFor(
        `document.documentElement.dataset.hydrated === 'true' && Boolean(document.querySelector('#draft-scope')) && document.querySelector('#draft-scope').textContent !== 'Đang tìm tác phẩm…'`,
        `800-height nav hydration (${width})`,
      );
      await page.eval(`document.documentElement.style.fontSize = '200%'; scrollTo(0, 0)`);
      await page.capture(`nav-ban-viet-${width}x800-200-${theme}`, {
        scenario: `bottom-nav-${width}x800`,
        requestedScene: { width, height: 800, scale: 200, theme },
        requiredTargets: ['.vk-ui-bottom-nav', '.vk-ui-bottom-nav a[href="/toi"]'],
        boundary: 'Chrome-CDP-dom-override-geometry',
      });
      await bottomNavigationAlignment(page, `nav-ban-viet-${width}x800-200-${theme}`);
    }
    await page.viewport(390);
    await page.goto(`${args.base}/`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'navigation contrast hydration');
    await textContrast(page, theme, 'bottom-navigation', [
      ...['/', '/ban-viet', '/toi'].map(href => `.vk-ui-bottom-nav a[href="${href}"] .vk-ui-nav-item__label`),
      '.vk-ui-bottom-nav a[aria-current="page"] .vk-ui-icon',
      '.vk-ui-bottom-nav a[href="/ban-viet"] .vk-ui-icon',
    ]);
  }
  // Before-hydration vs after-hydration check for Tôi AX name and alignment at 360x800.
  const preErrors = page.errors.length;
  await page.viewport(360, 800);
  await page.send('Network.setBlockedURLs', { urls: [`${args.base}/pkg/vink.js*`, `${args.base}/pkg/vink_bg.wasm*`] });
  try {
    await page.goto(`${args.base}/toi`);
    await page.eval(`document.documentElement.style.fontSize = '200%'; scrollTo(0, 0)`);
    await page.capture('nav-toi-360x800-200-light-before-hydration', {
      scenario: 'bottom-nav-before-hydration',
      hydrationPhase: 'before-hydration',
      state: 'before-hydration',
      requestedScene: { width: 360, height: 800, scale: 200, hydrationPhase: 'before-hydration' },
      requiredTargets: ['.vk-ui-bottom-nav', '.vk-ui-bottom-nav a[href="/toi"]'],
      boundary: 'no-js-bootstrap-pre-hydration',
    });
    await bottomNavigationAnatomy(page, '/toi', 'before hydration');
    await bottomNavigationAlignment(page, 'nav-toi-360x800-200-before-hydration');
  } finally {
    await page.send('Network.setBlockedURLs', { urls: [] });
    for (const deadline = Date.now() + 2000; page.errors.length < preErrors + 1 && Date.now() < deadline;) await sleep(50);
    const blocked = page.errors.splice(preErrors);
    check(blocked.every((e) => /Failed to fetch dynamically imported module|Importing a module script failed|ERR_BLOCKED_BY_CLIENT/.test(e)),
      `before-hydration nav check produces only expected blocked imports: ${JSON.stringify(blocked)}`);
  }
  await page.goto(`${args.base}/toi`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'after-hydration nav check');
  await page.waitFor(`document.querySelector('.vk-ui-bottom-nav a[href="/toi"] [data-kind=account]')?.dataset.initials === 'TT'`, 'nav avatar initials hydrated');
  await page.eval(`document.documentElement.style.fontSize = '200%'; scrollTo(0, 0)`);
  await bottomNavigationAnatomy(page, '/toi', 'signed-in avatar');
  await bottomNavigationAlignment(page, 'nav-toi-360x800-200-after-hydration');

  // The old failure was painting the whole selected link even though its geometry and AX name pass.
  const selected = '.vk-ui-bottom-nav a[aria-current="page"]';
  try {
    await page.eval(`document.querySelector('${selected}').style.background = 'var(--vk-primary-container)'`);
    const items = await page.eval(buildNavIndicatorInspectionScript());
    check(!items.find(item => item.current === 'page').flatTarget,
      'negative control: anatomy oracle rejects a selected fill on the whole destination');
    EVIDENCE_STATE.negativeControls.wholeDestinationFill = 'PASS';
  } finally {
    await page.eval(`document.querySelector('${selected}').style.background = ''`);
  }

  // Browser negative controls: misaligned first line and altered accessible name (Tôi -> TTTôi).
  try {
    await page.eval(`document.querySelector('.vk-ui-bottom-nav a[href="/toi"] .vk-ui-nav-item__label').style.paddingTop = '18px'`);
    const badNav = await page.eval(buildNavInspectionScript('.vk-ui-bottom-nav'));
    const badNavEval = evaluateNavAlignment(badNav.items, { tolerancePx: 1 });
    check(!badNavEval.ok && badNavEval.reasons.some(r => r.includes('firstLineY')), 'negative control: browser oracle rejects misaligned nav first line');
    await page.eval(`(() => {
      const av = document.querySelector('.vk-ui-bottom-nav a[href="/toi"] .vk-nav-account-avatar');
      av.removeAttribute('aria-hidden');
      for (const hidden of av.querySelectorAll('[aria-hidden]')) hidden.removeAttribute('aria-hidden');
    })()`);
    const badAx = await collectElementAccessibility(page, '.vk-ui-bottom-nav a[href="/toi"]');
    const badAxEval = evaluateAccessibleElement(badAx, { role: 'link', name: 'Tôi', excludeSubstrings: ['TT', '•'] });
    check(!badAxEval.ok && String(badAx.ax.name).includes('TT'), `negative control: Chrome AX oracle catches altered accessible name (${badAx.ax.name})`);
    EVIDENCE_STATE.negativeControls.misalignedNavFirstLine = 'PASS';
    EVIDENCE_STATE.negativeControls.alteredAccessibleName = 'PASS';
  } finally {
    await page.eval(`(() => {
      const lbl = document.querySelector('.vk-ui-bottom-nav a[href="/toi"] .vk-ui-nav-item__label');
      if (lbl) lbl.style.paddingTop = '';
      const av = document.querySelector('.vk-ui-bottom-nav a[href="/toi"] .vk-nav-account-avatar');
      if (av) {
        av.setAttribute('aria-hidden', 'true');
        const inner = av.querySelector('[data-kind=account]');
        if (inner) inner.setAttribute('aria-hidden', 'true');
      }
    })()`);
  }

  // Font-size and viewport changes above exercise ResizeObserver after hydration.
  await bottomNavigationStates(page);
  await bottomNavigationSafeArea(page);
  await bottomNavigationLinks(page);
  await page.eval(`document.documentElement.style.fontSize = ''`);
  await page.goto(`${args.base}/toi`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'rail hydration');
  for (const [width, railWidth] of [[759, 0], [760, 96], [834, 96], [1440, 220]]) {
    await page.viewport(width);
    check(await page.eval(`(() => {
      const rail = document.querySelector('.vk-ui-rail'), bar = document.querySelector('.vk-ui-bottom-nav');
      return rail.getBoundingClientRect().width === ${railWidth} &&
        Boolean(bar.getClientRects().length) === ${railWidth === 0};
    })()`), `${width}px: expected bottom bar or rail (${railWidth}px)`);
    if (railWidth) await page.capture(`nav-rail-${width}`);
  }
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

  async waitFor(expression, what, timeout = 35000) {
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
    const png = Buffer.from(data, 'base64');
    writeFileSync(join(dir, `${name}.png`), png);
    const pngMeta = parsePngMetadata(png);
    const requiredTargets = Array.isArray(evidence.requiredTargets)
      ? evidence.requiredTargets
      : typeof evidence.target === 'string'
        ? [evidence.target]
        : [];
    const scrollSelector = typeof evidence.scrollSelector === 'string' ? evidence.scrollSelector : null;
    const context = await this.eval(`(() => {
      const root = document.documentElement;
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
      const explicitOwner = ${JSON.stringify(scrollSelector)} ? document.querySelector(${JSON.stringify(scrollSelector)}) : null;
      const openDialog = document.querySelector('dialog[open]');
      const ownerEl = explicitOwner || (openDialog && openDialog.scrollHeight > openDialog.clientHeight ? openDialog : null) || document.scrollingElement || root;
      const ownerStyle = getComputedStyle(ownerEl);
      const navSnapshot = (containerSelector) => {
        const container = document.querySelector(containerSelector);
        if (!container || !container.getClientRects().length) return [];
        const containerFont = getComputedStyle(container).font;
        return [...container.querySelectorAll('.vk-ui-nav-item')].map(a => {
          const r = a.getBoundingClientRect();
          const iconEl = a.querySelector('.vk-ui-icon');
          const iconR = iconEl ? iconEl.getBoundingClientRect() : null;
          const labelEl = a.querySelector('.vk-ui-nav-item__label') ?? a;
          const labelRect = labelEl.getBoundingClientRect();
          const labelStyle = getComputedStyle(labelEl);
          const range = document.createRange();
          range.selectNodeContents(labelEl);
          const rects = [...range.getClientRects()].filter(q => q.width > 0 && q.height > 0);
          const first = rects[0] ?? null;
          return {
            label: a.textContent,
            labelText: labelEl.textContent,
            rawTextContent: a.textContent,
            href: a.getAttribute('href'),
            ariaCurrent: a.getAttribute('aria-current'),
            target: { x: r.x, y: r.y, width: r.width, height: r.height, fontSize: labelStyle.fontSize, lineHeight: labelStyle.lineHeight },
            icon: iconR ? { x: iconR.x, y: iconR.y, width: iconR.width, height: iconR.height } : null,
            iconCenterY: iconR ? iconR.y + iconR.height / 2 : null,
            text: { x: labelRect.x, y: labelRect.y, width: labelRect.width, height: labelRect.height, fontSize: labelStyle.fontSize, lineHeight: labelStyle.lineHeight },
            firstLine: first ? { x: first.x, y: first.y, width: first.width, height: first.height } : null,
            lineCount: new Set(rects.map(q => Math.round(q.top))).size,
            labelFont: labelStyle.font,
            labelFontSize: labelStyle.fontSize,
            labelLineHeight: labelStyle.lineHeight,
            containerFont,
          };
        });
      };
      const requiredSelectors = ${JSON.stringify(requiredTargets)};
      const requiredResults = requiredSelectors.map((sel) => {
        const el = document.querySelector(sel);
        if (!el) return { selector: sel, present: false, visible: false, full: false, hit: false };
        const visible = Boolean(el.checkVisibility?.({ visibilityProperty: true, opacityProperty: true }));
        const r = el.getBoundingClientRect();
        return { selector: sel, present: true, visible, rect: r.toJSON() };
      });
      const scaffold = document.querySelector('.vk-ui-scaffold');
      const bar = document.querySelector('.vk-ui-bottom-nav');
      const barVar = scaffold ? parseFloat(getComputedStyle(scaffold).getPropertyValue('--vk-ui-bar-height')) : null;
      const barRect = bar && bar.getClientRects().length ? bar.getBoundingClientRect() : null;
      return {
        route: location.pathname,
        width: innerWidth,
        layoutWidth: root.clientWidth,
        clientWidth: root.clientWidth,
        clientHeight: root.clientHeight,
        height: innerHeight,
        dpr: devicePixelRatio,
        scrollX,
        scrollY,
        documentScrollWidth: root.scrollWidth,
        documentScrollHeight: root.scrollHeight,
        hydrated: root.dataset.hydrated === 'true',
        theme: root.dataset.theme || 'system',
        requestedTheme: root.dataset.theme || 'system',
        effectiveTheme: root.dataset.theme || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'),
        osTheme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
        fontSize: getComputedStyle(root).fontSize,
        rootInlineFontSize: root.style.fontSize || '',
        dataText: root.dataset.text || null,
        textScale: Math.round(parseFloat(getComputedStyle(root).fontSize) / 16 * 100),
        media: {
          forcedColors: matchMedia('(forced-colors: active)').matches,
          reducedMotion: matchMedia('(prefers-reduced-motion: reduce)').matches,
          prefersColorScheme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
        },
        fonts: {
          body: getComputedStyle(document.body).fontFamily,
          required: ['16px "Be Vietnam Pro"', '16px "Literata"'].map(face => ({ face, ready: document.fonts.check(face) })),
        },
        scrollOwner: {
          selector: ownerEl === root ? 'html' : (ownerEl.id ? '#' + ownerEl.id : ownerEl.tagName.toLowerCase()),
          id: ownerEl.id || null,
          tagName: ownerEl.tagName,
          scrollTop: ownerEl.scrollTop,
          scrollLeft: ownerEl.scrollLeft,
          scrollHeight: ownerEl.scrollHeight,
          clientHeight: ownerEl.clientHeight,
          scrollWidth: ownerEl.scrollWidth,
          clientWidth: ownerEl.clientWidth,
          overflowX: ownerStyle.overflowX,
          overflowY: ownerStyle.overflowY,
        },
        userAgent: navigator.userAgent,
        appearance: { effectiveTheme: root.dataset.theme || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'), osTheme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light', hydrated: root.dataset.hydrated === 'true', rail: geometry('.vk-ui-rail'), railScrollWidth: document.querySelector('.vk-ui-rail')?.scrollWidth, railClientWidth: document.querySelector('.vk-ui-rail')?.clientWidth, pageChoice: document.querySelector('#page-theme input:checked')?.value, overlayChoice: document.querySelector('#overlay-theme input:checked')?.value, overlayOpen: document.querySelector('#profile-menu')?.open, overlayScroll: document.querySelector('#profile-menu')?.scrollTop, overlay: geometry('#profile-menu'), account: geometry('#page-account'), themeGroup: geometry('#page-theme') },
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
          const preferenceSurface = prefix === 'page' ? '.vk-appearance-card' : '#profile-menu';
          const surface = document.querySelector(preferenceSurface);
          const surfaceStyle = getComputedStyle(surface);
          const region = surface.querySelector('.vk-appearance-region');
          const rows = [...region.querySelectorAll('.vk-appearance-row')];
          const illEl = document.querySelector('#' + prefix + '-account .vk-account-illustration');
          const illStyle = illEl ? getComputedStyle(illEl) : null;
          const illVisible = Boolean(illEl && illEl.getClientRects().length > 0 && illStyle.display !== 'none');
          const illRect = illVisible ? illEl.getBoundingClientRect().toJSON() : null;
          const textBox = document.querySelector('#' + prefix + '-account .vk-account-status > div');
          const textStyle = textBox ? getComputedStyle(textBox) : null;
          const textBoxRect = textBox ? textBox.getBoundingClientRect() : null;
          const h2Rect = document.querySelector('#' + prefix + '-account h2')?.getBoundingClientRect();
          const helpRect = document.querySelector('#' + prefix + '-account .vk-ui-tone-muted')?.getBoundingClientRect();
          const textRight = Math.max(h2Rect?.right ?? 0, helpRect?.right ?? 0);
          return { prefix, surface: bounds(prefix === 'page' ? '#me' : '#profile-menu'),
            preferences: {
              surface: bounds(preferenceSurface), padding: ['Top', 'Right', 'Bottom', 'Left'].map(side => parseFloat(surfaceStyle['padding' + side])),
              background: surfaceStyle.backgroundColor, radius: parseFloat(surfaceStyle.borderRadius),
              scroll: { top: surface.scrollTop, height: surface.scrollHeight, clientHeight: surface.clientHeight },
              rowGap: parseFloat(getComputedStyle(region).rowGap),
              rows: rows.map((row, index) => {
                const fieldset = row.querySelector('fieldset');
                const selector = '#' + fieldset.id;
                const heading = bounds(selector + '-title');
                const group = bounds(selector);
                const selected = fieldset.querySelector('label:has(:checked)');
                return {
                  heading, group, gap: group.y - heading.y - heading.height,
                  before: index ? row.getBoundingClientRect().top - rows[index - 1].getBoundingClientRect().bottom : null,
                  inset: parseFloat(getComputedStyle(row).paddingBlockStart),
                  divider: parseFloat(getComputedStyle(row).borderBlockStartWidth),
                  choiceGap: parseFloat(getComputedStyle(fieldset).columnGap),
                  targets: [...fieldset.querySelectorAll('label')].map((_, i) => bounds(selector + ' label:nth-of-type(' + (i + 1) + ')')),
                  selected: selected && getComputedStyle(selected).backgroundColor,
                };
              }),
            },
            heading: bounds(prefix === 'page' ? '#me-title' : '.vk-profile-heading'),
            account: bounds('#' + prefix + '-account'), identity: bounds('#' + prefix + '-account .vk-account-status'),
            identityContainerWidth: document.querySelector('#' + prefix + '-account .vk-account-status')?.clientWidth ?? null,
            avatar: bounds('#' + prefix + '-account .vk-ui-avatar'),
            accountText: bounds('#' + prefix + '-account .vk-account-status > div'),
            accountLabel: bounds('#' + prefix + '-account h2'),
            accountHelp: bounds('#' + prefix + '-account .vk-ui-tone-muted'),
            illustration: illEl ? {
              present: true,
              visible: illVisible,
              display: illStyle.display,
              ariaHidden: illEl.getAttribute('aria-hidden'),
              pointerEvents: illStyle.pointerEvents,
              rect: illRect,
            } : { present: false, visible: false, display: null, ariaHidden: null, pointerEvents: null, rect: null },
            reservation: textBox ? {
              paddingInlineEnd: parseFloat(textStyle.paddingInlineEnd),
              accountTextContentWidth: textBox.clientWidth - parseFloat(textStyle.paddingLeft) - parseFloat(textStyle.paddingRight),
              accountTextScrollWidth: textBox.scrollWidth,
              accountTextClientWidth: textBox.clientWidth,
              accountTextContentRight: textBoxRect ? textBoxRect.right - parseFloat(textStyle.paddingRight) : null,
              textRight,
              illustrationLeft: illRect?.left ?? null,
              textOverlapsIllustration: Boolean(illVisible && illRect && textRight > illRect.left + 1),
            } : null,
            appearanceHeading: bounds(prefix === 'page' ? '#page-appearance-title' : '#profile-title'),
            choices: bounds((prefix === 'page' ? '#me' : '#profile-menu') + ' .vk-appearance-region'),
            group: bounds('#' + prefix + '-theme'),
            action: prefix === 'overlay' ? bounds('#open-me') : null };
        })(),
        desk: { heading: geometry('#heading'), resume: geometry('#resume'), title: geometry('#resume h2'), disclosure: geometry('#resume-title-toggle'), metadata: geometry('#resume .vk-draft-meta'), cta: geometry('[data-testid=resume-draft]'), list: geometry('#drafts'), tools: geometry('#draft-tools'), search: geometry('#draft-search'), sort: geometry('#draft-sort'), refresh: geometry('#refresh-drafts'), scope: geometry('#draft-scope'), firstRow: geometry('[data-testid=draft-row]'), rowLink: geometry('.vk-draft-row__link'), rowTitle: geometry('.vk-draft-row__title'), rowTitleLines: titleLines, rowMetadata: geometry('.vk-draft-row .vk-draft-meta'), rowOpen: geometry('.vk-draft-row__open'), results: geometry('#draft-results'), listHeading: geometry('#draft-list-title'), searchField: geometry('.vk-draft-search > .vk-ui-field'), sortField: geometry('#draft-tools > .vk-ui-field:nth-child(2)') },
        reader: { title: geometry('#reader-header h1'), header: geometry('#reader-header'), body: geometry('.vk-reader .vk-document') },
        editor: { status: document.querySelector('[data-testid=save-status]')?.dataset.status, textSetting: document.documentElement.dataset.text,
          toolbar: geometry('#editor-toolbar'), statusReadout: geometry('[data-testid=save-status]'), save: geometry('#save'), tools: geometry('#editor-save-details > summary'),
          paper: geometry('.vk-editor-paper'), title: geometry('#draft-title'), body: geometry('[data-testid=editor-body]'), firstLine: geometry('.cm-line'),
          fonts: { ui: document.fonts.check('16px "Be Vietnam Pro"'), reading: document.fonts.check('16px Literata') },
          colors: Object.fromEntries(['.vk-ui-scaffold','.vk-editor-paper','#editor-toolbar','[data-testid=save-status]','#editor-save-details > summary'].flatMap(selector => {
            const el=document.querySelector(selector); if (!el) return []; const s=getComputedStyle(el); return [[selector,{color:s.color,background:s.backgroundColor}]];
          })), preview: geometry('#draft-preview[open]'), leave: geometry('#leave-editor[open]') },
        explore: { heading: geometry('#explore-heading'), toolbar: geometry('.vk-discovery-toolbar'), tools: geometry('#discovery-tools'), brand: geometry('.vk-brand'), search: geometry('#public-search'), input: geometry('#public-query'), label: geometry('#public-query-label'), shell: geometry('#public-search .vk-ui-search__shell'), clear: geometry('#clear-public-search'), submit: geometry('#public-search button[type=submit]'), stream: geometry('#community-stream'), firstRow: geometry('.vk-public-row'), byline: geometry('.vk-public-row__byline'), author: geometry('.vk-public-row__byline .vk-ui-byline__name'), title: geometry('.vk-public-row__title'), excerpt: geometry('.vk-public-row__excerpt'), context: geometry('#context') },
        navigation: {
          bar: geometry('.vk-ui-bottom-nav'),
          barHeightObserver: Number.isFinite(barVar) && barRect ? { cssVarHeight: barVar, measuredHeight: barRect.height, delta: Math.abs(barVar - barRect.height) } : null,
          labels: navSnapshot('.vk-ui-bottom-nav'),
          rail: navSnapshot('#navigation'),
        },
        targets: { required: requiredResults },
      };
    })()`);
    const manifest = join(dir, 'contexts.json');
    const contexts = readdirSync(dir).includes('contexts.json') ? JSON.parse(readFileSync(manifest, 'utf8')) : {};
    const missingTargets = (context.targets?.required ?? []).filter((t) => !t.present || !t.visible);
    const captureStatus = missingTargets.length ? 'INCOMPLETE' : 'CAPTURED';
    const targetError = missingTargets.length
      ? `missing or invisible target(s): ${missingTargets.map((t) => t.selector).join(', ')}`
      : null;
    if (!evidence.skipSceneValidation) {
      validateCaptureScene({
        name,
        requestedScene: evidence.requestedScene ?? evidence.scene ?? evidence.viewport,
        editorScene: evidence.editorScene,
        actual: context,
        pngMeta,
      });
    }
    const textScaleLane = classifyTextScaleLane({
      rootInlineFontSize: context.rootInlineFontSize,
      dataText: context.dataText,
      explicitLane: evidence.textScaleLane,
    });
    if (textScaleLane === 'dom-override-geometry-only') {
      if (EVIDENCE_STATE.lanes.domOverrideGeometryOnly === 'NOT_RUN') {
        EVIDENCE_STATE.lanes.domOverrideGeometryOnly = 'PASS';
      }
    }
    const record = {
      schemaVersion: MANIFEST_SCHEMA_VERSION,
      scenarioId: evidence.scenario ?? evidence.sceneId ?? name,
      captureStatus,
      targetError,
      state: evidence.state ?? (context.hydrated ? 'hydrated' : 'before-hydration'),
      viewport: {
        width: context.width,
        height: context.height,
        layoutWidth: context.layoutWidth,
        clientWidth: context.clientWidth,
        clientHeight: context.clientHeight,
        dpr: context.dpr,
      },
      requestedScene: evidence.requestedScene ?? evidence.scene ?? evidence.editorScene ?? evidence.viewport ?? null,
      textScaleLane,
      boundary: evidence.boundary ?? null,
      source: SOURCE_IDENTITY,
      png: pngMeta,
      ...context,
      captured_at: new Date().toISOString(),
      evidence: { build: BUILD_SHA, dirty: BUILD_DIRTY, textScaleLane, ...evidence },
    };
    contexts[name] = record;
    writeFileSync(manifest, JSON.stringify(contexts, null, 2));
    writeFileSync(
      join(dir, 'evidence-summary.json'),
      JSON.stringify(
        buildLaneSummary({
          phase: args.phase,
          captures: contexts,
          lanes: EVIDENCE_STATE.lanes,
          negativeControls: EVIDENCE_STATE.negativeControls,
        }),
        null,
        2,
      ),
    );
    if (captureStatus !== 'CAPTURED' && !evidence.allowIncomplete) {
      throw new Error(`capture incomplete for ${name}: ${targetError}`);
    }
    return record;
  }

  async click(selector, modifiers = 0) {
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
      await this.send('Input.dispatchMouseEvent', { type, x: point.x, y: point.y, button: 'left', clickCount: 1, modifiers });
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
      await this.eval(`document.querySelector('#draft-title, #outline-toggle, #editor-save-details > summary')?.focus()`);
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

function validateEditorCapture(name, scene, actual, pixels) {
  validateCaptureScene({
    name,
    editorScene: scene,
    actual,
    pngMeta: pixels,
  });
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
    this.getURLs = new Map();
    this.requests = [];
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
    } else {
      this.getURLs.set(requestId, request.url);
      this.requests.push(request.url);
      if (this.holdGets && (!this.holdWhen || this.holdWhen(request.url))) this.gets.push(requestId);
      else await this.answerGet(requestId);
    }
  }

  enable() {
    return this.page.send('Fetch.enable', { patterns: [
      { urlPattern: `${args.base}/api/v1/drafts`, requestStage: 'Request' },
      { urlPattern: `${args.base}/api/v1/drafts`, requestStage: 'Response' },
      { urlPattern: `${args.base}/api/v1/drafts?*`, requestStage: 'Request' },
      { urlPattern: `${args.base}/api/v1/drafts?*`, requestStage: 'Response' },
      { urlPattern: `${args.base}/api/v1/author/works*`, requestStage: 'Request' },
      { urlPattern: `${args.base}/api/v1/author/works*`, requestStage: 'Response' },
    ] });
  }

  async disable() {
    await this.page.send('Fetch.disable');
    this.page.cdp.listeners = this.page.cdp.listeners.filter(listener => listener !== this.listener);
    if (this.error) throw this.error;
  }

  async answerGet(requestId) {
    if (!this.list) return this.page.send('Fetch.continueRequest', { requestId });
    const params = new URL(this.getURLs.get(requestId)).searchParams;
    this.getURLs.delete(requestId);
    let body = this.list.body;
    if ((this.list.status ?? 200) === 200 && !this.list.raw) {
      // This is a transport/geometry fixture, not evidence for PostgreSQL semantics.
      const key = s => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().replace(/đ/g, 'd').trim().replace(/\s+/g, ' ');
      const q = key(params.get('q') ?? '');
      let rows = body.drafts.filter(d => key(d.title || 'Chưa đặt tên').includes(q));
      if (params.get('sort') === 'title') rows = [...rows].sort((a,b) => {
        const x = key(a.title || 'Chưa đặt tên'), y = key(b.title || 'Chưa đặt tên');
        return x < y ? -1 : x > y ? 1 : a.work_id.localeCompare(b.work_id);
      });
      const start = Number(params.get('cursor')?.split(':')[1] ?? 0), limit = Number(params.get('limit') ?? 100);
      body = { drafts: rows.slice(start, start + limit), ...(start + limit < rows.length ? { next_cursor: `fixture:${start + limit}` } : {}) };
    }
    return this.fulfill(requestId, this.list.status ?? 200, body);
  }

  fulfill(requestId, status, body) {
    if (body?.drafts) body = { works: body.drafts.map(d => ({ ...d, draft_version: d.version,
      publication: d.publication ?? { audience:'private', lifecycle:'active', generation:1,
        intent_revision_id:null, intent_moderation:null, latest_revision_id:null, latest_moderation:null,
        review_withdrawn:false, has_draft_changes:null, current_public:null } })), next_cursor: body.next_cursor ?? null };
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

async function launchChrome(existingProfile) {
  const profile = existingProfile ?? mkdtempSync(join(tmpdir(), 'vink-chrome-'));
  if (existingProfile) rmSync(join(profile, 'DevToolsActivePort'), { force: true });
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
  // Linux headless can report hover:none even when CDP injects mouse events. The navigation
  // mouse probe uses Blink's hover-capable fine pointer types (hover = 2, fine = 4).
  if (['main', 'bottom-navigation'].includes(args.phase)) {
    flags.push('--blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4');
  }
  // #234 measures a new SSR document with back_forward navigation timing, not a BFCache hit.
  if (['perf-baseline','reader-guest-history'].includes(args.phase)) flags.push('--disable-back-forward-cache');
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

async function openPage(cdp, browserContextId) {
  // `browserContextId` opens the tab in an isolated context: its own cookie jar, so two real
  // sessions (an author and a moderator) can be signed in at the same moment.
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank', ...(browserContextId ? { browserContextId } : {}) });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const page = new Page(cdp, sessionId);
  await page.send('Page.enable');
  await page.send('Runtime.enable');
  await page.send('Network.enable');
  await page.send('Accessibility.enable');
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
async function tracePublicReadLifecycle(page) {
  let phase = 'setup';
  const ledger = recordTransport(page, args.base, () => phase);
  const cases = [];
  let status = 'FAIL';
  try {
    await cacheFirstPublicReads(page, (label, injection = null) => {
      phase = label;
      cases.push({ phase, trigger_or_intent: label, injection });
    });
    status = 'PASS';
  } finally {
    ledger.stop();
    writeFileSync(join(dirname(args.state), 'public-read-lifecycle.json'), JSON.stringify({
      source_sha: BUILD_SHA, status, build_profile: args['build-profile'] ?? 'unknown',
      boundary: 'READ-02 main phase; real App/PG reads except tagged controls; browser attempts, not SQL or internal owner counts',
      cases, transport: ledger.rows,
    }, null, 2));
  }
}

async function cacheFirstPublicReads(page, note) {
  console.log('cache-first public reads (READ-02): snapshot first, lifecycle revalidation, failure and removal:');
  const worksPath = '/api/v1/works';
  const publicationPath = `/api/v1/works/${SAMPLE_WORK}/reader`;
  const original = await (await fetch(`${args.base}${publicationPath}`)).json();
  const seen = [];
  // ExtraInfo events may arrive before or after requestWillBeSent: merge by request id.
  const byId = new Map();
  const record = id => byId.get(id) ?? byId.set(id, {}).get(id);
  const network = ({ sessionId, method, params }) => {
    if (sessionId !== page.sessionId) return;
    if (method === 'Network.requestWillBeSent') {
      const parsed = new URL(params.request.url);
      const path = parsed.pathname;
      if (path === worksPath || path === publicationPath) {
        seen.push(Object.assign(record(params.requestId), { path, href: parsed.pathname + parsed.search }));
      }
    }
    if (method === 'Network.requestWillBeSentExtraInfo') record(params.requestId).ifNoneMatch = Object.entries(params.headers).find(([name]) => name.toLowerCase() === 'if-none-match')?.[1] ?? null;
    if (method === 'Network.responseReceivedExtraInfo') record(params.requestId).wire = params.statusCode;
  };
  const count = path => seen.filter(entry => entry.path === path).length;
  const countHref = href => seen.filter(entry => entry.href === href).length;
  // Fetch-domain fixtures: undefined = continue to the real server, 'hold' = keep pending.
  const fixtures = new Map();
  const held = [];
  const gate = ({ sessionId, method, params }) => {
    if (sessionId !== page.sessionId || method !== 'Fetch.requestPaused') return;
    const parsed = new URL(params.request.url);
    const entry = fixtures.get(parsed.pathname + parsed.search) ?? fixtures.get(parsed.pathname);
    if (entry === 'hold') return held.push(params.requestId);
    const answer = typeof entry === 'function' ? entry(params) : entry;
    if (!answer) {
      page.send('Fetch.continueRequest', { requestId: params.requestId }).catch(() => {});
      return;
    }
    const headers = [
      ...(answer.status === 304 ? [] : [{ name: 'Content-Type', value: answer.contentType ?? 'application/json' }]),
      { name: 'Cache-Control', value: 'no-store' },
      ...(answer.headers ?? []),
    ];
    const req = { requestId: params.requestId, responseCode: answer.status, responseHeaders: headers };
    if (answer.status !== 304) {
      const payload = answer.rawBody !== undefined ? answer.rawBody : JSON.stringify(answer.body);
      req.body = Buffer.from(payload).toString('base64');
    }
    page.send('Fetch.fulfillRequest', req).catch(() => {});
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
    note('fresh-discovery-ssr');
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
    note('initial-client-discovery');
    const first = await timed(nav, ROWS);
    check(count(worksPath) === 1 && first.loading, `a first client visit fetches once and shows its loading state (${first.ms} ms to rows)`);
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${original.revision_id}'`, 'reader by client navigation');
    note('fresh-client-return');
    const back = await timed('#reader-return', ROWS);
    await sleep(300);
    check(count(worksPath) === 1 && !back.loading, `returning within the stale threshold renders the snapshot at once with no request and no loading state (${back.ms} ms to rows)`);
    console.log(`  ..  time to rows: first client visit ${first.ms} ms (network), return ${back.ms} ms (tab snapshot)`);

    await page.eval(`document.querySelector('.vk-public-row').__kept = true; document.querySelector('.vk-public-row__link').focus()`);
    // Deliberately exceed the former one-second timing margin on even a fast host.
    await sleep(1500);
    note('fresh-lifecycle-signal', 'clock +119 seconds');
    await advance(119);
    await signals(['focus']);
    await sleep(300);
    check(count(worksPath) === 1, '119 s after validation a focus does not revalidate');
    note('stale-lifecycle-burst', 'clock +2 seconds; real conditional App response');
    await advance(2);
    await signals(['focus', 'online', 'visibilitychange', 'pageshow']);
    await settled('#public-collection', 'list revalidation');
    await sleep(300);
    const revalidation = seen.filter(entry => entry.path === worksPath).at(-1);
    check(count(worksPath) === 2, `four simultaneous lifecycle signals at 121 s share one request (${count(worksPath) - 1} sent)`);
    check(revalidation.ifNoneMatch && revalidation.wire === 304, `the revalidation used the browser's stored validator and the origin answered 304 (${JSON.stringify(revalidation)})`);
    check(await page.eval(`document.querySelector('.vk-public-row').__kept === true && document.activeElement?.matches('.vk-public-row__link')`), 'an unchanged refresh keeps the same row elements and focus');

    note('hidden-and-stale', 'visibility and clock');
    await setHidden(true);
    await advance(300);
    await signals(['focus', 'online']);
    await sleep(800);
    check(count(worksPath) === 2, 'a hidden document does not revalidate on focus/online and pauses the timer');
    note('becomes-visible');
    await setHidden(false);
    await settled('#public-collection', 'revalidation after becoming visible');
    await sleep(300);
    check(count(worksPath) === 3, 'becoming visible with a stale snapshot sends exactly one request');

    await page.send('Fetch.enable', { patterns: [{ urlPattern: `${args.base}/api/v1/works*`, requestStage: 'Request' }] });
    note('stale-refresh-failure', 'injected list 503 and Retry-After');
    fixtures.set(worksPath, {
      status: 503,
      headers: [{ name: 'Retry-After', value: '30' }],
      body: { error: { code: 'dependency_unavailable' } },
    });
    await advance(121);
    await signals(['focus']);
    await page.waitFor(`document.querySelector('[data-testid=public-refresh-failed]')`, 'refresh failure notice');
    const afterFailureCount = count(worksPath);
    check(await page.eval(`document.querySelector('.vk-public-row').__kept === true && !document.querySelector('[data-testid=public-unavailable], [data-testid=public-empty]') && document.querySelector('#public-collection').getAttribute('aria-busy') === 'false'`), 'a temporary refresh failure keeps the last success on screen, separate from the unavailable/empty states');
    // AC-02: focus/online/visibilitychange before Retry-After deadline sends zero new requests.
    note('retry-after-backoff', 'clock +10 seconds');
    await advance(10);
    await signals(['focus', 'online', 'visibilitychange']);
    await sleep(350);
    check(count(worksPath) === afterFailureCount, 'Retry-After backoff refuses focus/online/visibility revalidation before retry_at');
    // Once retry_at passes, focus revalidates once and clears failure notice without remount.
    fixtures.delete(worksPath);
    note('retry-after-elapses', 'clock +25 seconds, then real App');
    await advance(25);
    await signals(['focus']);
    await page.waitFor(`!document.querySelector('[data-testid=public-refresh-failed]') && document.querySelector('#public-collection').dataset.refreshing === 'false'`, 'post-backoff signal recovers cleanly');
    check(count(worksPath) === afterFailureCount + 1 && await page.eval(`document.querySelector('.vk-public-row').__kept === true`), 'backoff clears after retry_at and preserves mounted rows');

    // Manual retry still bypasses stale/backoff and clears failure notice.
    note('failure-before-manual', 'injected list 503 and Retry-After');
    fixtures.set(worksPath, { status: 503, headers: [{ name: 'Retry-After', value: '60' }], body: { error: { code: 'dependency_unavailable' } } });
    await advance(121);
    await signals(['focus']);
    await page.waitFor(`document.querySelector('[data-testid=public-refresh-failed]')`, 'second refresh failure for manual retry');
    await page.capture('explore-refresh-failed');
    fixtures.delete(worksPath);
    note('manual-list-retry', 'real App after injected failure');
    await page.click('[data-testid=public-refresh-failed] button');
    await page.waitFor(`!document.querySelector('[data-testid=public-refresh-failed]') && document.querySelector('#public-collection').dataset.refreshing === 'false'`, 'manual retry recovers');
    check(await page.eval(`document.querySelector('.vk-public-row').__kept === true`), 'manual retry restores freshness without remounting the list');

    // AC-06: Raw 304 on an uncached route triggers at most one unconditional reload (`200`), and double 304 stops cleanly.
    const clientNavigate = href => page.eval(`(() => { const a = document.createElement('a'); a.href = ${JSON.stringify(href)}; document.body.append(a); a.click(); a.remove(); })()`);
    const raw304Query = '/api/v1/works?q=raw304';
    let raw304Once = true;
    fixtures.set(raw304Query, () => {
      if (!raw304Once) return undefined;
      raw304Once = false;
      return { status: 304 };
    });
    note('raw-304-fallback', 'first request injected 304');
    await clientNavigate('/?q=raw304');
    await page.waitFor(`new URL(location.href).searchParams.get('q') === 'raw304' && document.querySelector('[data-testid=public-no-results]')`, 'raw 304 recovers via single unconditional reload');
    check(countHref(raw304Query) === 2, 'raw 304 triggers at most one unconditional fallback reload');

    const double304Query = '/api/v1/works?q=double304';
    fixtures.set(double304Query, { status: 304 });
    note('double-304-stops', 'two injected 304 responses');
    await clientNavigate('/?q=double304');
    await page.waitFor(`new URL(location.href).searchParams.get('q') === 'double304' && document.querySelector('[data-testid=public-unavailable]')`, 'double raw 304 fails finitely');
    await advance(130);
    await signals(['focus']);
    await sleep(250);
    check(countHref(double304Query) === 2, 'double raw 304 stops at two requests without infinite loop');
    fixtures.delete(raw304Query);
    fixtures.delete(double304Query);
    await clientNavigate('/');
    await page.waitFor(`location.pathname === '/' && location.search === '' && document.querySelector('#public-collection')`, 'return to Explore before reader checks');

    // #202 transport fixture: route ABA and disposal must fence a delayed Published A.
    const raceA = '77777777-7777-4777-8777-777777777701';
    const raceB = '77777777-7777-4777-8777-777777777702';
    const racePathA = `/api/v1/works/${raceA}/reader`;
    const racePathB = `/api/v1/works/${raceB}/reader`;
    fixtures.set(racePathA, 'hold');
    fixtures.set(racePathB, { status: 200, body: { ...original, status: 'published', work_id: raceB, revision_id: 'race-b', title: '[Fixture] B' } });
    note('late-reader-aba', 'held Published A; injected B and new Hidden A');
    await clientNavigate(`/tac-pham/${raceA}`);
    const raceDeadline = Date.now() + 10000;
    while (!held.length && Date.now() < raceDeadline) await sleep(20);
    check(held.length === 1, 'first A read is deferred');
    const oldA = held.pop();
    await clientNavigate(`/tac-pham/${raceB}`);
    await page.waitFor(`document.querySelector('#reader-paper')?.dataset.revision === 'race-b'`, 'B completes before A');
    fixtures.set(racePathA, { status: 200, body: { status: 'hidden', work_id: raceA } });
    await clientNavigate(`/tac-pham/${raceA}`);
    await page.waitFor(`document.querySelector('[data-testid=reader-hidden]')`, 'new A receives authoritative Hidden instead of joining disposed A');
    await page.send('Fetch.fulfillRequest', { requestId: oldA, responseCode: 200,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify({ ...original, status: 'published', work_id: raceA, revision_id: 'old-a', title: '[Fixture] Obsolete A' })).toString('base64') }).catch(() => {});
    await sleep(150);
    check(await page.eval(`!!document.querySelector('[data-testid=reader-hidden]') && !document.querySelector('.vk-document') && !document.title.includes('Obsolete A') && !document.querySelector('link[rel=canonical]')`), 'late Published after Hidden cannot revive body/title/metadata across A→B→A');
    fixtures.set(racePathA, { status: 200, body: { ...original, status: 'published', work_id: raceA, revision_id: 'new-a', title: '[Fixture] New A' } });
    await clientNavigate(`/tac-pham/${raceB}`);
    await page.waitFor(`document.querySelector('#reader-paper')?.dataset.revision === 'race-b'`, 'leave hidden A');
    await clientNavigate(`/tac-pham/${raceA}`);
    await page.waitFor(`document.querySelector('#reader-paper')?.dataset.revision === 'new-a'`, 'new authoritative publication replaces control without retained no-store state');
    fixtures.delete(racePathA); fixtures.delete(racePathB);
    await clientNavigate('/');
    await page.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row__link')`, 'return after ABA fixture');

    // #202: reopen offline within the mount-age window without a successful /reader gate.
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${original.revision_id}'`, 'prime cached Reader');
    await settled('#reader-freshness', 'prime Reader checked');
    await page.click('#reader-return');
    await page.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row__link')`, 'leave primed Reader');
    await advance(121);
    fixtures.set(publicationPath, 'hold');
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${original.revision_id}' && document.querySelector('[data-testid=reader-cached]')`, 'cached Reader opens before control verification');
    await page.capture('reader-cached-unverified', { scenario: '#202-cached-before-network', boundary: 'held-reader-response' });
    const pendingControlDeadline = Date.now() + 10000;
    while (!held.length && Date.now() < pendingControlDeadline) await sleep(20);
    check(held.length === 1, 'cached Reader has one pending control verification');
    await page.send('Fetch.fulfillRequest', { requestId: held.pop(), responseCode: 503,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify({ error: { code: 'unavailable' } })).toString('base64') });
    fixtures.set(publicationPath, { status: 503, body: { error: { code: 'unavailable' } } });
    check(await page.eval(`!!document.querySelector('.vk-document') && !document.querySelector('[data-testid=public-unavailable]')`), 'offline revisit renders the retained full snapshot before /reader succeeds');
    await page.waitFor(`document.querySelector('[data-testid=reader-refresh-failed]')`, 'cached Reader reports failed verification');
    for (const width of [390, 1440]) for (const text of [100, 200]) {
      await page.send('Emulation.setDeviceMetricsOverride', { width, height: 900, deviceScaleFactor: 1, mobile: false });
      await page.eval(`document.documentElement.style.fontSize = '${text}%'`);
      await page.capture(`reader-cache-${width}-${text}`, { scenario: '#202-cached-unverified', viewport: { width, height: 900 }, textScale: text, boundary: 'Chrome-CDP-delayed-transport' });
      check(await page.eval(`document.documentElement.scrollWidth <= innerWidth + 1`), `cached Reader reflows at ${width}px / text ${text}%`);
    }
    await page.eval(`document.documentElement.style.fontSize = ''`);
    await page.send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
    await page.click('#reader-return');
    await page.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row__link')`, 'leave Reader before true offline revisit');
    note('offline-revisit', 'CDP browser transport offline');
    await page.send('Network.emulateNetworkConditions', { offline: true, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${original.revision_id}'`, 'Reader snapshot opens with browser transport offline');
    check(await page.eval(`document.querySelector('.vk-document').textContent.includes(${JSON.stringify(PUBLISHED_LINE)})`), 'browser offline revisit needs no successful /reader response');
    await page.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
    fixtures.delete(publicationPath);
    note('online-manual-reader-retry');
    await page.click('[data-testid=reader-refresh-failed] button');
    await settled('#reader-freshness', 'Reader restored online');
    await page.click('#reader-return');
    await page.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row__link')`, 'back to Explore');
    await settled('#public-collection', 'Explore snapshot confirmed before the removal race');

    // Reader: a changed representation is offered, never swapped mid-reading; a 404 removes it.
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${original.revision_id}'`, 'reader again');
    await settled('#reader-freshness', 'reader revalidation on mount');
    const updated = { ...original, revision_id: '[mẫu-kiểm-tra]-revision', title: '[Mẫu kiểm tra] Mưa trên phố cũ (bản cập nhật)', document: defaultDocument('Khổ thay thế chỉ có trong fixture kiểm tra.') };
    fixtures.set(publicationPath, { status: 200, body: { ...updated, status: "published" } });
    await page.waitFor(`document.querySelector('#root-composer')`, 'Reader layout loaded before choosing a reading anchor');
    await page.eval(`window.__readingArticle = document.querySelector('#reader-paper'); window.__readingAnchor = document.querySelector('.vk-document'); const r = document.createRange(); r.selectNodeContents(window.__readingAnchor); const selection = getSelection(); selection.removeAllRanges(); selection.addRange(r); window.__readingSelection = selection.toString(); scrollTo({top:scrollY + window.__readingAnchor.getBoundingClientRect().top - 80, behavior:'instant'})`);
    await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
    await page.eval(`window.__readingAnchorY = window.__readingAnchor.getBoundingClientRect().top`);
    const anchorStart = await page.eval(`({scroll:scrollY, top:window.__readingAnchorY, height:innerHeight})`);
    check(anchorStart.scroll > 0 && anchorStart.top >= 0 && anchorStart.top < anchorStart.height / 2, `anchor test starts inside the article, away from scroll position zero: ${JSON.stringify(anchorStart)}`);
    await advance(121);
    await signals(['focus']);
    await page.waitFor(`document.querySelector('[data-testid=reader-update]')`, 'new version notice');
    check(await page.eval(`document.querySelector('.vk-reader').dataset.revision === '${original.revision_id}' && document.querySelector('.vk-document').textContent.includes(${JSON.stringify(PUBLISHED_LINE)})`), 'a changed publication is offered without replacing the text being read');
    await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
    check(await page.eval(`document.querySelector('#reader-paper') === window.__readingArticle && getSelection().toString() === window.__readingSelection && Math.abs(window.__readingAnchor.getBoundingClientRect().top - window.__readingAnchorY) <= 1`), 'background Reader update keeps article identity, selection and the visible paragraph anchor');
    await page.eval(`getSelection().removeAllRanges()`);
    await page.waitFor(`document.querySelector('#root-composer')`, 'composer ready for reconnect');
    await page.typeAtEnd('#root-composer', 'Chữ đang soạn chưa gửi');
    await page.send('Input.imeSetComposition', { text: 'đ', selectionStart: 1, selectionEnd: 1 });
    await page.eval(`window.__keptComposer = document.querySelector('#root-composer')`);
    await advance(121);
    const beforeBurst = count(publicationPath);
    await signals(['online', 'focus', 'visibilitychange', 'pageshow']);
    await settled('#reader-freshness', 'reconnect during simulated composition');
    check(count(publicationPath) === beforeBurst + 1, 'Reader lifecycle burst makes one logical flight');
    check(await page.eval(`document.querySelector('#root-composer') === window.__keptComposer && document.activeElement === window.__keptComposer && window.__keptComposer.value.includes('Chữ đang soạn chưa gửi')`), 'reconnect keeps composer node, focus and unsent Vietnamese input (CDP composition, not OS IME)');
    await page.send('Input.insertText', { text: 'đ' });
    await page.capture('reader-update-available');
    await page.click('[data-testid=reader-update] button');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${updated.revision_id}' && !document.querySelector('[data-testid=reader-update]')`, 'applied new version');
    check(await page.eval(`document.querySelectorAll('.vk-reader').length === 1 && document.querySelector('.vk-document').textContent.includes('Khổ thay thế') && !document.querySelector('.vk-document').textContent.includes(${JSON.stringify(PUBLISHED_LINE)})`), 'the reader applies the new version only on request, replacing the whole text once');
    await page.waitFor(`document.activeElement?.id === 'noi-dung'`, 'focus moves to the content start after applying');

    // The update/composition checks advance more than the 5-minute unobserved retention
    // before the next 404 signal. Confirm a new list snapshot for the separate purge race.
    await page.click('#reader-return');
    await page.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row__link')`, 'prime list for card revocation');
    await signals(['focus']);
    await settled('#public-collection', 'list confirmed for card revocation');
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${updated.revision_id}'`, 'return to updated Reader before revocation');
    fixtures.set(publicationPath, { status: 404, body: { error: { code: 'not_found' } } });
    note('reader-control-removal', 'injected 404 for both public projections');
    // Both public projections must agree while removed. A fresh Published /reader response
    // is authoritative recovery, so do not leave that route serving the real published fixture.
    const readerPath = `/api/v1/works/${SAMPLE_WORK}/reader`;
    fixtures.set(readerPath, { status: 404, body: { error: { code: 'not_found' } } });
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
    fixtures.delete(readerPath);
    note('origin-recovers-list', 'release held request to App');
    await page.send('Fetch.continueRequest', { requestId: held.pop() });
    await page.waitFor(`document.querySelector('a[href^="/tac-pham/${SAMPLE_WORK}"]')`, 'origin list restores the card');
    check(true, 'the origin’s list, not the derived preview, decides membership once it answers');
    await page.click('.vk-public-row__link');
    await page.waitFor(`document.querySelector('.vk-reader')?.dataset.revision === '${original.revision_id}'`, 're-publication recovers without permanent tombstone');
    check(await page.eval(`document.querySelector('.vk-document').textContent.includes(${JSON.stringify(PUBLISHED_LINE)})`), 'after prior 404 removal a subsequent fresh 200 restores the reader cleanly');

    // AC-01: Typed 422 / malformed error recovery without retrying bad keys or looping on non-cursor errors.
    const cursorQuery = '/api/v1/works?q=cursorerr&cursor=stale';
    fixtures.set(cursorQuery, {
      status: 422,
      body: { error: { code: 'validation_failed', field: 'cursor', message: 'opaque server message ignored' } },
    });
    note('typed-rejection', 'injected 422 and invalid envelope');
    await clientNavigate('/?q=cursorerr&cursor=stale');
    await page.waitFor(`document.querySelector('[data-testid=public-invalid][data-invalid-kind="cursor"]') && document.querySelector('#restart-public')`, '422 cursor recovery view');
    const cCount = countHref(cursorQuery);
    await advance(130);
    await signals(['focus', 'online']);
    await sleep(300);
    check(countHref(cursorQuery) === cCount, '422 cursor stops retrying even across focus/online/130s');
    await page.click('#restart-public');
    await page.waitFor(`new URL(location.href).searchParams.get('q') === 'cursorerr' && !new URL(location.href).searchParams.has('cursor') && document.querySelector('#public-results')`, 'clicking #restart-public drops cursor and recovers');

    const inputQuery = '/api/v1/works?q=!!!';
    fixtures.set(inputQuery, {
      status: 422,
      body: { error: { code: 'validation_failed', field: 'q', message: 'unrelated' } },
    });
    await clientNavigate('/?q=!!!');
    await page.waitFor(`document.querySelector('[data-testid=public-invalid][data-invalid-kind="q"]') && document.querySelector('#clear-invalid-filters') && !document.querySelector('#restart-public')`, '422 q renders input alert without cursor reset');
    const qCount = countHref(inputQuery);
    await advance(130);
    await signals(['focus', 'online']);
    await sleep(300);
    check(countHref(inputQuery) === qCount, '422 q never enters retry or cursor-reset loop');

    const fallbackQuery = '/api/v1/works?q=malformed422';
    fixtures.set(fallbackQuery, { status: 422, contentType: 'text/html', rawBody: '<html>not json</html>' });
    await clientNavigate('/?q=malformed422');
    await page.waitFor(`document.querySelector('[data-testid=public-invalid][data-invalid-kind="fallback"]') && !document.querySelector('#restart-public')`, 'malformed 422 stops in finite fallback');
    const fCount = countHref(fallbackQuery);
    await advance(130);
    await signals(['focus']);
    await sleep(250);
    check(countHref(fallbackQuery) === fCount, 'malformed 422 stops retrying');
    fixtures.clear();
    await page.click('[data-testid=public-invalid] + a');
    await page.waitFor(`location.pathname === '/' && location.search === '' && document.querySelector('#public-collection')`, 'return cleanly to unfiltered Explore');
  } finally {
    await page.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 }).catch(() => {});
    await page.eval(`if (window.__vinkClock) { performance.now = window.__vinkClock.realNow; delete window.__vinkClock; }`).catch(() => {});
    await page.send('Fetch.disable').catch(() => {});
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== network && item !== gate);
  }
}

async function serverHtml() {
  console.log('server HTML before any JavaScript:');
  const home = await fetch(args.base);
  const homeHtml = await home.text();
  check(home.status === 200 && homeHtml.includes('<title>Khám phá — Vink</title>') && homeHtml.includes('Trang này: 1 tác phẩm') && homeHtml.includes('vk-public-row__link'), 'Explore HTML contains its finite public collection and actual inventory before JavaScript');
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
    body: JSON.stringify({ expected_version: draft.version, title: privateTitle, document: defaultDocument(DRAFT_ONLY_LINE) }),
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
// Poll live transitions. Waiting on a snapshot of Animation.finished promises
// can stall DevTools after a dirty editor resize.
const APPEARANCE_SETTLED = `!document.getAnimations().some(a =>
  a instanceof CSSTransition && (a.pending || a.playState === 'running'))`;

async function openProfile(page) {
  await page.click('#profile-trigger');
  await page.waitFor(PROFILE_OPEN, 'Tôi overlay');
}

async function waitForPageAccount(page) {
  // Hydration can finish before the account panel above the choices changes height.
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' &&
    document.querySelector('#account-session')?.dataset.standing === 'active'`, 'hydrated Tôi account panel');
  await page.eval('document.fonts.ready');
}

async function chooseTheme(page, value, group = 'overlay-theme') {
  if (group === 'page-theme') await waitForPageAccount(page);
  const selector = `label:has(input[name="${group}"][value="${value}"])`;
  try {
    await page.click(selector);
    await page.waitFor(value === 'system' ? `!document.documentElement.hasAttribute('data-theme')` : `document.documentElement.dataset.theme === '${value}'`, `${value} appearance`);
  } catch (error) {
    const target = await page.eval(`(() => {
      const label = document.querySelector(${JSON.stringify(selector)});
      const rect = label?.getBoundingClientRect();
      const hit = rect && document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
      const choice = hit?.closest('label')?.querySelector('input');
      return { viewport: [innerWidth, innerHeight], scroll: [scrollX, scrollY], rect: rect?.toJSON(),
        hit: hit && { tag: hit.localName, id: hit.id, group: choice?.name, value: choice?.value } };
    })()`).catch(() => null);
    throw new Error(`${error.message}; ${selector} current hit test ${JSON.stringify(target)}`, { cause: error });
  }
  await page.waitFor(APPEARANCE_SETTLED, `${value} appearance transitions`);
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
    const toolbar=document.querySelector('.vk-discovery-toolbar').getBoundingClientRect();
    const heading=document.querySelector('#explore-heading h1');
    const search=document.querySelector('#public-search');
    const launcher=document.querySelector('#open-public-search');
    const searchTarget=(search && search.getClientRects().length > 0 ? search : launcher)?.getBoundingClientRect();
    return brand.width>=44 && brand.right<=innerWidth && toolbar.width>0 && toolbar.right<=innerWidth &&
      !!heading && heading.textContent.trim().length>0 && getComputedStyle(heading).display!=='none' &&
      !!searchTarget && searchTarget.width>0 && searchTarget.right<=innerWidth &&
      document.documentElement.scrollWidth<=innerWidth;
  })()`), scenario + ': brand and toolbar hierarchy reflow');
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
      manifest.icons.every(icon => new URL(icon.src, response.url).pathname.startsWith(`/pkg/brand/vpen-r1-mark-${theme}`) && icon.purpose === 'any') &&
      manifest.icons.some(icon => icon.sizes === '192x192') && manifest.icons.some(icon => icon.sizes === '512x512'), `${theme} manifest has stable identity and only R1 any-purpose 192/512 icons`);
    for (const icon of manifest.icons) {
      check((await fetch(new URL(icon.src, response.url))).ok, `${theme} manifest icon ${icon.src} exists`);
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

async function textContrast(page, theme, surface = 'shell', targets) {
  const selectors = targets ?? (surface.startsWith('editor-') ? (() => {
    const list = ['.vk-editor-heading__back', '[data-testid=save-status]', '#editor-save-details > summary', '#preview', '#draft-title', '#editor-body-label', '#editor-body-hint', '[data-testid=editor-body]'];
    if (surface === 'editor-dirty' || surface === 'editor-saving' || surface === 'editor-failed') list.push('#save');
    if (surface === 'editor-conflict') list.push('#save-recovery-jump');
    return list;
  })() : {
    outline: ['#editor-outline-title', '.vk-editor-outline__item--title', '#editor-outline-panel [aria-current="location"] button'],
    desk: ['.vk-desk-create .vk-ui-button', '.vk-draft-row__title', '.vk-draft-row .vk-draft-meta', '.vk-draft-row__open', '.vk-draft-search input', '#draft-sort', '#draft-sort-label', '#draft-scope'],
    explore: ['#public-query', '#explore-heading h1', '#open-tag-filters', '.vk-public-row__title', '.vk-public-row__byline .vk-ui-byline__name', '.vk-public-row__byline .vk-ui-avatar', '.vk-public-row__excerpt', '.vk-public-scope'],
    filter: ['#modal-tag-help', '#tag-selection-count', '#modal-tag-form legend', '#modal-tag-form .vk-ui-choice__label', '#modal-tag-form button[type=submit]'],
    empty: ['.vk-empty-state__message', '.vk-empty-state__actions .vk-ui-button'],
    'desk-empty': ['.vk-empty-state__message', '.vk-empty-state__actions .vk-ui-button', '.vk-desk-privacy'],
    reader: ['#reader-return', '#reader-header h1', '#reader-header .vk-work-byline', '#reader-header .vk-ui-type-caption', '.vk-reader .vk-block'],
    me: ['#me-title', '#page-account h2', '#page-account .vk-ui-tone-muted', '#page-appearance-title', '#page-theme-title', '#page-text-title', '#page-rail-title', ...['light', 'dark', 'system'].map(value => `#page-theme label:has([value="${value}"])`)],
    account: ['#profile-title', '#overlay-theme-title', '#overlay-text-title', '#overlay-rail-title', ...['light', 'dark', 'system'].map(value => `#overlay-theme label:has([value="${value}"])`)],
  }[surface] ?? ['#navigation-toggle', '#navigation-primary a', '#profile-trigger', '#profile-title', '#profile-menu .vk-ui-choice__target:has(:checked)', '#profile-menu .vk-ui-button']);
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
    const reservation = evaluateMePanelReservation(m);
    check(reservation.ok,
      `${scenario}: Me panel illustration reservation matches text padding (${reservation.reasons.join('; ')})`);
    if (context.width < 760) {
      check(near(m.surface.x, 16) && near(m.surface.width, context.layoutWidth - 32),
        `${scenario}: narrow Tôi keeps one 16px gutter on both sides`);
    }
  } else {
    check(m.surface.paddingLeft === m.surface.paddingRight && near(m.action.x, m.surface.contentLeft) &&
      m.action.x + m.action.width <= m.surface.contentLeft + m.surface.contentWidth + 1,
      `${scenario}: the panel action shares the content axis`);
  }
  const { preferences: p } = m;
  const inside = r => r && r.x >= p.surface.contentLeft - 1 &&
    r.x + r.width <= p.surface.contentLeft + p.surface.contentWidth + 1;
  check(near(p.radius, 28) && p.rows.every(row => [row.heading, row.group, ...row.targets].every(inside) &&
    row.inset === 0 && row.divider === 0),
    `${scenario}: all preference controls fit the inner rect (1px tolerance) without row dividers`);
  if (context.fontSize === '16px') {
    check(p.padding.every(value => near(value, 24)) && near(p.rowGap, 24) &&
      p.rows.every(row => near(row.gap, 8) && near(row.choiceGap, 8) && (row.before === null || near(row.before, 24))),
      `${scenario}: 24px gutter and group gap, 8px label and choice gaps at 100%`);
  }
}

function appearanceRows(page, prefix) {
  return page.eval(`(() => {
    const scope = document.querySelector(${JSON.stringify(prefix === 'page' ? '#me' : '#profile-menu')});
    const rows = [...scope.querySelectorAll('.vk-appearance-row')];
    const colors = document.createElement('canvas').getContext('2d');
    const root = getComputedStyle(document.documentElement);
    const asColor = value => { colors.fillStyle = value.trim(); return colors.fillStyle; };
    const selectedColors = ['--vk-primary-container', '--vk-on-primary-container'].map(name => asColor(root.getPropertyValue(name)));
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
          const rect = el.getBoundingClientRect(), selected = Boolean(el.querySelector(':checked'));
          return rect.width >= 48 && rect.height >= 48 && style.borderRadius === root.getPropertyValue('--vk-radius-full').trim() &&
            (!selected || (asColor(style.backgroundColor) === selectedColors[0] && asColor(style.color) === selectedColors[1])) &&
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

async function chooseTextSize(page, scale, group) {
  if (!await page.eval(`document.querySelector('#${group} input:checked')?.value === '${scale}'`)) {
    await page.click(`#${group} label:has([value="${scale}"])`);
  }
  await page.waitFor(`getComputedStyle(document.documentElement).fontSize === '${scale === 200 ? 32 : 16}px' &&
    document.querySelector('#${group} input:checked')?.value === '${scale}' && ${APPEARANCE_SETTLED}`, `${group} applies ${scale}% text`);
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
  console.log('shared appearance surfaces and native build disclosure:');
  for (const theme of ['light', 'dark']) {
    await page.viewport(390, 844);
    await chooseTheme(page, theme, 'page-theme');
    await page.waitFor(`document.querySelector('#page-account [data-kind=account]')?.dataset.initials === 'TT'`, 'current account name loaded');
    await page.eval(`document.querySelector('#page-theme input:checked').focus()`);
    await page.key('Space');
    await appearanceFocusContrast(page, theme);
    await page.capture(`r22-me-${theme}-selected-focus`);
    check(await appearanceRows(page, 'page'), `${theme}: page uses three compact native groups with icons and selection checks`);
    check(await page.eval(`document.querySelectorAll('#build-info').length === 0 &&
      document.querySelector('#page-account h2').textContent === 'Tài khoản' &&
      document.querySelector('#page-account p').textContent === 'Tác giả mẫu (dữ liệu phát triển)' &&
      document.querySelector('#page-account .vk-account-illustration').getAttribute('aria-hidden') === 'true'`),
      `${theme}: truthful account without prototype footer banner`);
  }

  await page.send('Network.setBlockedURLs', { urls: [`${args.base}/pkg/vink.js*`, `${args.base}/pkg/vink_bg.wasm*`] });
  await page.send('Emulation.setScriptExecutionDisabled', { value: true });
  const errorsBefore = page.errors.length;
  try {
    await page.goto(`${args.base}/toi`);
    check(await page.eval(`document.documentElement.dataset.hydrated!=='true' && document.querySelectorAll('#build-info').length===0 && document.querySelector('#page-account h2')?.textContent==='Tài khoản' && document.querySelector('#page-theme input[type=radio]')!==null`), 'SSR /toi renders without page JavaScript or build-info footer');
  } finally {
    await page.send('Emulation.setScriptExecutionDisabled', { value: false });
    await page.send('Network.setBlockedURLs', { urls: [] });
    const blocked = page.errors.splice(errorsBefore);
    check(blocked.every(error => /Failed to fetch dynamically imported module|Importing a module script failed|ERR_BLOCKED_BY_CLIENT/.test(error)), 'no-JS disclosure check produces only expected blocked imports');
  }
  await page.goto(`${args.base}/toi`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'hydration restored after the native build detail');
}

async function shortAppearanceStart(page) {
  for (const height of [320, 280]) {
    await page.viewport(834, height);
    await openProfile(page);
    const opened = await page.eval(`(() => {
      const panel = document.querySelector('#profile-menu');
      const { top, bottom } = panel.getBoundingClientRect();
      const visible = selector => {
        const r = panel.querySelector(selector).getBoundingClientRect();
        return r.top >= top && r.bottom <= bottom;
      };
      return { scrollTop: panel.scrollTop, title: visible('#profile-title'),
        close: visible('#profile-close'), focused: document.activeElement.id };
    })()`);
    check(opened.scrollTop <= 1 && opened.title && opened.close && opened.focused === 'profile-close',
      `834×${height}/200%: opening keeps the title and focused Close visible ${JSON.stringify(opened)}`);
    await page.capture(`account-overlay-834-${height}-compact-dark-text200-start`);
    await page.key('Tab');
    check(await page.eval(`(() => {
      const choice = document.querySelector('#overlay-theme input[value=dark]:checked');
      if (document.activeElement !== choice) return false;
      const r = choice.getBoundingClientRect();
      const panel = document.querySelector('#profile-menu').getBoundingClientRect();
      return r.top >= panel.top && r.bottom <= panel.bottom &&
        choice === document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
    })()`), `834×${height}/200%: Tab reveals and focuses the selected radio`);
    await page.key('Escape');
    check(await page.eval(`!${PROFILE_OPEN} && document.activeElement.id === 'profile-trigger'`),
      `834×${height}/200%: Escape returns focus to Tôi`);
  }
  await page.viewport(834, 360);
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
  check(await page.eval(`(() => {
    const rail = document.querySelector('#navigation').getBoundingClientRect();
    const panel = document.querySelector('#profile-menu').getBoundingClientRect();
    return document.activeElement === document.querySelector('#overlay-rail input:checked') &&
      Math.abs(panel.left - rail.right - 16) <= 1;
  })()`), 'changing the rail moves the open panel without moving radio focus');
  await page.key('Escape');
  check(await page.eval(`document.activeElement?.id === 'profile-trigger'`), 'Escape returns to the trigger on /toi');
  await page.reload();
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' &&
    document.querySelector('#page-text input:checked')?.value === '200' &&
    document.querySelector('#page-rail input:checked')?.value === 'compact'`, 'text and rail choices survive reload');
  await waitForPageAccount(page);
  await page.click('#page-text label:has([value="100"])');
  await page.waitFor(`!document.documentElement.dataset.text`, '100% text applied before changing the rail');
  await page.click('#page-rail label:has([value="auto"])');
  await page.waitFor(`!document.documentElement.dataset.text && !document.documentElement.dataset.rail &&
    localStorage.getItem('vink.ui.text') === null && localStorage.getItem('vink.ui.rail') === null &&
    ${RAIL_WIDTH} === 220 && document.querySelector('#overlay-rail input:checked')?.value === 'auto'`, '100% and automatic clear overrides and sync the panel');
  await chooseTheme(page, 'system', 'page-theme');
  await page.eval(`document.querySelector('#page-theme input:checked').focus()`);
  await page.key('ArrowRight');
  await page.waitFor(`document.querySelector('#page-theme input:checked')?.value === 'light' && document.documentElement.dataset.theme === 'light'`, 'native Arrow updates page preference');
  await page.click('#page-text label:has([value="200"])');
  await page.waitFor(`document.documentElement.dataset.text === '200' && getComputedStyle(document.documentElement).fontSize === '32px' && localStorage.getItem('vink.ui.text') === '200'`, 'real 200% control applies root font scale on /toi');
  const meOverflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript()));
  check(meOverflow.ok, `AC-04 real-control 200% /toi has no unintended overflow (${meOverflow.reasons.join('; ')})`);
  await page.click('.vk-ui-rail a[href="/"]');
  await page.waitFor(`location.pathname === '/' && document.documentElement.dataset.text === '200' && getComputedStyle(document.documentElement).fontSize === '32px'`, '200% preference persists across client route to /');
  const exploreOverflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript()));
  check(exploreOverflow.ok, `AC-04 real-control 200% / has no unintended overflow (${exploreOverflow.reasons.join('; ')})`);
  await page.click('.vk-ui-rail a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('#create-draft') && document.documentElement.dataset.text === '200' && getComputedStyle(document.documentElement).fontSize === '32px'`, '200% preference persists across client route to /ban-viet');
  await page.reload();
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.documentElement.dataset.text === '200' && getComputedStyle(document.documentElement).fontSize === '32px'`, '200% preference survives reload on /ban-viet');
  await page.goto(`${args.base}/tac-pham/${SAMPLE_WORK}`);
  await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.documentElement.dataset.text === '200' && getComputedStyle(document.documentElement).fontSize === '32px'`, '200% preference persists to reader route');
  await openProfile(page);
  check(await page.eval(`(() => {
    const checked = document.querySelector('#overlay-text input:checked');
    const face = checked?.closest('label');
    return checked?.value === '200' && Boolean(face) && getComputedStyle(face, '::after').opacity === '1';
  })()`), 'work route overlay preserves selected 200% control and visible ::after check');
  const workOverlayAx = await page.send('Accessibility.getFullAXTree');
  check(workOverlayAx.nodes.some(node => !node.ignored && node.role?.value === 'radio' && node.name?.value === '200%' &&
    node.properties?.some(prop => prop.name === 'checked' && String(prop.value?.value) === 'true')),
    'Chrome AX exposes checked=true on 200% radio after multi-route persistence and reload');
  const workOverflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({ containers: ['#profile-menu'] })));
  check(workOverflow.ok, `AC-04 real-control 200% work route has no unintended overflow (${workOverflow.reasons.join('; ')})`);
  await page.capture('appearance-control-200-work', {
    scenario: 'AC-04-app-appearance-control',
    state: 'text-200-route-persisted',
    textScaleLane: 'app-appearance-control',
    requestedScene: { width: 1440, height: 900, scale: 200, theme: 'light' },
    requiredTargets: ['#profile-menu', '#overlay-text'],
    boundary: 'real-local-postgresql-with-fixture-identity',
  });
  await page.click('#overlay-text label:has([value="100"])');
  await page.waitFor(`!document.documentElement.dataset.text && getComputedStyle(document.documentElement).fontSize === '16px' && localStorage.getItem('vink.ui.text') === null`, 'restoring 100% via real control clears text scale');
  await page.key('Escape');
  await page.waitFor(`!${PROFILE_OPEN}`, 'close appearance overlay after real-control persistence test');
  EVIDENCE_STATE.lanes.appAppearanceControl = 'PASS';
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
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  const visible = nodes.filter(node => !node.ignored);
  const hasName = (role, name) => visible.some(node => node.role.value === role && node.name?.value === name);
  const checked = visible.filter(node => node.role.value === 'radio' &&
    node.properties?.some(prop => prop.name === 'checked' && String(prop.value.value) === 'true'));
  check(hasName('dialog', 'Giao diện') && hasName('button', 'Đóng Giao diện') && hasName('button', 'Mở trang Tôi') &&
    ['Chế độ màu', 'Cỡ chữ', 'Thanh điều hướng'].every(name => hasName('radiogroup', name)) &&
    ['Hệ thống', 'Sáng', 'Tối', '100%', '200%', 'Tự động', 'Mở rộng', 'Thu gọn'].every(name => hasName('radio', name)) &&
    checked.length === 3 && ['Sáng', '100%', 'Tự động'].every(name => checked.some(node => node.name?.value === name)),
    'Chrome AX exposes the named modal, actions and full native radio groups with checked state');
  await chooseTheme(page, 'system');
  const choicePositions = () => page.eval(`[...document.querySelectorAll('#overlay-theme .vk-ui-choice__label')].map(el => {
    const { x, y, width, height } = el.getBoundingClientRect();
    return [x, y, width, height];
  })`);
  const beforeArrow = await choicePositions();
  await page.eval(`document.querySelector('#overlay-theme input:checked').focus()`);
  await page.key('ArrowRight');
  await page.waitFor(`document.querySelector('#overlay-theme input:checked')?.value === 'light' && document.documentElement.dataset.theme === 'light'`, 'native Arrow updates utility preference');
  check(JSON.stringify(await choicePositions()) === JSON.stringify(beforeArrow),
    'selection keeps each theme label in place');
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
  // CDP media/key acknowledgments do not guarantee style recalculation has settled.
  // Keep every focus/check/motion condition, with a bounded wait and a second-frame check.
  const forcedColorsReady = `(() => { const input = document.activeElement, face = input.closest('label'), style = getComputedStyle(face); return input.checked && input.matches(':focus-visible') && style.forcedColorAdjust === 'none' && style.outlineStyle === 'solid' && style.outlineWidth === '2px' && parseFloat(style.transitionDuration) <= 0.001 && getComputedStyle(face, '::after').opacity === '1' && [...document.querySelectorAll('#overlay-theme label')].every(el => getComputedStyle(el, '::before').forcedColorAdjust === 'none'); })()`;
  await page.waitFor(forcedColorsReady, 'forced-colors retains selected check and keyboard focus; reduced-motion reduces transitions below 1ms', 2000);
  await editorLayoutReady(page);
  check(await page.eval(forcedColorsReady), 'forced-colors retains selected check and keyboard focus; reduced-motion reduces transitions below 1ms');
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
        await openProfile(page);
        await chooseTextSize(page, scale, 'overlay-text');
        await chooseTheme(page, theme);
        await page.key('Escape');
        await openProfile(page);
        const scenario = `${width}-${height}-${rail}-${theme}-text${scale}`;
        check(await page.eval(`(() => { const rail = document.querySelector('.vk-ui-rail'); return rail.scrollWidth <= rail.clientWidth; })()`), `${scenario}: the rail needs no horizontal scrolling`);
        check(await page.eval(`(() => {
          const dialog = document.querySelector('#profile-menu'), r = dialog.getBoundingClientRect();
          return dialog.matches(':modal') && r.left >= document.querySelector('.vk-ui-rail').getBoundingClientRect().right &&
            r.right <= innerWidth && r.top >= 0 && r.bottom <= innerHeight &&
            dialog.scrollWidth <= dialog.clientWidth && document.documentElement.scrollWidth <= innerWidth &&
            document.querySelector('#profile-trigger').getAttribute('aria-expanded') === 'true';
        })()`), `${scenario}: modal stays beside the rail, within the viewport, without internal horizontal overflow`);
        check(await page.eval(`(() => {
          const panel = document.querySelector('#profile-menu'), rect = panel.getBoundingClientRect();
          const title = document.querySelector('#profile-title').getBoundingClientRect();
          return panel.scrollTop <= 1 && title.top >= rect.top && title.bottom <= rect.bottom &&
            document.activeElement === panel.querySelector('#overlay-theme input:checked');
        })()`), `${scenario}: title and initial radio focus are present at the start of the panel`);
        if (height === 900 && width === 1440 && scale === 100 && theme !== 'system') await textContrast(page, theme, 'account');
        check(await appearanceRows(page, 'overlay'), `${scenario}: compact panel groups keep whole labels and touch targets`);
        accountLayoutGeometry(await page.capture(`account-overlay-${scenario}`), scenario);
        if (width === 834 && height === 360 && scale === 200) {
          check(await page.eval(`(() => {
            const panel = document.querySelector('#profile-menu');
            const targets = [...panel.querySelectorAll('.vk-ui-choice__target')];
            const reachable = targets.every(target => {
              target.scrollIntoView({ block: 'nearest', behavior: 'instant' });
              const r = target.getBoundingClientRect();
              return target.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2));
            });
            panel.scrollTop = 0;
            return reachable;
          })()`), `${scenario}: every radio hit target can be revealed in the scrollport`);
        }
        await page.eval(`document.querySelector('#profile-close').focus()`);
        await page.key('Tab', SHIFT);
        check(await page.eval(`(() => {
          const el = document.activeElement, r = el.getBoundingClientRect(), d = document.querySelector('#profile-menu').getBoundingClientRect();
          return el.id === 'open-me' && r.top >= d.top && r.bottom <= d.bottom && r.height >= 44 &&
            el.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2));
        })()`), `${scenario}: Shift+Tab reaches the final action and its hit target is exposed`);
        if (await page.eval(`document.querySelector('#profile-menu').scrollHeight > document.querySelector('#profile-menu').clientHeight`)) await page.capture(`account-overlay-${scenario}-bottom`);
        await page.key('Tab');
        check(await page.eval(`(() => {
          const el = document.activeElement, r = el.getBoundingClientRect(), d = document.querySelector('#profile-menu').getBoundingClientRect();
          return el.id === 'profile-close' && r.top >= d.top && r.bottom <= d.bottom &&
            el.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2));
        })()`), `${scenario}: Tab reveals the close action and its hit target`);
        if (width === 834 && height === 360 && scale === 200) {
          await page.capture(`account-overlay-${scenario}-close`);
        }
        await page.key('Escape');
        check(await page.eval(`!${PROFILE_OPEN} && document.activeElement?.id === 'profile-trigger'`), `${scenario}: Escape restores the account utility focus`);
      }
    }
  }
  // The matrix ends on the compact rail, dark theme and 200% text.
  await shortAppearanceStart(page);
  await openProfile(page);
  await chooseTextSize(page, 100, 'overlay-text');
  await page.key('Escape');
  await page.eval(`localStorage.removeItem('vink.ui.rail')`);
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
    const railNav = await page.eval(buildNavInspectionScript('#navigation'));
    check(railNav.present && railNav.items.length === 3, `${name}: #navigation renders all 3 .vk-ui-nav-item links`);
    const baseLabel = railNav.items[0];
    check(railNav.items.every(item => item.wordsIntact && item.lineCount <= 2 && item.labelScrollWidth <= item.labelClientWidth + 1 &&
      item.labelFont === baseLabel?.labelFont && item.labelLineHeight === baseLabel?.labelLineHeight),
      `${name}: rail nav labels keep Vietnamese words intact, consistent font, and <= 2 lines`);
    const axProfile = await collectElementAccessibility(page, '#profile-trigger');
    const axProfileEval = evaluateAccessibleElement(axProfile, { role: 'button', name: 'Tôi', excludeSubstrings: ['TT', '•'] });
    check(axProfileEval.ok, `${name}: #profile-trigger AX exposes 'Tôi' without decorative badges (${axProfileEval.reasons.join('; ')})`);
    const railOverflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({ containers: ['#navigation'] })));
    check(railOverflow.ok, `${name}: rail and page have no unintended/hidden horizontal overflow (${railOverflow.reasons.join('; ')})`);
    check(await page.eval(`(() => {
      const rows = [...document.querySelectorAll('#explore .vk-public-row')];
      return rows.length > 0 && rows.every(row => row.scrollWidth <= row.clientWidth + 1);
    })()`), `${name}: each work link stays within its discovery row`);
    await page.capture(name, {
      scenario: 'rail-r1',
      requestedScene: { width, height, scale, theme },
      textScaleLane: 'dom-override-geometry-only',
      requiredTargets: ['#navigation', '#navigation-toggle', '#profile-trigger'],
      railNav: { itemCount: railNav.items.length, fonts: railNav.items.map(i => i.labelFont), lineCounts: railNav.items.map(i => i.lineCount) },
      profileAx: axProfile.ax,
    });
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
      await page.capture(`${name}-tooltip`, {
        scenario: 'rail-r1-tooltip',
        requestedScene: { width, height, scale, theme },
        textScaleLane: 'dom-override-geometry-only',
        requiredTargets: ['#navigation', '#navigation-tooltip'],
      });
      await page.key('Escape');
      for (let i = 0; i < 3; i++) await page.key('Tab');
      check(await page.eval(`(() => {
        const r = document.querySelector('#profile-trigger').getBoundingClientRect();
        return document.activeElement?.id === 'profile-trigger' && r.top >= 0 && r.bottom <= innerHeight;
      })()`), `${name}: Tab reaches and reveals Tôi at the foot`);
      await page.capture(`${name}-account`, {
        scenario: 'rail-r1-account',
        requestedScene: { width, height, scale, theme },
        textScaleLane: 'dom-override-geometry-only',
        requiredTargets: ['#navigation', '#profile-trigger'],
      });
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
    brand: document.querySelector('.vk-brand'), input: document.querySelector('#public-query'),
    top: document.querySelector('.vk-brand').getBoundingClientRect().top
  };
  window.__exploreRailProbe.input.focus()`);
  await page.send('Input.insertText', { text: 'Mưa trên phố' });
  await page.eval(`document.querySelector('#navigation-toggle').focus()`);
  await page.key('Enter');
  check((await page.eval(RAIL_WIDTH)) === 96 && await page.eval(`document.activeElement === window.__railToggle`), 'Enter collapses once and retains the toggle');
  check(await page.eval(`(() => {
    const { brand, input, top } = window.__exploreRailProbe;
    const toolbar = document.querySelector('.vk-discovery-toolbar').getBoundingClientRect();
    const row = document.querySelector('.vk-public-row').getBoundingClientRect();
    const b = brand.getBoundingClientRect();
    return brand === document.querySelector('.vk-brand') && Math.abs(b.top - top) <= 1 &&
      Math.abs(b.right - toolbar.right) <= 1 && Math.abs(b.right - row.right) <= 1 &&
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
  await chooseTextSize(page, 200, 'overlay-text');
  await page.click('#open-me');
  await page.waitFor(`location.pathname === '/toi' && document.querySelector('#page-theme input:checked')?.value === 'system' &&
    document.querySelector('#page-text input:checked')?.value === '200'`, 'storage-denied System and text size survive client navigation');
  await page.capture('appearance-storage-denied-page-system');
  await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier });
  await page.reload();
  await page.eval(`localStorage.removeItem('vink.ui.rail'); localStorage.removeItem('vink.ui.theme')`);
  await page.reload();

  await accountAppearance(page);
  await preferenceSurfaces(page);

  for (const width of [1536, 1440, 1280, 1024, 834, 390, 360, 320]) {
    const height = width <= 360 ? 800 : width === 390 ? 844 : 900;
    await page.viewport(width, height);
    for (const theme of ['system', 'light', 'dark']) {
      await chooseTheme(page, theme, 'page-theme');
      if (width === 1440 && theme !== 'system') await textContrast(page, theme, 'me');
      for (const scale of [100, 200]) {
        await chooseTextSize(page, scale, 'page-text');
        await page.eval(`scrollTo(0, 0)`);
        await bottomNavigationAlignment(page, `shell ${width}px/${scale}%/${theme}`);
        // Layout can update before glyphs repaint after a root-text change. The
        // generic capture helper also serves actual no-JS pages, so wait only here.
        await page.eval(`new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))`);
        const meMeta = {
          scenario: 'shell-me-matrix',
          state: `${theme}-text${scale}`,
          requestedScene: { width, height, scale, theme },
          textScaleLane: 'app-appearance-control',
          requiredTargets: ['#me-title', '#page-account', '.vk-appearance-card'],
          boundary: 'real-local-postgresql-with-fixture-identity',
        };
        accountLayoutGeometry(await page.capture(`${width}-me-${theme}-text${scale}`, meMeta), `${width}px/${scale}% ${theme}`);
        const meOverflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({
          containers: ['.vk-me', '.vk-account', '.vk-account-status', '.vk-appearance-card'],
        })));
        check(meOverflow.ok, `${width}px/${scale}% ${theme}: /toi has no unintended or hidden overflow (${meOverflow.reasons.join('; ')})`);
        check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), `${width}px/${scale}% ${theme}: no horizontal overflow`);
        check(await appearanceRows(page, 'page'), `${width}px/${scale}% ${theme}: appearance choices keep whole labels and touch targets`);
        if (width < 760 && scale === 200) {
          await page.eval(`document.querySelector('#page-theme-title').scrollIntoView({ block: 'start' })`);
          await page.capture(`${width}-me-${theme}-text${scale}-appearance`, {
            ...meMeta,
            state: `${theme}-text${scale}-appearance`,
            requiredTargets: ['#page-theme-title', '#page-theme'],
          });
          await page.eval(`scrollTo(0, document.documentElement.scrollHeight)`);
          check(await page.eval(`(() => {
            const content = document.querySelector('.vk-me').getBoundingClientRect();
            const bar = document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect();
            return content.bottom <= bar.top + 1;
          })()`), `${width}px/${scale}% ${theme}: end of page content remains reachable above navigation`);
          await page.capture(`${width}-me-${theme}-text${scale}-end`, {
            ...meMeta,
            state: `${theme}-text${scale}-end`,
            requiredTargets: ['#page-text'],
          });
          await page.eval(`scrollTo(0, 0)`);
        }
      }
      await chooseTextSize(page, 100, 'page-text');
    }
  }
  await page.viewport(1024);
  await chooseTextSize(page, 200, 'page-text');
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
  await page.capture('320-me-dark-text200-after-rail-reflow');
  check(await page.eval(`document.documentElement.scrollWidth <= innerWidth`), '320px / 200% text reflows without horizontal overflow');
  await page.eval(`scrollTo(0, document.documentElement.scrollHeight)`);
  await page.waitFor(`parseFloat(getComputedStyle(document.querySelector('.vk-ui-scaffold')).getPropertyValue('--vk-ui-bar-height')) === document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().height`, 'measured bottom clearance');
  check(await page.eval(`document.querySelector('.vk-me').getBoundingClientRect().bottom <= document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect().top`), 'bottom bar clears the end of page content at 320px / 200% text');
  await page.capture('320-me-dark-text200-bottom');
  await page.click('.vk-ui-bottom-nav a[href="/ban-viet"]');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('#create-draft')`, 'mobile desk destination');
  await page.capture('320-desk-dark-text200');
  await page.click('.vk-ui-bottom-nav a[href="/"]');
  await page.waitFor(`location.pathname === '/' && document.querySelector('#discovery-tools .vk-brand') && document.querySelector('#open-tag-filters')`, 'mobile discovery destination');
  await assertBrandPlacement(page);
  await page.eval(`document.querySelector('.vk-ui-skip-link').focus()`);
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('#open-tag-filters:focus-visible') && getComputedStyle(document.activeElement).outlineStyle === 'solid'`), 'at 320px / 200% text, Tab reaches the filter launcher with a visible focus ring');
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('#open-public-search:focus-visible') && getComputedStyle(document.activeElement).outlineStyle === 'solid'`), 'the next Tab reaches the mobile search launcher with a visible focus ring');
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('.vk-brand:focus-visible') && getComputedStyle(document.activeElement).outlineStyle === 'solid'`), 'the next Tab reaches the Explore brand with a visible focus ring');
  await page.click('.vk-ui-bottom-nav a[href="/toi"]');
  await page.waitFor(`location.pathname === '/toi'`, 'mobile Tôi screen');
  await chooseTheme(page, 'system', 'page-theme');
  await chooseTextSize(page, 100, 'page-text');
  await page.eval(`scrollTo(0, 0)`);
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
      await page.viewport(width, 900);
      await page.send('Network.setBlockedURLs', { urls: [`${args.base}/pkg/vink.js*`, `${args.base}/pkg/vink_bg.wasm*`] });
      await page.goto(`${args.base}/toi`);
      navigations++;
      const toiNavSelector = width >= 760 ? '#profile-trigger' : '.vk-ui-bottom-nav a[href="/toi"]';
      const toiAxRole = width >= 760 ? 'button' : 'link';
      for (const scale of [100, 200]) {
        await page.eval(`document.documentElement.style.fontSize = '${scale}%'; scrollTo(0, 0)`);
        const name = `appearance-first-paint-${theme}-os-${os}-${width}-text${scale}`;
        const firstPaintMeta = {
          scenario: 'UI-REVIEW-21-first-paint',
          fixture: 'local-fixture-unknown-account',
          boundary: 'no-js-bootstrap-pre-hydration',
          hydrationPhase: 'before-hydration',
          requestedScene: { width, height: 900, scale, theme, hydrationPhase: 'before-hydration' },
          requiredTargets: ['#me-title', '#page-theme'],
          textScaleLane: 'dom-override-geometry-only',
        };
        await page.capture(name, firstPaintMeta);
        await page.eval(`document.querySelector('#page-theme').scrollIntoView({ block: 'end' })`);
        await page.capture(`${name}-appearance`, { ...firstPaintMeta, state: 'before-hydration-scrolled' });
        if (scale === 100) {
          const axBefore = await collectElementAccessibility(page, toiNavSelector);
          const axBeforeEval = evaluateAccessibleElement(axBefore, { role: toiAxRole, name: 'Tôi', excludeSubstrings: ['TT', '•'] });
          check(axBeforeEval.ok, `${name}: Tôi AX exposes 'Tôi' before hydration (${axBeforeEval.reasons.join('; ')})`);
        }
        const state = await page.eval(`({ hydrated: document.documentElement.dataset.hydrated === 'true', theme: document.documentElement.dataset.theme || 'system', choices: ['page-theme', 'overlay-theme'].map(id => document.querySelector('#' + id + ' input:checked')?.value) })`);
        if (state.hydrated || state.theme !== theme || state.choices.some(value => value !== theme)) mismatches.push({ name, expected: theme, ...state });
      }
      await page.send('Network.setBlockedURLs', { urls: [] });
      await page.reload();
      await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'appearance hydration after reload');
      const axAfter = await collectElementAccessibility(page, toiNavSelector);
      const axAfterEval = evaluateAccessibleElement(axAfter, { role: toiAxRole, name: 'Tôi', excludeSubstrings: ['TT', '•'] });
      check(axAfterEval.ok, `first-paint reload ${theme}/${os}/${width}: Tôi AX exposes 'Tôi' after hydration (${axAfterEval.reasons.join('; ')})`);
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
  await chooseTextSize(page, 200, 'page-text');
  await page.eval(`localStorage.removeItem('vink.ui.theme');
    localStorage.setItem('vink.ui.rail', 'compact');`);
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
  for (const destination of ['/', '/ban-viet', '/toi', `${editorPath}/kiem-duyet`]) {
    if (destination.endsWith('/kiem-duyet')) {
      await page.click('#editor-review-status');
    } else if (destination === '/toi') {
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
      for (const width of [1536, 1440, 1280, 1024, 834, 390, 360, 320]) {
        const height = 900;
        await page.viewport(width, height);
        for (const scale of [100, 200]) {
          await page.eval(`document.documentElement.style.fontSize='${scale}%'; scrollTo(0, 0)`);
          await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
          const containerBefore = await page.eval(buildTargetGeometryScript(selector));
          check(containerBefore.present, `${state} ${width}/${scale}/${theme}: recovery container ${selector} exists before scrolling`);
          const count = await page.eval(`document.querySelector(${JSON.stringify(selector)}).querySelectorAll('button,a[href]').length`);
          check(count > 0, `${state} ${width}/${scale}/${theme}: recovery container exposes at least one action`);
          const actionSummaries = [];
          for (let index = 0; index < count; index++) {
            await page.eval(`(() => { const el=document.querySelector(${JSON.stringify(selector)}).querySelectorAll('button,a[href]')[${index}];
              if(document.activeElement===el) el.blur(); el?.focus(); el?.setAttribute('data-vink-probe-action', '1'); })()`);
            await page.eval(`new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))`);
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
            const actionTarget = await page.eval(buildTargetGeometryScript('[data-vink-probe-action="1"]'));
            await page.eval(`document.querySelector('[data-vink-probe-action]')?.removeAttribute('data-vink-probe-action')`);
            const targetEval = evaluateTargetGeometry(actionTarget, { requireFull: true, requireHit: true, minWidth: 44, minHeight: 44 });
            if (!geometry.ok || !targetEval.ok) {
              await page.capture(`recovery-failure-${state}-${width}-${scale}-${theme}`, {
                allowIncomplete: true,
                state: 'failure-capture',
                target: selector,
                requestedScene: { width, height, scale, theme },
              });
            }
            check(geometry.ok && targetEval.ok, `${state} ${width}/${scale}/${theme}: recovery ${index + 1}/${count} is reachable above navigation without horizontal overflow${index === 0 ? ' and carries primary emphasis' : ''} ${JSON.stringify({ geometry, targetReasons: targetEval.reasons })}`);
            actionSummaries.push({
              index,
              id: actionTarget.id,
              visible: actionTarget.visible,
              full: actionTarget.full,
              hit: actionTarget.hit,
              focused: actionTarget.focused,
              disabled: actionTarget.disabled,
              rect: actionTarget.rect,
              owner: actionTarget.owner,
            });
          }
          const containerAfter = await page.eval(buildTargetGeometryScript(selector));
          const containerCheck = evaluateTargetGeometry(containerAfter, { requireFull: false, requireHit: false });
          check(containerCheck.ok, `${state} ${width}/${scale}/${theme}: recovery container fits without horizontal overflow (${containerCheck.reasons.join('; ')})`);
          const overflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({ containers: [selector] })));
          check(overflow.ok, `${state} ${width}/${scale}/${theme}: recovery view has no hidden/unintended overflow (${overflow.reasons.join('; ')})`);
          await page.capture(`recovery-${state}-${width}-${scale}-${theme}`, {
            scenario: `recovery-${state}`,
            state,
            target: selector,
            requiredTargets: [selector],
            requestedScene: { width, height, scale, theme },
            textScaleLane: 'dom-override-geometry-only',
            recovery: { before: containerBefore, after: containerAfter },
            actions: actionSummaries,
            overflow,
          });
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
  for (const theme of ['light','dark']) for (const width of [320,390,834,1440,1536]) for (const scale of [100,200]) {
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

async function timestampPresentation(page) {
  console.log('#213: real timezone conversion, no-JS fallback and hydration:');
  const path=`${args.base}/tac-pham/${SAMPLE_WORK}`;
  const html=await (await fetch(path)).text();
  check(/<time[^>]*datetime="[^"]+"[^>]*>[^<]*UTC/.test(html), 'public SSR has a deterministic absolute instant with explicit UTC');
  await page.send('Emulation.setScriptExecutionDisabled',{value:true});
  try {
    await page.goto(path);
    check(await page.eval(`document.querySelector('.vk-work-byline__date time').textContent.includes('UTC')`), 'no-JS public byline retains the labelled UTC fallback');
  } finally { await page.send('Emulation.setScriptExecutionDisabled',{value:false}); }
  for (const zone of ['Pacific/Kiritimati','America/Los_Angeles']) {
    await page.send('Emulation.setTimezoneOverride',{timezoneId:zone});
    await page.goto(path);
    await page.waitFor(`document.documentElement.dataset.hydrated==='true' && document.querySelector('.vk-work-byline__date time')?.textContent===new Intl.DateTimeFormat('vi-VN',{day:'2-digit',month:'2-digit',year:'numeric',hour:'2-digit',minute:'2-digit',hourCycle:'h23',timeZoneName:'short'}).format(new Date(document.querySelector('.vk-work-byline__date time').dateTime))`, 'public local time after hydration');
    const gate=new DraftGate(page);
    const instants=['2026-11-01T23:30:00Z','2099-01-01T00:00:00Z','2026-02-30T08:30:00Z'];
    gate.list={body:{drafts:instants.map((updated_at,i)=>({work_id:`88888888-8888-4888-8888-${String(i).padStart(12,'0')}`,title:'[Time] '+i,version:i+1,updated_at}))}};
    await gate.enable();
    try {
      await page.goto(`${args.base}/ban-viet`);
      await page.waitFor(`document.querySelectorAll('.vk-draft-row time').length===3`, 'timezone fixture');
      check(await page.eval(`(() => {
        const times=[...document.querySelectorAll('.vk-draft-row time')];
        return times.every((t,i)=>t.dateTime===${JSON.stringify(instants)}[i] && t.title===t.dateTime) &&
          times.slice(0,2).every(t=>t.textContent===new Intl.DateTimeFormat('vi-VN',{day:'2-digit',month:'2-digit',year:'numeric',hour:'2-digit',minute:'2-digit',hourCycle:'h23',timeZoneName:'short'}).format(new Date(t.dateTime))) &&
          times[2].textContent==='Thời gian không hợp lệ';
      })()`), `${zone}: true conversion, future absolute time, invalid fallback and intact raw instant`);
      check(await page.eval(`document.querySelector('.vk-draft-row time').textContent.includes('${zone==='Pacific/Kiritimati'?'02/11/2026':'01/11/2026'}')`), `${zone}: UTC-to-local day boundary`);
      check(await page.eval(`(() => {const t=document.querySelector('.vk-draft-row time'), before=t.textContent, clock=Date.now; try {Date.now=()=>0; return t.textContent===before;} finally {Date.now=clock;}})()`), 'absolute display is independent of a relative/fake clock');
      await page.capture(`time-${zone.replaceAll('/','-')}`,{scenario:'#213-absolute-device-zone',boundary:'Chrome Intl/device zone + synthetic draft instants'});
    } finally { await gate.disable(); }
  }
  await page.send('Emulation.setTimezoneOverride',{timezoneId:''});
}

async function draftR6Hierarchy(page) {
  console.log('Desk #213: same R6 fixture, compact rows, full titles on focus and no resume read:');
  const title = 'Những ngày mưa ở khuỷu đường cũ, nơi người trở về vẫn nhớ tiếng Việt và những khổ thơ còn đang viết dở'.normalize('NFD');
  const cards = [title, 'Hiên nhà còn nắng', 'Bên kia ô cửa, nơi tiếng gió giữ lại những khổ thơ chưa kịp đặt tên'].map((title, i) => ({
    work_id: `88888888-8888-4888-8888-${String(i).padStart(12, '0')}`, title, version: 3-i, updated_at: '2026-11-09T08:42:00Z',
  }));
  const gate = new DraftGate(page); gate.list = {body:{drafts:cards}}; await gate.enable();
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length===3 && document.querySelector('#draft-scope').textContent==='Đã tải 3 tác phẩm.'`, 'compact R6 fixture');
  for (const [width, scale, theme] of [[390,100,'light'], [320,200,'dark'], [1440,100,'light'], [1280,100,'light'], [1440,200,'dark'], [1280,200,'dark']]) {
    await page.viewport(width);
    await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:0,y:0});
    await page.eval(`document.documentElement.dataset.theme='${theme}';document.documentElement.style.fontSize='${scale}%';scrollTo(0,0)`);
    await page.eval(`Promise.all([...document.fonts].map(f=>f.load().catch(()=>{})))`);
    await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
    await page.capture(`r6-${width}-${scale}-${theme}`);
    check(await page.eval(`(() => {
      const rows=[...document.querySelectorAll('.vk-draft-row__link')], list=document.querySelector('#draft-results');
      return !document.querySelector('#resume, #resume-title-toggle') && document.documentElement.scrollWidth<=innerWidth &&
        getComputedStyle(list).backgroundColor!=='rgba(0, 0, 0, 0)' && rows.every((row,i)=>
          row.getAttribute('href')==='/ban-viet/'+${JSON.stringify(cards)}[i].work_id &&
          row.querySelector('.vk-draft-row__title').textContent===${JSON.stringify(cards)}[i].title &&
          !row.textContent.includes('Phiên bản') && getComputedStyle(row).paddingLeft===getComputedStyle(row).paddingRight &&
          row.querySelectorAll('a,button,input').length===0);
    })()`), `#213 ${width}/${scale}/${theme}: no resume, one list surface, full Unicode titles and exact IDs`);
    await page.eval(`document.querySelector('#drafts').scrollIntoView({block:'start'})`);
    await page.capture(`r6-list-${width}-${scale}-${theme}`);
    await textContrast(page, theme, 'desk');
  }
  for (const forced of [false,true]) {
    await page.send('Emulation.setEmulatedMedia',{features:[{name:'forced-colors',value:forced?'active':'none'}]});
    await page.eval(`document.querySelector('.vk-draft-row__link').focus()`);
    await page.key('Tab'); await page.key('Tab', SHIFT);
    check(await page.eval(`(() => {
      const row=document.querySelector('.vk-draft-row__link'), t=row.querySelector('.vk-draft-row__title');
      return document.activeElement===row && row.matches(':focus-visible') && getComputedStyle(t).overflow==='visible' &&
        t.textContent===${JSON.stringify(title)} && parseFloat(getComputedStyle(row).outlineWidth)>=2;
    })()`), '#213 full NFD title expands on keyboard focus, including forced colors');
    await page.capture(`r6-focus-${forced?'forced-colors':'normal'}`);
  }
  check(gate.requests.length===1 && gate.requests.every(u=>new URL(u).searchParams.get('limit')==='24'), '#213 one bounded head GET, zero Recent/1 resume reads');
  await page.send('Emulation.setEmulatedMedia',{features:[LIGHT_SYSTEM]});
  await page.eval(`document.documentElement.style.fontSize=''`); await gate.disable();
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
  const cards = Array.from({ length: 24 }, (_, i) => ({
    work_id: `88888888-8888-4888-8888-${String(i).padStart(12, '0')}`,
    title: i === 0 ? longTitle.normalize('NFD') : i === 1 ? '[Fixture] ĐƯỜNG VỀ CÓ NẮNG' : i === 2 ? '' : `[Fixture] Bản thảo ${i}`,
    version: i + 1, updated_at: '2026-11-01T08:30:00Z',
  }));
  cards[19].title = cards[18].title; // Repeated titles still open their own work IDs.
  gate.list = { body: { drafts: cards } };
  gate.holdGets = true;
  await gate.enable();
  await page.viewport(1440);
  if (await page.eval(`location.href === ${JSON.stringify(`${args.base}/ban-viet`)}`)) {
    await page.reload();
  } else {
    await page.goto(`${args.base}/ban-viet`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'hydrated dashboard');
  }
  const initial = await gate.next('gets');
  await page.waitFor(`document.querySelector('[data-testid=draft-loading]')`, 'initial dashboard loading');
  check(await page.eval(`document.querySelector('[data-testid=draft-content]').getAttribute('aria-busy') === 'true' && !document.querySelector('[data-testid=draft-empty]') && !document.querySelector('#create-draft')`), 'loading is distinct from empty and cannot create');
  await gate.fulfill(initial, 503, {error:{code:'dependency_unavailable'}});
  await page.waitFor(`!!document.querySelector('[data-testid=desk-error]')`, 'first-load failure');
  check(await page.eval(`!document.querySelector('[data-testid=draft-empty], [data-testid=draft-no-results], #create-draft') && !!document.querySelector('#draft-search')`), 'first-load failure is not empty and search stays reachable without enabling create');
  gate.holdGets = false;
  await page.click('#retry-drafts');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 24`, '24 loaded rows');
  const firstHref = `/ban-viet/${cards[0].work_id}`;
  check(await page.eval(`!document.querySelector('#resume, #resume-title-toggle, [data-testid=draft-resume]') && document.querySelector('.vk-draft-row__link').getAttribute('href')===${JSON.stringify(firstHref)}`), 'removed resume has no replacement or disclosure; first row opens the authorized work');
  check(gate.requests.length===2 && gate.requests.every(u=>new URL(u).searchParams.get('limit')==='24'), 'failed initial read and retry each use only the page read');
  check(await page.eval(`document.querySelector('#draft-scope').textContent.includes('Đã tải 24 tác phẩm') && document.querySelector('#draft-search-help').textContent === 'Theo tên bản thảo; gồm cả tác phẩm đã đăng'`), '24 loaded rows are not claimed as a global total');
  check(await page.eval(`[...document.querySelectorAll('[data-testid=draft-row]')].every((row, i) => row.querySelector('a').getAttribute('href') === '/ban-viet/' + ${JSON.stringify(cards)}[i].work_id && !row.textContent.includes('Phiên bản') && row.querySelector('time').dateTime === '2026-11-01T08:30:00Z')`), 'every row keeps the provider work_id and raw update instant, with no visible version');
  check(manuscriptRequests === 0, 'list never fetches full manuscripts (no N+1)');
  check(await page.eval(`!document.querySelector('#desk').textContent.match(/Chưa đăng|Đã đăng|góp ý|thể loại|D-05|VINK_DEV_PRINCIPAL/)`), 'no invented publication/profile/feedback state or implementation copy');
  await page.eval(`document.querySelector('#create-draft').focus()`);
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('.vk-draft-search button[type=submit]:focus-visible')`), 'keyboard order moves from create to the leading search submit');
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.id === 'draft-search' && document.activeElement.matches(':focus-visible')`), 'keyboard continues to the stable search input with visible focus');
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.id === 'draft-sort'`), 'empty search has no clear target; sort follows in keyboard order');
  await page.key('Tab');
  await page.waitFor(`!document.querySelector('#refresh-drafts-tooltip').hidden`, 'refresh tooltip on keyboard focus');
  check(await page.eval(`document.activeElement.id === 'refresh-drafts' && document.querySelector('#refresh-drafts').getAttribute('aria-label') === 'Tải lại' && !document.querySelector('#refresh-drafts-tooltip :is(button,a,input,[tabindex])')`), 'refresh has its own name and the shared tooltip never takes focus');
  await page.key('Escape');
  check(await page.eval(`document.querySelector('#refresh-drafts-tooltip').hidden && document.activeElement.id === 'refresh-drafts'`), 'Escape dismisses only the tooltip and keeps trigger focus');
  await page.viewport(390);
  const { nodes: deskAX } = await page.send('Accessibility.getFullAXTree');
  check(deskAX.some(node=>node.role?.value==='searchbox' && node.name?.value==='Tìm tác phẩm'), 'searchbox retains its native label');
  check(deskAX.some(node=>node.role?.value==='link' && node.name?.value.includes(cards[0].title)) && !deskAX.some(node=>node.role?.value==='button' && node.name?.value==='Xem đủ tên bản thảo'), 'accessibility tree retains the full clamped title without resume disclosure');
  for (const theme of ['light','dark']) for (const scale of [100,200]) for (const width of [1536,1440,1280,1024,834,390,320]) {
    await page.viewport(width);
    await page.send('Input.dispatchMouseEvent',{type:'mouseMoved',x:0,y:0});
    await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%'; document.activeElement.blur(); scrollTo(0,0)`);
    await page.eval(`Promise.all(document.getAnimations().filter(a=>Number.isFinite(a.effect.getComputedTiming().endTime)).map(a=>a.finished.catch(()=>{})))`);
    await page.capture(`desk-long-${width}-${scale}-${theme}`);
    check(await page.eval(`(() => {
      const row=document.querySelector('.vk-draft-row__link'), work=row.closest('.vk-author-work'), title=row.querySelector('.vk-draft-row__title'), meta=work.querySelector('.vk-draft-meta');
      const t=title.getBoundingClientRect(), m=meta.getBoundingClientRect(), r=work.getBoundingClientRect(), h=document.querySelector('#heading').getBoundingClientRect();
      return !document.querySelector('#resume') && document.documentElement.scrollWidth<=innerWidth &&
        title.textContent===${JSON.stringify(cards[0].title)} && getComputedStyle(title).webkitLineClamp==='2' &&
        t.height<=2*parseFloat(getComputedStyle(title).lineHeight)+1 && m.top>=t.bottom-1 && m.bottom<=r.bottom &&
        !row.textContent.includes('Phiên bản') && row.getAttribute('href')===${JSON.stringify(firstHref)} && row.querySelectorAll('a,button,input').length===0 &&
        ['#drafts','.vk-draft-controls','#draft-results','.vk-draft-search .vk-ui-search__shell'].every(s=>Math.abs(document.querySelector(s).getBoundingClientRect().left-h.left)<1);
    })()`), `#213 ${theme} ${width}/${scale}: two-line default, complete DOM title, metadata and common list/search axis`);
    await page.eval(`document.querySelector('#drafts').scrollIntoView({block:'start'})`);
    await searchBarGeometry(page,'#draft-search',`Desk ${width}/${scale}/${theme}`);
    await quietListSurfaces(page,'.vk-draft-row__link',`Desk ${width}/${scale}/${theme}`);
    await textContrast(page,theme,'desk');
    await page.capture(`desk-list-${width}-${scale}-${theme}`);
    check(await page.eval(`(() => {
      const row=document.querySelector('.vk-draft-row__link'), arrow=row.querySelector('.vk-draft-row__open');
      const targets=['#draft-sort','#refresh-drafts'].map(s=>document.querySelector(s).getBoundingClientRect());
      return arrow.getAttribute('aria-hidden')==='true' && targets.every(r=>r.left>=0 && r.right<=innerWidth && r.width>=48 && r.height>=48);
    })()`), `#213 ${width}/${scale}: sort/refresh targets fit without shrinking`);
    check(await page.eval(`(() => {
      const create=document.querySelector('#create-draft'), refresh=document.querySelector('#refresh-drafts');
      const r=create.getBoundingClientRect(), label=create.querySelector('.vk-ui-button__label');
      const range=document.createRange(); range.selectNodeContents(label);
      return document.querySelectorAll('#create-draft').length===1 && create.type==='button' &&
        create.textContent==='Tạo bản thảo' && create.classList.contains('vk-ui-button--filled') &&
        getComputedStyle(create).backgroundColor!=='rgba(0, 0, 0, 0)' &&
        create.querySelectorAll('svg[data-icon=add][aria-hidden=true]').length===1 &&
        r.width>=48 && r.height>=48 && r.left>=0 && r.right<=innerWidth &&
        [...range.getClientRects()].every(t=>t.left>=r.left-1 && t.right<=r.right+1 && t.top>=r.top-1 && t.bottom<=r.bottom+1) &&
        refresh.type==='button' && refresh.getAttribute('aria-label')==='Tải lại' && refresh.dataset.surface==='tonal' &&
        getComputedStyle(refresh).backgroundColor!=='rgba(0, 0, 0, 0)' &&
        !refresh.hasAttribute('aria-pressed') && !create.querySelector('button,a,input') &&
        document.querySelector('#draft-sort').tagName==='SELECT';
    })()`), `#225 ${width}/${scale}/${theme}: full create label + one Add target, native Select/refresh and no fake pressed`);
    if (width<=390 && scale===200) {
      await page.eval(`document.querySelector('.vk-draft-row__link').focus()`); await page.key('Tab'); await page.key('Tab',SHIFT);
      check(await page.eval(`getComputedStyle(document.querySelector('.vk-draft-row__title')).overflow==='visible' && document.activeElement.matches('.vk-draft-row__link:focus-visible')`), 'keyboard reveals the entire title at 200%');
      await page.capture(`desk-title-expanded-${width}-${scale}-${theme}`);
      await page.eval(`document.querySelector('.vk-draft-row:last-child a').focus()`);
      await page.key('Tab'); await page.key('Tab',SHIFT);
      check(await page.eval(`(() => { const r=document.activeElement.getBoundingClientRect(), nav=document.querySelector('.vk-ui-bottom-nav').getBoundingClientRect(); return r.bottom<=nav.top+1 && r.top>=0; })()`), 'last row keyboard target is visible above bottom navigation');
      await page.capture(`desk-last-${width}-${scale}-${theme}`);
    }
  }
  // Blank and unbroken titles retain their full accessible value and one real destination.
  for (const [variant,title] of [['blank',''],['word','[Fixture] '+'Đườngỹỷữợ'.repeat(20)]]) {
    gate.list={body:{drafts:[{...cards[0],title}]}};
    await page.click('#refresh-drafts');
    await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length===1 && document.querySelector('.vk-draft-row__title').textContent===${JSON.stringify(title||'Chưa đặt tên')}`,`one ${variant} row`);
    await page.viewport(320);
    for (const scale of [100,200]) {
      await page.eval(`document.documentElement.style.fontSize='${scale}%'; document.querySelector('.vk-draft-row__link').focus()`);
      check(await page.eval(`document.documentElement.scrollWidth<=innerWidth && getComputedStyle(document.querySelector('.vk-draft-row__title')).overflow==='visible'`), 'focused unbroken title reflows without loss');
      await page.capture(`desk-one-${variant}-320-${scale}`);
    }
  }
  await page.eval(`document.documentElement.style.fontSize = ''`);
  await page.viewport(1440);
  gate.list = { body: { drafts: cards } };
  await page.click('#refresh-drafts');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 24`, 'restore 24 loaded rows');
  const search = async text => {
    const requestsBefore = gate.requests.length;
    await page.eval(`document.querySelector('#draft-search').focus(); document.querySelector('#draft-search').select()`);
    await page.send('Input.insertText', { text });
    check(gate.requests.length === requestsBefore, 'unsubmitted search text does not issue a read');
    await page.key('Enter');
  };
  await page.waitFor(`document.querySelector('[data-testid=draft-result-count]').textContent.includes('Đã tải 24')`, 'settled inventory before typing');
  await page.eval(`window.__resultAnnouncements=[]; window.__resultObserver = new MutationObserver(() => window.__resultAnnouncements.push(document.querySelector('[data-testid=draft-result-count]').textContent)); window.__resultObserver.observe(document.querySelector('[data-testid=draft-result-count]'), {childList:true,subtree:true,characterData:true})`);
  await search('duong ve');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 2`, 'accentless Vietnamese results');
  await search('ĐƯỜNG VỀ CÓ NẮNG'.normalize('NFD'));
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 1`, 'combining accented results');
  await search('không có tên này');
  await page.waitFor(`document.querySelector('[data-testid=draft-no-results]')`, 'no results');
  check(await page.eval(`document.activeElement.id==='draft-search'`), 'submitting a server query preserves focus');
  await page.waitFor(`document.querySelector('[data-testid=draft-result-count]').textContent==='Đã tải 0 tác phẩm.'`, 'one settled no-result announcement');
  check(await page.eval(`window.__resultAnnouncements.length>=3 && document.querySelectorAll('#drafts [role=status]').length===1`), 'one live result status reports each completed server query');
  await page.eval(`window.__resultObserver.disconnect()`);
  check(await page.eval(`!document.querySelector('[data-testid=draft-empty]') && !!document.querySelector('#create-draft') && !!document.querySelector('#draft-search')`), 'filtered miss is not first-use; create and search remain reachable');
  await page.viewport(390);
  await page.eval(`document.querySelector('#drafts').scrollIntoView({ block: 'start' })`);
  await page.capture('desk-no-results-390');
  check(await page.eval(`!!document.querySelector('[data-testid=draft-no-results] .vk-empty-state__actions #clear-draft-search')`), 'draft clear belongs to the shared no-results composition');
  await captureRecovery(page, 'desk-no-results', '[data-testid=draft-no-results]');
  await page.click('#clear-draft-search');
  await page.waitFor(`document.querySelectorAll('[data-testid=draft-row]').length === 24`, 'clear query');
  await page.waitFor(`document.activeElement?.id === 'draft-search'`, 'clearing draft results restores search focus');
  await page.eval(`const sort = document.querySelector('#draft-sort'); sort.value = 'title'; sort.dispatchEvent(new Event('change', { bubbles: true }))`);
  await page.waitFor(`document.querySelector('[data-testid=draft-row]').textContent.includes('[Fixture] Bản thảo 10')`, 'title sort');
  check(await page.eval(`[...document.querySelectorAll('[data-testid=draft-row]')].every(row=>!row.textContent.includes('Phiên bản') && row.querySelector('time').dateTime==='2026-11-01T08:30:00Z')`), 'sorting preserves raw instants and compact metadata');
  await search('duong ve');
  await page.eval(`window.__draftSearchInput = document.querySelector('#draft-search'); window.__draftSubmits=0; document.querySelector('.vk-draft-search').addEventListener('submit',()=>window.__draftSubmits++)`);
  await page.key('Enter');
  await page.click('.vk-draft-search button[type=submit]');
  check(await page.eval(`!location.search && window.__draftSubmits===2 && document.querySelectorAll('[data-testid=draft-row]').length===2`), 'Desk Enter and click submit query the server without URL persistence and preserve server sort');
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
  await page.waitFor(`document.querySelector('[data-testid=draft-content]').getAttribute('aria-busy') === 'false'`, 'retry settled before lifecycle hints');
  gate.holdGets = true;
  const beforeReconnectPosts = gate.postCount;
  await page.eval(`window.__deskNow = performance.now.bind(performance); performance.now = () => window.__deskNow() + 121000; document.querySelector('#draft-search').focus(); for (const name of ['online','focus','visibilitychange','pageshow']) (name === 'visibilitychange' ? document : window).dispatchEvent(new Event(name))`);
  const backgroundList = await gate.next('gets');
  await sleep(150);
  check(gate.gets.length === 0 && gate.postCount === beforeReconnectPosts, 'Desk reconnect burst coalesces one GET and sends no mutation');
  check(await page.eval(`document.activeElement === window.__draftSearchInput && window.__draftSearchInput === document.querySelector('#draft-search') && window.__draftSearchInput.value === 'duong ve'`), 'background Desk refresh keeps the focused search node and private query');
  gate.holdGets = false;
  await gate.answerGet(backgroundList);
  await page.waitFor(`document.querySelector('[data-testid=draft-content]').getAttribute('aria-busy') === 'false'`, 'background Desk refresh accepted');
  check(await page.eval(`document.activeElement === window.__draftSearchInput`), 'background completion does not move focus');
  await page.eval(`performance.now = window.__deskNow; delete window.__deskNow`);
  gate.list = { status: 403, body: { error: { code: 'forbidden', message: 'test fixture' } } };
  await page.click('#refresh-drafts');
  await page.waitFor(`document.querySelector('[data-testid=desk-access]')?.textContent.includes('không có quyền')`, 'forbidden dashboard');
  check(await page.eval(`!document.querySelector('[data-testid=draft-empty], #draft-search, #retry-drafts, #create-draft')`), 'forbidden clears private controls and keeps a distinct access state');
  await captureRecovery(page, 'desk-forbidden', '[data-testid=desk-access]');
  gate.list = { status: 401, body: { error: { code: 'unauthenticated', message: 'test fixture' } } };
  await page.reload();
  await page.waitFor(`document.querySelector('[data-testid=desk-access]')?.textContent.includes('Phiên đăng nhập cần được kiểm tra lại')`, 'unauthenticated dashboard while fixture standing remains active');
  await captureRecovery(page, 'desk-unauthenticated', '[data-testid=desk-access]');
  check(await page.eval(`!document.querySelector('#create-draft, #retry-drafts') && document.querySelector('[data-testid=desk-access] a').getAttribute('href') === '/toi'`), '401 during an active standing goes to Account for reconciliation');
  gate.list = { body: { drafts: [] } };
  await page.reload();
  await page.waitFor(`document.querySelector('[data-testid=draft-empty]')`, 'authorized unfiltered empty dashboard');
  await page.typeAtEnd('#draft-search', 'duong ve');
  await page.key('Enter');
  await page.waitFor(`document.querySelector('[data-testid=draft-no-results]')`, 'empty filtered result is still a search miss');
  await page.click('#clear-draft-search');
  await page.waitFor(`document.querySelector('[data-testid=draft-empty]')`, 'unfiltered empty dashboard');
  check(await page.eval(`document.querySelectorAll('#create-draft').length === 1 && document.querySelector('#create-draft').textContent === 'Viết bản thảo đầu tiên' && !document.querySelector('[data-testid=draft-resume]')`), 'empty has exactly one primary create action');
  check(await page.eval(`!!document.querySelector('[data-testid=draft-empty] .vk-empty-state__actions #create-draft')`), 'first create belongs to the shared empty composition');
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
  await page.waitFor(`document.querySelector('.vk-draft-row__link')`, 'real authorized list');
  const before = await page.eval(`fetch('/api/v1/drafts').then(r => r.json()).then(list => list.drafts.map(d => d.work_id))`);
  await page.click('#create-draft');
  const lost = await gate.next('posts');
  const postsBefore = gate.postCount;
  await page.eval(`for (let i = 0; i < 5; i++) document.querySelector('#create-draft').click()`);
  await page.eval(`document.querySelector('#create-draft').focus()`);
  await page.key('Enter'); await page.key('Space');
  check(gate.postCount===postsBefore, '#225 pending native Enter/Space cannot dispatch another create POST');
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
  check(await page.eval(`!!document.querySelector('.vk-draft-row__link[href="/ban-viet/${created[0]}"]')`), 'reconciled draft is an ordinary row');
  await page.click('.vk-draft-row__link[href="/ban-viet/' + created[0] + '"]');
  await page.waitFor(`${EDITOR_READY} && ${TAGS_READY} && location.pathname === '/ban-viet/${created[0]}'`, 'row opens the real draft and its tags');
  check((await page.status()) === 'saved', 'reconciled draft opens at its real saved version');
  const renamed = '[Fixture] Tiêu đề vừa sửa — tiếng Việt ỹ ỷ ữ ợ';
  await page.typeAtEnd('input[name=title]', renamed);
  await page.key('Enter');
  await page.waitStatus('saved', 'title Enter submits the dirty editor and the server confirms it');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`location.pathname === '/ban-viet' && document.querySelector('.vk-draft-row__link')`, 'return to dashboard');
  check(await page.eval(`document.querySelector('.vk-draft-row__link[href="/ban-viet/${created[0]}"] .vk-draft-row__title').textContent===${JSON.stringify(renamed)}`), 'return after real save shows the acknowledged title on its work row');
  check((await (await fetch(args.base+'/api/v1/drafts/'+created[0])).json()).version===2, 'the real saved DTO version/CAS still advances without a visible version');
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
  const rowUrl = `${args.base}/ban-viet/${created[0]}`;
  const priorTargets = (await page.cdp.send('Target.getTargets')).targetInfos.map(target => target.targetId);
  await page.click(realRow, page.mod);
  let opened;
  for (let attempt = 0; attempt < 50 && !opened; attempt++) {
    const { targetInfos } = await page.cdp.send('Target.getTargets');
    opened = targetInfos.find(target => target.type === 'page' && target.url === rowUrl && !priorTargets.includes(target.targetId));
    if (!opened) await sleep(100);
  }
  check(opened && await page.eval(`location.pathname === '/ban-viet'`), 'modifier-click opens the same draft in a new tab, leaving the Desk in place');
  await page.cdp.send('Target.closeTarget', { targetId: opened.targetId });
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

// The border box must fit the viewport AND every clipping ancestor. A successful .focus()
// or unchanged window.scrollX is compatible with a clipped or covered action.
function editorTargetGeometry(selector) {
  const el = document.querySelector(selector);
  const root = document.documentElement;
  const page = { scrollX, scrollY, scrollWidth: root.scrollWidth, clientWidth: root.clientWidth };
  if (!el?.checkVisibility({ visibilityProperty: true, opacityProperty: true })) {
    return { selector, present: Boolean(el), full: false, hit: false, page };
  }
  const rect = el.getBoundingClientRect();
  const clip = { left: 0, right: root.clientWidth, top: 0, bottom: innerHeight };
  let owner = document.scrollingElement;
  for (let node = el.parentElement; node && node !== root; node = node.parentElement) {
    const css = getComputedStyle(node);
    const bounds = node.getBoundingClientRect();
    const left = bounds.left + node.clientLeft;
    const top = bounds.top + node.clientTop;
    if (css.overflowX !== 'visible') {
      clip.left = Math.max(clip.left, left);
      clip.right = Math.min(clip.right, left + node.clientWidth);
    }
    if (css.overflowY !== 'visible') {
      clip.top = Math.max(clip.top, top);
      clip.bottom = Math.min(clip.bottom, top + node.clientHeight);
    }
    if (owner === document.scrollingElement && /auto|scroll/.test(css.overflowY)) owner = node;
  }
  const full = rect.width > 0 && rect.height > 0 &&
    rect.left >= clip.left - 1 && rect.right <= clip.right + 1 &&
    rect.top >= clip.top - 1 && rect.bottom <= clip.bottom + 1;
  const x = (rect.left + rect.right) / 2;
  const inset = Math.min(6, rect.height / 4);
  const hit = full && [rect.top + inset, (rect.top + rect.bottom) / 2, rect.bottom - inset]
    .every(y => el.contains(document.elementFromPoint(x, y)));
  const style = getComputedStyle(el);
  const label = el.querySelector('.vk-ui-button__label') ?? el;
  return {
    selector, present: true, full, hit, rect: rect.toJSON(), clip, page,
    focused: el === document.activeElement,
    focusRing: el.matches(':focus-visible') && parseFloat(style.outlineWidth) > 0,
    labelFits: label.scrollWidth <= label.clientWidth + 1,
    disabled: el.matches(':disabled,[aria-disabled=true]'),
    description: el.getAttribute('aria-describedby')?.split(/\s+/)
      .map(id => document.getElementById(id)?.textContent.trim()).join(' ') ?? null,
    owner: { id: owner.id || owner.tagName, scrollTop: owner.scrollTop,
      scrollHeight: owner.scrollHeight, clientHeight: owner.clientHeight,
      scrollWidth: owner.scrollWidth, clientWidth: owner.clientWidth },
    scrollWidth: el.scrollWidth, clientWidth: el.clientWidth,
  };
}

const editorTarget = (page, selector) => page.eval(`(${editorTargetGeometry})(${JSON.stringify(selector)})`);
const fitsEditorAction = m => m.full && m.hit && m.labelFits && m.rect.width >= 44 && m.rect.height >= 44 &&
  m.page.scrollWidth <= m.page.clientWidth + 1 && m.owner.scrollWidth <= m.owner.clientWidth + 1;
const editorLayoutReady = page => page.eval(`document.fonts.ready.then(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))))`);
const editorSceneName = (stem, { width, height, scale, theme }) => `${stem}-${width}x${height}-${scale}-${theme}`;
const captureEditorScene = (page, stem, editorScene, evidence) =>
  page.capture(editorSceneName(stem, editorScene), { editorScene, boundary: 'CDP root font override; synthetic LocalFixture', ...evidence });

async function editorRecoveryScene(page, selector, scene, stem) {
  const actions = selector.includes('save-failed')
    ? ['#retry-save', '#save-failure-details > summary']
    : ['#keep-mine', '#save-conflict-details > summary'];
  const detailSelector = selector.includes('save-failed') ? '#save-failure-details' : '#save-conflict-details';
  await page.eval(`scrollTo(0, 0)`);
  await editorLayoutReady(page);
  const before = await page.eval(buildTargetGeometryScript(selector));
  check(before.present, `${stem}: recovery target exists before scrolling`);
  await page.eval(`document.querySelector('${selector}').scrollIntoView({block:'start',behavior:'instant'})`);
  await editorLayoutReady(page);
  const recovery = await page.eval(buildTargetGeometryScript(selector));
  const recoveryCheck = evaluateTargetGeometry(recovery, { requireFull: false, requireHit: false });
  const warning = await page.eval(buildTargetGeometryScript(`${selector} .vk-ui-notice__message`));
  const warningCheck = evaluateTargetGeometry(warning, { requireFull: true, requireHit: false });
  check(recoveryCheck.ok && warningCheck.ok && recovery.scrollWidth <= recovery.clientWidth + 1,
    `${stem}: warning is visible and recovery fits horizontally ${JSON.stringify({ recovery, warning, reasons: [...recoveryCheck.reasons, ...warningCheck.reasons] })}`);
  if ([320, 390].includes(scene.width)) {
    await captureEditorScene(page, `${stem}-warning`, scene, {
      state: 'recovery-warning',
      target: selector,
      requiredTargets: [selector, `${selector} .vk-ui-notice__message`],
      scrollOwner: recovery.owner,
      before,
      recovery,
      warning,
    });
  }
  const actionSummaries = [];
  for (const action of actions) {
    await page.eval(`document.querySelector(${JSON.stringify(action)}).scrollIntoView({block:'center',behavior:'instant'})`);
    await editorLayoutReady(page);
    const revealed = await page.eval(buildTargetGeometryScript(action));
    const actionCheck = evaluateTargetGeometry(revealed, { requireFull: true, requireHit: true, minWidth: 44, minHeight: 44 });
    check(fitsEditorAction(revealed) && actionCheck.ok, `${stem}: ${action} is visible and hit-testable ${JSON.stringify({ revealed, reasons: actionCheck.reasons })}`);
    actionSummaries.push({
      selector: action,
      id: revealed.id,
      visible: revealed.visible,
      full: revealed.full,
      hit: revealed.hit,
      disabled: revealed.disabled,
      disabledReason: revealed.description,
      rect: revealed.rect,
      scrollOwner: revealed.owner,
    });
    if ([320, 390].includes(scene.width)) {
      await captureEditorScene(page, `${stem}-${action.split(' ')[0].slice(1)}`, scene, {
        state: `recovery-action-${action.split(' ')[0].slice(1)}`,
        target: action,
        requiredTargets: [selector, action],
        scrollOwner: revealed.owner,
        before,
        recovery: await page.eval(buildTargetGeometryScript(selector)),
        action: revealed,
        actions: actionSummaries,
      });
    }
  }
  await page.eval(`document.querySelector(${JSON.stringify(actions[0])}).focus()`);
  for (const action of actions.slice(1)) {
    await page.key('Tab');
    const focused = await page.eval(buildTargetGeometryScript(action));
    check(fitsEditorAction(focused) && focused.focused && focused.focusRing,
      `${stem}: Tab reveals ${action} ${JSON.stringify(focused)}`);
  }
  for (const action of actions.slice(0, -1).reverse()) {
    await page.key('Tab', SHIFT);
    const focused = await page.eval(buildTargetGeometryScript(action));
    check(fitsEditorAction(focused) && focused.focused && focused.focusRing,
      `${stem}: Shift+Tab reveals ${action} ${JSON.stringify(focused)}`);
  }
  const detailBefore = await page.eval(`Boolean(document.querySelector('${detailSelector}')?.open)`);
  await page.click(`${detailSelector} > summary`);
  await editorLayoutReady(page);
  const detailAfterOpen = await page.eval(`Boolean(document.querySelector('${detailSelector}')?.open)`);
  const detailOverflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({ containers: [selector, detailSelector] })));
  check(detailAfterOpen === !detailBefore && detailOverflow.ok,
    `${stem}: toggling recovery details updates state cleanly without overflow (${detailOverflow.reasons.join('; ')})`);
  await page.click(`${detailSelector} > summary`);
  await editorLayoutReady(page);
  const detailAfterRestore = await page.eval(`Boolean(document.querySelector('${detailSelector}')?.open)`);
  check(detailAfterRestore === detailBefore, `${stem}: restoring recovery details returns to initial open=${detailBefore}`);
}

async function leaveActionScene(page, scene, stage, expected, disabled = []) {
  const dialog = '#leave-editor';
  const title = `${dialog} .vk-ui-dialog__title`;
  const stem = `editor-leave-${stage}`;
  const name = editorSceneName(stem, scene);
  const read = () => Promise.all(expected.map(id => page.eval(buildTargetGeometryScript(`#${id}`))));
  await editorLayoutReady(page);
  const modalTop = await page.eval(buildModalInspectionScript(dialog));
  const branchTopEval = evaluateLeaveModalBranch(modalTop, stage);
  check(branchTopEval.ok, `${name}: leave modal branch (${stage}) passes (${branchTopEval.reasons.join('; ')})`);
  const inventory = await page.eval(`[...document.querySelectorAll('${dialog} button')].map(b => b.id)`);
  check(inventory.join() === expected.join(), `${name}: exact action inventory ${JSON.stringify(inventory)}`);
  const anatomy = await page.eval(`(() => {
    const d = document.querySelector('${dialog}');
    const panel = d.querySelector('.vk-ui-dialog__panel');
    const ps = getComputedStyle(panel);
    const padLeft = parseFloat(ps.paddingLeft), padRight = parseFloat(ps.paddingRight);
    const padTop = parseFloat(ps.paddingTop), padBottom = parseFloat(ps.paddingBottom);
    const ctx = d.querySelector('.vk-ui-dialog__context');
    const filled = [...d.querySelectorAll('.vk-ui-button--filled')].map(b => b.id);
    const secondary = d.querySelector('.vk-ui-dialog__actions--secondary');
    const secondaryBorder = secondary ? parseFloat(getComputedStyle(secondary).borderTopWidth) : 0;
    const classes = Object.fromEntries([...d.querySelectorAll('button')].map(b => [b.id, [...b.classList]]));
    return { padLeft, padRight, padTop, padBottom, ctxId: ctx?.id ?? null, ctxText: ctx?.textContent?.trim() ?? '', filled, secondaryBorder, classes };
  })()`);
  const minGutter = scene.width >= 390 && scene.scale === 100 ? 24 : 12;
  check(anatomy.padLeft >= minGutter - 0.5 && Math.abs(anatomy.padLeft - anatomy.padRight) < 1 &&
      Math.abs(anatomy.padTop - anatomy.padBottom) < 1 && anatomy.ctxId === 'leave-editor-context' &&
      anatomy.ctxText.length > 0 && anatomy.filled.length <= 1 && anatomy.secondaryBorder === 0,
    `${name}: shared dialog anatomy, gutter (>=${minGutter}px), divider-free actions and <=1 filled CTA ${JSON.stringify(anatomy)}`);
  if (stage === 'confirm') {
    check(anatomy.classes['discard-back']?.includes('vk-ui-button--tonal') &&
      anatomy.classes['discard-confirm']?.includes('vk-ui-button--danger'),
      `${name}: confirm stage hierarchy Quay lại=Tonal, confirm=Danger`);
  } else if (stage.includes('conflict')) {
    check(anatomy.classes['leave-stay']?.includes('vk-ui-button--filled') &&
      !anatomy.classes['leave-export'] &&
      anatomy.classes['leave-discard']?.includes('vk-ui-button--danger-text'),
      `${name}: conflict hierarchy Stay=Filled, Discard=DangerText`);
  } else {
    check(anatomy.classes['leave-stay']?.includes('vk-ui-button--text') &&
      anatomy.classes['leave-save']?.includes('vk-ui-button--filled') &&
      !anatomy.classes['leave-export'] &&
      (!anatomy.classes['leave-discard'] || anatomy.classes['leave-discard']?.includes('vk-ui-button--danger-text')),
      `${name}: saveable hierarchy Stay=Text, Save=Filled, Discard=DangerText`);
  }
  await page.eval(`document.querySelector('${title}').focus({preventScroll:true}); document.querySelector('${dialog}').scrollTop = 0`);
  const top = await read();
  const dialogRect = await page.eval(`document.querySelector('${dialog}').getBoundingClientRect().toJSON()`);
  const scrollBefore = top[0].page.scrollY;
  await captureEditorScene(page, `${stem}-top`, scene, {
    stage,
    canDiscard: branchTopEval.canDiscard,
    scroll: modalTop.scroll,
    modalActions: modalTop.actions,
    actions: top,
    requiredTargets: [dialog, ...expected.map(id => `#${id}`)],
  });

  // Scroll inside the dialog with wheel events. The last action must be wholly visible at
  // the actual end, including when some controls above it are now outside the scrollport.
  for (let i = 0; i < 2; i++) {
    await page.send('Input.dispatchMouseEvent', { type: 'mouseWheel', deltaX: 0, deltaY: 2400,
      x: dialogRect.x + dialogRect.width / 2, y: dialogRect.y + dialogRect.height / 2 });
    await editorLayoutReady(page);
  }
  const modalBottom = await page.eval(buildModalInspectionScript(dialog));
  const branchBottomEval = evaluateLeaveModalBranch(modalBottom, stage);
  check(branchBottomEval.ok, `${name}: leave modal bottom branch (${stage}) passes (${branchBottomEval.reasons.join('; ')})`);
  const bottom = await read();
  const last = bottom.at(-1);
  check(fitsEditorAction(last) && last.owner.id === 'leave-editor' && last.page.scrollY === scrollBefore &&
      Math.abs(last.owner.scrollHeight - last.owner.clientHeight - last.owner.scrollTop) <= 1,
    `${name}: wheel to the end reveals the last action without scrolling the page ${JSON.stringify(last)}`);
  await captureEditorScene(page, `${stem}-bottom`, scene, {
    stage,
    canDiscard: branchBottomEval.canDiscard,
    scroll: modalBottom.scroll,
    modalActions: modalBottom.actions,
    actions: bottom,
    requiredTargets: [dialog, ...expected.map(id => `#${id}`)],
  });

  await page.eval(`document.querySelector('${title}').focus({preventScroll:true}); document.querySelector('${dialog}').scrollTop = 0`);
  for (const id of expected) {
    await page.key('Tab');
    // In confirmation, the negative tabindex heading still precedes both buttons.
    const m = await page.eval(buildTargetGeometryScript(`#${id}`));
    check(fitsEditorAction(m) && m.focused && m.focusRing && m.disabled === disabled.includes(id) &&
        (!m.disabled || m.description) && m.page.scrollY === scrollBefore,
      `${name}: Tab reveals ${id} with its expected state ${JSON.stringify(m)}`);
    await captureEditorScene(page, `${stem}-focus-${id}`, scene, {
      stage,
      canDiscard: branchTopEval.canDiscard,
      action: m,
      requiredTargets: [dialog, `#${id}`],
    });
  }
  for (const id of expected.slice(0, -1).reverse()) {
    await page.key('Tab', SHIFT);
    const m = await page.eval(buildTargetGeometryScript(`#${id}`));
    check(fitsEditorAction(m) && m.focused && m.focusRing,
      `${name}: Shift+Tab reveals ${id} ${JSON.stringify(m)}`);
  }
}

async function editorEvidenceNegativeControls(page, scene) {
  const last = '#leave-discard';
  await page.eval(`document.querySelector('${last}').scrollIntoView({block:'center',behavior:'instant'})`);
  const original = await page.eval(buildTargetGeometryScript(last));
  check(fitsEditorAction(original), 'negative control starts with a reachable last action');
  try {
    await page.eval(`(() => {
      const dialog = document.querySelector('#leave-editor');
      dialog.style.overflowX = 'hidden';
      const overflow = document.createElement('span');
      overflow.id = 'editor-overflow-fixture';
      overflow.style.cssText = 'position:absolute;top:0;left:0;width:200vw;height:1px;pointer-events:none';
      dialog.append(overflow);
    })()`);
    const overflow = await page.eval(buildTargetGeometryScript(last));
    const hiddenOverflowEval = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({ containers: ['#leave-editor'] })));
    check(!fitsEditorAction(overflow) && overflow.owner.scrollWidth > overflow.owner.clientWidth &&
      overflow.page.scrollX === original.page.scrollX && !hiddenOverflowEval.ok && hiddenOverflowEval.unintendedOverflows.length > 0,
      'oracle rejects horizontally overflowing content inside a clipped scroll owner even when scrollX === 0');
    EVIDENCE_STATE.negativeControls.overflowWithoutScrollX = 'PASS';

    await page.eval(`document.querySelector('#editor-overflow-fixture').remove(); document.querySelector('#leave-editor').style.overflowX=''`);
    await page.eval(`document.querySelector('${last}').style.transform='translateX(-200vw)'`);
    const clipped = await page.eval(buildTargetGeometryScript(last));
    const clippedEval = evaluateTargetGeometry(clipped);
    check(!fitsEditorAction(clipped) && !clippedEval.ok && clipped.page.scrollX === original.page.scrollX &&
      clipped.page.scrollWidth === original.page.scrollWidth, 'oracle rejects a clipped action even with unchanged page scrollX and scrollWidth');
    await page.eval(`document.querySelector('${last}').style.visibility='hidden'`);
    const hidden = await page.eval(buildTargetGeometryScript(last));
    const hiddenEval = evaluateTargetGeometry(hidden);
    check(!hidden.visible && !hiddenEval.ok, 'oracle rejects a visibility:hidden target');
    await page.eval(`document.querySelector('${last}').style.visibility=''; document.querySelector('${last}').style.transform=''`);
    EVIDENCE_STATE.negativeControls.clippedOrHiddenTarget = 'PASS';

    await page.eval(`document.querySelector('${last}').id='missing-last-action'`);
    const missingLastTarget = await page.eval(buildTargetGeometryScript(last));
    const missingLastEval = evaluateTargetGeometry(missingLastTarget);
    const missingLastModalEval = evaluateLeaveModalBranch(await page.eval(buildModalInspectionScript('#leave-editor')), 'text-conflict');
    check(!missingLastTarget.present && missingLastEval.status === 'INCOMPLETE' && !missingLastModalEval.ok,
      'oracle reports a missing last action distinctly from clipping');
    EVIDENCE_STATE.negativeControls.missingLastModalAction = 'PASS';
  } finally {
    await page.eval(`const el = document.querySelector('#missing-last-action,${last}'); if (el) { el.id='leave-discard'; el.style.transform=''; el.style.visibility=''; }`);
    await page.eval(`document.querySelector('#editor-overflow-fixture')?.remove(); document.querySelector('#leave-editor').style.overflowX=''`);
  }
  const { width, height, scale, theme } = scene;
  const actual = { width, height, fontSize: `${16 * scale / 100}px`, theme, dpr: 1 };
  let rejected = false;
  try {
    validateEditorCapture(editorSceneName('editor-negative', { ...scene, height: height + 100 }),
      scene, actual, { width, height });
  } catch (error) {
    rejected = error.message.includes('capture scene mismatch') || error.message.includes('height mismatch');
  }
  let silent800Rejected = false;
  try {
    validateCaptureScene({
      name: `nav-ban-viet-${width}x800-200-${theme}`,
      requestedScene: { width, height: 800, scale: 200, theme },
      actual: { width, height: 900, fontSize: '32px', theme, dpr: 1 },
      pngMeta: { width, height: 900 },
    });
  } catch (error) {
    silent800Rejected = error.message.includes('mismatched viewport filename') || error.message.includes('height mismatch');
  }
  check(rejected && silent800Rejected, 'oracle rejects silent 900-height substitution and mismatched viewport filenames');
  EVIDENCE_STATE.negativeControls.mismatchedViewportFilename = 'PASS';

  const missingScenarioTarget = await page.eval(buildTargetGeometryScript('#nonexistent-vink-target-195'));
  const currentRoute = await page.eval('location.pathname');
  const missingTargetEval = evaluateTargetGeometry(missingScenarioTarget);
  const incompleteManifest = validateManifestRecord('missing-target-scene', {
    schemaVersion: MANIFEST_SCHEMA_VERSION,
    scenarioId: 'negative-control-missing-target',
    captureStatus: 'INCOMPLETE',
    state: 'failed',
    hydrated: true,
    route: currentRoute,
    width,
    layoutWidth: width,
    height,
    dpr: 1,
    theme,
    fontSize: actual.fontSize,
    captured_at: new Date().toISOString(),
    viewport: { width, height, layoutWidth: width, clientWidth: width, clientHeight: height, dpr: 1 },
    scrollOwner: { tagName: 'HTML' },
    source: { sourceSHA: SOURCE_IDENTITY.sourceSHA, dirtyInputs: SOURCE_IDENTITY.dirtyInputs },
    png: { sha256: 'a'.repeat(64), width, height, byteSize: 128 },
    targets: { required: [missingScenarioTarget] },
  });
  check(missingTargetEval.status === 'INCOMPLETE' && !incompleteManifest.ok,
    'oracle marks missing scenario target INCOMPLETE and blocks PASS manifest certification');
  EVIDENCE_STATE.negativeControls.missingTargetIncomplete = 'PASS';
}

async function setEditorScene(page, { width, height, scale, theme }) {
  await page.viewport(width, height);
  await page.eval(`document.documentElement.style.fontSize='${scale}%'; document.documentElement.dataset.theme='${theme}'`);
  await editorLayoutReady(page);
}

async function editorChrome(page) {
  await editorPaperFirst(page, { args, check, openProfile, chooseTheme, editorTarget, fitsEditorAction });
  await editorWriting(page, { args, check, setEditorScene, captureEditorScene, editorTarget, fitsEditorAction });
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
    for (const width of [1536, 1440, 1024, 834, 390, 360, 320]) {
      for (const scale of [100, 200]) {
        for (const theme of ['light', 'dark']) {
          const height = 900;
          await page.viewport(width, height);
          await page.eval(`document.documentElement.style.fontSize = '${scale}%'; document.documentElement.dataset.theme = '${theme}'; scrollTo(0, 0)`);
          await page.eval(`document.fonts.ready.then(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))`);
          // Closed native details can leave chip transitions idle with unresolved finished
          // promises. Poll current visible transitions instead of holding stale promises.
          await page.waitFor(`!document.getAnimations().some(a => a.playState === 'running' && a.effect.target?.checkVisibility() && Number.isFinite(a.effect.getComputedTiming().endTime))`, 'visible editor transitions settled');
          const scenario = `editor-${state}-${width}-${scale}-${theme}`;
          if (width === 1440 && scale === 100) await textContrast(page, theme, `editor-${state}`);
          const overflow = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({
            containers: ['.vk-editor', '.vk-editor-paper', '#editor-toolbar'],
          })));
          check(overflow.ok && await page.eval(`document.documentElement.scrollWidth <= innerWidth && ${live}`),
            `${scenario}: no unintended overflow (${overflow.reasons.join('; ')}) and one live EditorView`);
          check(await page.eval(`(() => {
            const toolbar = document.querySelector('#editor-toolbar'), t = toolbar.getBoundingClientRect();
            return [...toolbar.querySelectorAll('button,summary')].filter(el=>el.checkVisibility()).every(button => {
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
            const expected = { clean:'Đã lưu', dirty:'Chưa lưu chữ', saving:'Đang lưu; có chữ mới…', failed:'Chưa xác nhận lưu.', conflict:'Có bản mới trên máy chủ.' };
            const cleanSlotOk = '${state}' !== 'clean' || (!document.querySelector('#save') && Boolean(document.querySelector('#save-slot .vk-save-slot__icon svg')));
            const problemSlotOk = '${state}' !== 'conflict' || (!document.querySelector('#save') && Boolean(document.querySelector('#save-recovery-jump')));
            return flat && cleanSlotOk && problemSlotOk &&
              document.querySelector('[data-testid=save-status]').textContent === expected['${state}'] &&
              getComputedStyle(document.querySelector('[data-testid=editor-body]')).backgroundColor === 'rgba(0, 0, 0, 0)' &&
              getComputedStyle(document.querySelector('.vk-editor-paper .vk-ui-field__control')).backgroundColor === 'rgba(0, 0, 0, 0)' &&
              document.querySelectorAll('#editor-toolbar [role=status],.vk-editor-recovery [role=status],.vk-editor-recovery [role=alert]').length === 1 &&
              [...document.querySelectorAll('.vk-editor [aria-describedby]')].every(el => el.getAttribute('aria-describedby').split(/\\s+/).every(id => document.getElementById(id))) &&
              !document.querySelector('.vk-editor-paper [aria-invalid=true]') &&
              getComputedStyle(document.querySelector('#editor-body-hint')).position === 'absolute';
          })()`), `${scenario}: one uninterrupted writing sheet, flat resting chrome, complete descriptions and one live status owner`);

          await page.capture(scenario, {
            scenario: `editor-${state}`,
            state,
            requestedScene: { width, height, scale, theme },
            textScaleLane: 'dom-override-geometry-only',
            requiredTargets: ['#draft-title', '[data-testid=editor-body]', '#editor-toolbar'],
            overflow,
          });
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
    // Let the restored viewport deliver its resize before focusing a tooltip; a
    // resize dismisses tooltips by design, even when the browser has already focused.
    await page.eval(`document.documentElement.style.fontSize = ''; document.documentElement.dataset.theme = 'light'; scrollTo(0,0);
      new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
  };
  await geometry('clean');
  await page.eval(`document.querySelector('#editor-save-details > summary').focus()`);
  await page.key('Enter');
  check(await page.eval(`document.querySelector('#editor-save-details').open && document.querySelector('#editor-save-details').textContent.includes('phiên bản 2') && document.querySelector('#editor-save-details').textContent.includes('trên máy chủ') && !document.querySelector('#editor-save-details').textContent.includes('Đã lưu trên máy') && !document.querySelector('#editor-save-details').textContent.includes('tự động') && getComputedStyle(document.querySelector('#editor-toolbar')).position==='static' && ${live}`), 'native tools expand in document flow and state server truth without device/autosave claims');
  await page.key('Enter');
  // Clean activation, clean Ctrl/Cmd+S and clean title Enter must never issue a redundant PUT.
  let cleanWrites = 0;
  const cleanPutListener = (message) => {
    if (message.sessionId === page.sessionId && message.method === 'Network.requestWillBeSent' && message.params.request.method === 'PUT') cleanWrites++;
  };
  page.cdp.listeners.push(cleanPutListener);
  await page.click('#save-slot .vk-save-status');
  await page.eval(`document.querySelector('#draft-title').focus()`);
  await page.key('Enter');
  await page.key('s', page.mod);
  await sleep(200);
  page.cdp.listeners = page.cdp.listeners.filter((item) => item !== cleanPutListener);
  check(cleanWrites === 0 && (await page.status()) === 'saved', 'clean save slot, clean Ctrl/Cmd+S and clean title Enter issue zero PUT writes');
  // Preview is direct and precedes Thêm; closed details reach the native title directly.
  await page.eval(`document.querySelector('#preview').focus()`);
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('#editor-save-details > summary') && document.activeElement.checkVisibility()`), 'clean Tab reaches the visible tools trigger');
  await page.key('Tab');
  check(await page.eval(`document.activeElement.id === 'draft-title'`), 'closed Thêm reaches the native title directly');
  for (const id of ['preview']) {
    await page.eval(`document.querySelector('#${id}').focus()`);
    await page.waitFor(`!document.querySelector('#${id}-tooltip').hidden && getComputedStyle(document.querySelector('#${id}-tooltip')).visibility === 'visible'`, `${id} focus tooltip`);
    check(await page.eval(`document.querySelector('#${id}').getAttribute('aria-describedby').includes('${id}-tooltip') && document.querySelector('#${id}-tooltip').getAttribute('role') === 'tooltip' && document.activeElement.id === '${id}'`), `${id} has its own native description and focus tooltip`);
    await page.key('Escape');
    check(await page.eval(`document.querySelector('#${id}-tooltip').hidden && document.activeElement.id === '${id}'`), `${id} Escape dismisses only the tooltip`);
  }
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
  await page.eval(`document.querySelector('#save').focus()`);
  await page.waitFor(`!document.querySelector('#save-tooltip').hidden && getComputedStyle(document.querySelector('#save-tooltip')).visibility === 'visible'`, 'save focus tooltip');
  check(await page.eval(`document.querySelector('#save').getAttribute('aria-describedby').includes('save-tooltip') && document.querySelector('#save-tooltip').getAttribute('role') === 'tooltip' && document.querySelector('#save-tooltip').textContent.includes(${JSON.stringify(page.mac ? '⌘ + S' : 'Ctrl + S')})`), 'dirty save button keeps platform shortcut tooltip');
  await page.key('Escape');
  check(await page.eval(`document.querySelector('#save-tooltip').hidden && document.activeElement.id === 'save'`), 'save Escape dismisses only the tooltip');
  // shellWithDirtyEditor in the main phase checks native selection and unsaved text
  // through actual Giao diện choices; its short fixture avoids DOM virtualization.
  await page.eval(`document.querySelector('.cm-content').dispatchEvent(new KeyboardEvent('keydown', {key:'s',ctrlKey:true,isComposing:true,bubbles:true,cancelable:true}))`);
  check((await page.status()) === 'unsaved', 'composing shortcut never saves a partial syllable (synthetic guard check)');
  check(!await page.eval(`Boolean(document.querySelector('.vk-editor-help'))`), 'quiet editor removes long line-break writing help');
  // Save -> direct preview -> visible native tools -> title, full-title disclosure, body.
  await page.eval(`document.querySelector('#save').focus()`);
  await page.key('Tab');
  check(await page.eval(`document.activeElement.id==='preview'`), 'dirty editor Tab reaches direct preview after Save');
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('#editor-save-details > summary') && document.activeElement.checkVisibility()`), 'dirty editor Tab reaches visible tools');
  await page.key('Tab');
  check(await page.eval(`document.activeElement.id==='draft-title'`), 'dirty closed Thêm reaches the native title directly');
  await page.key('Tab');
  check(await page.eval(`document.activeElement.matches('#draft-title-details > summary') && !document.querySelector('#draft-title-details').open`), 'editor Tab reaches the collapsed full-title disclosure');
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
  await page.eval(`document.querySelector('#save').focus()`);
  await page.key('Enter');
  const first = await gate.next();
  await page.waitStatus('saving');
  check(await page.eval(`document.activeElement?.id === 'save' && document.querySelector('[data-testid=save-status]').textContent === 'Đang lưu…' && document.querySelector('#save').textContent.includes('Đang lưu…')`), 'keyboard Enter on #save preserves focus and Vietnamese pending text');
  await page.eval(`document.querySelector('#save').click(); document.querySelector('#save').click()`);
  await page.key('s', page.mod);
  await page.typeInBody(' Gõ tiếp khi chờ.');
  await geometry('saving');
  check(gate.held.length === 0, 'double click and save shortcut cannot dispatch another pending PUT');
  await gate.release(first);
  await page.waitStatus('unsaved');
  check((await page.draft(workId)).document.source.endsWith('\n\nChữ mới e\u0302\u0301 — chưa lưu.'), 'ack only persisted its snapshot (exact NFD text, blank lines), not newer typing');
  await page.eval(`document.querySelector('#save').focus()`);
  await page.key('Space');
  await gate.fail(await gate.next());
  await page.waitStatus('failed');
  check(await page.eval(`document.activeElement?.id === 'save' && document.querySelector('#save').textContent.includes('Thử lưu lại')`), 'Space on #save keeps focus on retry button after failure');
  await geometry('failed');
  for (const scene of [
    { width: 320, height: 900, scale: 200, theme: 'light' },
    { width: 320, height: 800, scale: 200, theme: 'dark' },
    { width: 390, height: 420, scale: 200, theme: 'light' },
    { width: 1440, height: 900, scale: 100, theme: 'dark' },
  ]) {
    await setEditorScene(page, scene);
    await editorRecoveryScene(page, '[data-testid=save-failed]', scene, 'editor-failed');
  }
  await setEditorScene(page, { width: 1440, height: 900, scale: 100, theme: 'light' });
  check(await page.eval(`document.querySelector('[data-testid=save-failed]').textContent.includes('chưa rõ') && document.querySelector('[data-testid=save-failed] .vk-ui-notice__message').textContent === 'Nội dung đang viết vẫn còn trên trang này. Chọn Thử lại hoặc giữ trang mở.' && !document.querySelector('#save-failure-details').open && !document.querySelector('#save-failure-details').contains(document.querySelector('#retry-save')) && !document.querySelector('#export-failed')`), 'unconfirmed notice keeps stay/retry guidance and recovery actions outside optional detail without download');
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
  check(paragraphs((await page.draft(workId)).document).at(-1).lines.at(-1).endsWith('Gõ tiếp khi chờ.'), 'deliberate retry saves the current text after reconciliation');
  const savedBeforeUnavailable = (await page.draft(workId)).version;
  await page.typeInBody(' Gõ lúc máy chủ tạm nghỉ.');
  await page.click('#save');
  await page.send('Fetch.fulfillRequest', {
    requestId: await gate.next(), responseCode: 503,
    responseHeaders: [{ name: 'content-type', value: 'application/json' }],
    body: Buffer.from(JSON.stringify({ error: { code: 'dependency_unavailable', message: 'CDP fixture' } })).toString('base64'),
  });
  await page.waitStatus('failed');
  check(await page.eval(`document.querySelector('[data-testid=save-status]').textContent === 'Chưa xác nhận lưu.' &&
    document.querySelector('#save-failure-details').textContent.includes('Máy chủ tạm không phục vụ')`) &&
    (await page.draft(workId)).version === savedBeforeUnavailable, 'injected 503 leaves text unconfirmed and identifies the unavailable server');
  await page.click('#retry-save');
  await gate.release(await gate.next());
  await page.waitStatus('saved');
  await gate.disable();

  // Response stage: server has really committed. A retry must GET and enter conflict, with
  // no second PUT even when that first reconciliation itself fails (403 here).
  const responseGate = new SaveGate(page, 'Response');
  await responseGate.enable();
  await page.typeInBody(' Đã tới máy chủ.');
  await page.click('#save');
  const response = await responseGate.next();
  const committed = await page.draft(workId);
  check(paragraphs(committed.document).at(-1).lines.at(-1).endsWith('Đã tới máy chủ.'), 'held response belongs to a real committed save');
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
  check(await page.eval(`document.querySelector('[data-testid=save-status]').textContent === 'Chưa lưu được.'`), 'rejected write is distinguishable from an unconfirmed response');
  check(writes === 0, 'failed reconciliation cannot write');
  await page.typeInBody(' Chữ còn trên trang.');
  await page.click('#retry-save');
  await page.waitStatus('conflict');
  check(writes === 0, 'retry after a failed reconciliation still reads before writing');
  await page.send('Fetch.disable');
  page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
  await geometry('conflict');
  await page.eval(`document.querySelector('#save-recovery-jump').focus()`);
  await page.key('Enter');
  check(await page.eval(`document.activeElement?.id === 'editor-text-conflict'`), 'conflict toolbar jump focuses the recovery region before its choices');
  await page.key('Enter');
  check((await page.draft(workId)).version === committed.version, 'another Enter on the recovery region cannot confirm overwrite');
  await page.key('Tab');
  check(await page.eval(`document.activeElement?.id === 'keep-mine'`), 'the explicit overwrite remains next in keyboard order');
  check((await page.draft(workId)).version === committed.version, 'unknown outcome never blindly replays an already committed save');
  for (const scene of [
    { width: 320, height: 800, scale: 200, theme: 'light' },
    { width: 390, height: 844, scale: 200, theme: 'dark' },
  ]) {
    await setEditorScene(page, scene);
    await editorRecoveryScene(page, '[data-testid=conflict]', scene, 'editor-conflict');
  }
  await setEditorScene(page, { width: 1440, height: 900, scale: 100, theme: 'light' });

  // Dialogs use current unsaved text and retain focus/scroll/body through close and reopen.
  for (const kind of ['preview', 'leave']) {
    await page.click(kind === 'preview' ? '#preview' : '.vk-editor-heading__back');
    const dialog = kind === 'preview' ? '#draft-preview' : '#leave-editor';
    await page.waitFor(`document.querySelector('${dialog}').open`, `${kind} dialog`);
    if (kind === 'preview') check(await page.eval(`document.querySelector('#draft-preview').textContent.includes('Chữ còn trên trang.') && document.querySelector('#draft-preview-context')?.classList.contains('vk-ui-dialog__context') && document.querySelector('#preview-close')?.classList.contains('vk-ui-button--tonal')`), 'preview reads unsaved text during conflict and uses shared context + Tonal close');
    for (const width of [1440, 1024, 834, 390, 320]) for (const scale of [100, 200]) for (const theme of ['light', 'dark']) {
      await page.viewport(width);
      await page.eval(`document.documentElement.style.fontSize = '${scale}%'; document.documentElement.dataset.theme = '${theme}'; document.querySelector('${dialog}').scrollTop = 0`);
      await page.waitFor(`!document.getAnimations().some(a => a.playState === 'running' && a.effect.target?.checkVisibility() && Number.isFinite(a.effect.getComputedTiming().endTime))`, 'visible editor transitions settled');
      await page.capture(`editor-${kind}-${width}-${scale}-${theme}`);
      check(await page.eval(`(() => { const d = document.querySelector('${dialog}'), r = d.getBoundingClientRect(), p = parseFloat(getComputedStyle(d.querySelector('.vk-ui-dialog__panel')).paddingLeft); return r.left >= 0 && r.right <= innerWidth && r.top >= 0 && r.bottom <= innerHeight && d.scrollWidth <= d.clientWidth + 1 && (${width} >= 390 && ${scale} === 100 ? p >= 23.5 : p >= 11.5) && ${live}; })()`), `${kind} ${width}/${scale}/${theme}: dialog fits, shared gutter holds and text view survives`);
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
  const conflictChoices = ['leave-stay', 'leave-save', 'leave-discard'];
  const discardChoices = ['discard-back', 'discard-confirm'];
  const shortScene = { width: 390, height: 420, scale: 200, theme: 'dark' };
  const conflictScenes = [320, 390, 1440].flatMap(width =>
    [100, 200].flatMap(scale => ['light', 'dark'].map(theme =>
      ({ width, height: width === 320 ? 800 : width === 390 ? 844 : 900, scale, theme }))));
  conflictScenes.push(shortScene, { width: 320, height: 900, scale: 200, theme: 'dark' });
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('#leave-editor').open`, 'reopened conflict choices');
  for (const scene of conflictScenes) {
    await setEditorScene(page, scene);
    await leaveActionScene(page, scene, 'text-conflict', conflictChoices, ['leave-save']);
  }
  await setEditorScene(page, shortScene);
  await editorEvidenceNegativeControls(page, shortScene);
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  for (const label of ['Ở lại', 'Lưu và rời trang', 'Rời mà không lưu…']) {
    const action = nodes.find(node => node.role.value === 'button' && node.name?.value === label);
    check(action && (label !== 'Lưu và rời trang' || action.properties?.some(p => p.name === 'disabled' && p.value.value)),
      `Chrome AX exposes ${label} with the expected state`);
  }
  check(!nodes.some(node => node.role.value === 'button' && /Tải/.test(node.name?.value ?? '')), 'Chrome AX exposes no draft export button in leave dialog');
  await page.eval(`document.querySelector('#leave-stay').focus()`);
  await page.key('Tab');
  await page.key('Enter');
  check(await page.eval(`document.querySelector('#leave-editor').open && location.pathname.endsWith('${workId}')`) &&
    (await page.draft(workId)).version === committed.version, 'activating disabled Save keeps the text conflict and stored version');
  await page.key('Tab');
  check((await editorTarget(page, '#leave-discard')).focused, 'keyboard reaches discard after disabled Save');
  await page.key('Enter');
  await page.waitFor(`document.activeElement.id === 'discard-back'`, 'confirmation focuses the safe action');
  await leaveActionScene(page, shortScene, 'confirm', discardChoices);
  await page.key('Enter');
  await page.waitFor(`document.activeElement.id === 'leave-discard'`, 'keyboard returns to the original discard control');
  const returned = await editorTarget(page, '#leave-discard');
  check(fitsEditorAction(returned), 'cancel returns to the revealed discard control');
  await captureEditorScene(page, 'editor-leave-confirm-return', shortScene, { action: returned });
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('#leave-editor').open && document.activeElement.matches('.cm-content') && ${live}`, 'Escape restores the writing field and editor instance');
  await page.typeInBody('');
  await page.key('ArrowLeft', SHIFT);
  const caret = `(() => { const s = getSelection(); return { text: s.toString(), anchor: s.anchorOffset, focus: s.focusOffset }; })()`;
  const writingPosition = `({ caret: ${caret}, y: scrollY })`;
  const beforeBack = await page.eval(writingPosition);
  check(beforeBack.caret.text.length > 0 && beforeBack.y > 0, 'Back cancellation starts with writing selected below the fold');
  await page.eval(`history.back()`);
  await page.waitFor(`location.pathname.endsWith('${workId}') && document.querySelector('#leave-editor').open`, 'Back is held at the editor while in conflict');
  await page.click('#leave-stay');
  // The native dialog closes before the next-frame focus restoration and CodeMirror's
  // selection synchronization. Observe the restored state after those have both settled.
  const restored = `!document.querySelector('#leave-editor').open && document.activeElement.matches('.cm-content') &&
    JSON.stringify(${writingPosition}) === ${JSON.stringify(JSON.stringify(beforeBack))} && ${live}`;
  await page.waitFor(restored, 'Stay restores the original selection, caret and page scroll after Back');
  await editorLayoutReady(page);
  check(await page.eval(restored), 'restored selection, caret and scroll remain stable after the next layout');

  await page.viewport(1440, 360);
  await page.eval(`document.documentElement.style.fontSize=''; scrollTo(0,0)`);
  check(await page.eval(`getComputedStyle(document.querySelector('#editor-toolbar')).position === 'static'`), 'short desktop leaves writing viewport clear');
  await page.capture('editor-short-desktop-1440');
  await page.viewport(390, 420);
  // Finish the desktop→mobile reflow before issuing document/line movement. Otherwise
  // Chrome's resize scroll anchoring can run after CodeMirror's keyboard scroll and undo it.
  await editorLayoutReady(page);
  await page.typeInBody('');
  await page.lineStart();
  // CodeMirror schedules selection scrolling after resize. Observe the resulting geometry,
  // without scrolling/focusing on the test's behalf or weakening the viewport bounds.
  const mobileCaretReady = `(() => { if (!document.activeElement?.matches('.cm-content') || !getSelection()?.rangeCount) return false; const r = getSelection().getRangeAt(0).getBoundingClientRect(); return getComputedStyle(document.querySelector('#editor-toolbar')).position === 'static' && r.top >= 0 && r.bottom <= innerHeight; })()`;
  try {
    await page.waitFor(mobileCaretReady, 'reduced mobile viewport keeps the active caret visible without fixed controls', 2000);
  } catch (error) {
    console.error('mobile caret geometry:', await page.eval(`(() => {
      const selection=getSelection(), range=selection?.rangeCount ? selection.getRangeAt(0).getBoundingClientRect() : null;
      return {active:document.activeElement?.className, range:range?.toJSON(), height:innerHeight, scrollY, toolbar:getComputedStyle(document.querySelector('#editor-toolbar')).position};
    })()`));
    await page.capture('editor-short-mobile-caret-failed', {scenario:'existing-editor-resize-caret-regression',boundary:'Chrome-CDP-geometry'});
    throw error;
  }
  await editorLayoutReady(page);
  check(await page.eval(mobileCaretReady), 'reduced mobile viewport keeps the active caret visible without fixed controls');
  await page.capture('editor-short-mobile-390');
  await page.viewport(1440);
  await page.eval(`document.documentElement.dataset.theme = 'light'; document.querySelector('#draft-title').focus(); document.querySelector('#draft-title').scrollIntoView({block:'start'})`);
  check(await page.eval(`document.querySelector('#draft-title').getBoundingClientRect().top >= document.querySelector('#editor-toolbar').getBoundingClientRect().bottom`), 'document-flow tools do not cover the focused title');
  await page.capture('editor-sticky-title-1440');
  await page.click('#keep-mine');
  await page.waitStatus('saved');
  await page.reload();
  await page.waitFor(EDITOR_READY, 'chrome editor reloaded');
  check((await page.draft(workId)).title === title && paragraphs((await page.draft(workId)).document).length === 9, 'long title and poetry survive explicit recovery and reload');
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

  // Verify dirty -> pending -> saved keyboard handoff keeps focus on save-details summary cleanly
  // without leaking focus to <body>, and Ctrl/Cmd+S from body preserves CodeMirror focus.
  await gate.enable();
  await page.eval(`window.__afterReloadEditor = document.querySelector('.cm-editor')`);
  await page.typeInBody(' Kiểm tra focus.');
  await page.waitStatus('unsaved');
  await page.eval(`document.querySelector('#save').focus()`);
  await page.key('Enter');
  const handoffReq = await gate.next();
  await page.waitStatus('saving');
  await gate.release(handoffReq);
  await page.waitStatus('saved');
  await page.waitFor(`document.activeElement?.matches('#editor-save-details > summary')`, 'focus moves cleanly to save details summary when #save hides on clean');
  check(await page.eval(`document.querySelector('#save') === null && document.activeElement?.matches('#editor-save-details > summary') && document.activeElement.checkVisibility() && !document.querySelector('#editor-save-details').open`), 'keyboard save dirty -> pending -> saved hands focus to visible closed tools');
  // Reflow can move the destination to a new row: check actual clipping/hit testing,
  // not only activeElement, in a narrow enlarged-text short viewport.
  await setEditorScene(page, {width:320,height:420,scale:200,theme:'dark'});
  await page.typeInBody(' Lưu trên cửa sổ hẹp.');
  await page.click('#save');
  const narrowAck = await gate.next();
  await gate.release(narrowAck); await page.waitStatus('saved');
  await page.waitFor(`document.activeElement.matches('#editor-save-details > summary')`, 'narrow ACK destination');
  const narrowFocus = await editorTarget(page, '#editor-save-details > summary');
  check(fitsEditorAction(narrowFocus) && narrowFocus.focused && !await page.eval(`document.querySelector('#editor-save-details').open`), '320x420/text200 ACK hands off to a fully visible closed tools trigger');
  await captureEditorScene(page, 'paper-ack-focus', {width:320,height:420,scale:200,theme:'dark'}, {action:narrowFocus});
  await setEditorScene(page, {width:1440,height:900,scale:100,theme:'light'});
  // An already open native disclosure remains open through ACK; the same visible
  // summary is safe in both states. No auto-open or hidden nested focus destination.
  await page.key('Space');
  await page.typeInBody(' Lưu khi công cụ còn mở.');
  await page.eval(`document.querySelector('#save').focus()`);
  await page.key('Enter');
  await gate.release(await gate.next());
  await page.waitStatus('saved');
  await page.waitFor(`document.activeElement.matches('#editor-save-details > summary')`, 'open-tools ACK focus');
  check(await page.eval(`document.querySelector('#editor-save-details').open && document.activeElement.checkVisibility()`), 'save ACK retains open tools and focuses its live summary');
  await page.key('Space');
  await page.eval(`document.querySelector('.cm-content').focus()`);
  await page.typeInBody(' Giữ con trỏ.');
  await page.waitStatus('unsaved');
  await page.key('s', page.mod);
  const cmSaveReq = await gate.next();
  await gate.release(cmSaveReq);
  await page.waitStatus('saved');
  check(await page.eval(`document.activeElement?.classList.contains('cm-content') && window.__afterReloadEditor === document.querySelector('.cm-editor') && document.querySelectorAll('.cm-editor').length === 1`), 'Ctrl/Cmd+S from body keeps focus and live EditorView in place after saved');
  await gate.disable();

  // A pre-write GET is not a PUT already in flight: discard cancels that future mutation.
  const beforeDiscard = await page.draft(workId);
  await gate.enable();
  await page.typeInBody(' Chữ bỏ khi đang đối chiếu.');
  await page.click('#save');
  await gate.fail(await gate.next());
  await page.waitStatus('failed');
  await gate.disable();
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('#leave-editor').open`, 'leave choices in failed save state');
  await setEditorScene(page, shortScene);
  await leaveActionScene(page, shortScene, 'failed', conflictChoices, []);
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('#leave-editor').open`, 'close failed leave dialog before retry reconciliation');
  await setEditorScene(page, { width: 1440, height: 900, scale: 100, theme: 'light' });
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
  await setEditorScene(page, shortScene);
  await page.key('Tab');
  check(fitsEditorAction(await editorTarget(page, '#discard-confirm')) &&
    (await editorTarget(page, '#discard-confirm')).focused, 'Tab reveals confirmation before a destructive activation');
  await page.key('Enter');
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
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('#leave-editor').open`, 'leave while unsaved');
  const openLeave = `document.querySelector('#leave-editor').open`;
  await leaveActionScene(page, shortScene, 'unsaved', conflictChoices.slice(0, 2));
  await page.key('Tab');
  await page.key('Enter');
  const pendingAck = await gate.next();
  await page.waitStatus('saving');
  check(await page.eval(`${openLeave} && document.querySelector('#leave-save').textContent.trim() === 'Đang lưu…'`), 'save-and-leave waits inside the dialog');
  await leaveActionScene(page, shortScene, 'saving', conflictChoices, ['leave-save']);
  await page.click('#leave-discard');
  await page.waitFor(`document.activeElement.id === 'discard-back'`, 'confirmation before ACK');
  await gate.release(pendingAck);
  await page.waitStatus('saved');
  check(await page.eval(`${openLeave} && Boolean(document.querySelector('#discard-confirm'))`), 'ACK cannot navigate from discard confirmation');
  await page.key('Enter');
  await page.waitFor(`document.activeElement.id === 'leave-stay'`, 'focus falls back to Stay after ACK removes discard');
  check(await page.eval(`!document.querySelector('#leave-discard')`), 'settled save removes the discard action');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('#leave-editor').open`, 'stay after ACK');
  await gate.disable();
  // A committed response delivered after disposal cannot move focus in a new draft.
  const disposedGate = new SaveGate(page, 'Response');
  await disposedGate.enable();
  const disposedWork = await page.eval(`location.pathname.split('/').pop()`);
  await page.typeInBody(' ACK sau khi rời bản cũ.');
  await page.eval(`document.querySelector('#save').focus()`); await page.key('Enter');
  const disposedAck = await disposedGate.next();
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('#leave-editor').open`, 'leave with committed response held');
  await page.click('#leave-discard');
  await page.waitFor(`document.querySelector('#discard-confirm')`, 'explicit discard confirmation before disposal');
  await page.click('#discard-confirm');
  await page.waitFor(`location.pathname==='/ban-viet' && document.querySelector('#create-draft')`, 'disposed old editor');
  await page.click('#create-draft'); await page.waitFor(EDITOR_READY, 'new draft before old ACK');
  const nextWork = await page.eval(`location.pathname.split('/').pop()`);
  await page.eval(`document.querySelector('#draft-title').focus(); window.__newDraftTitle=document.activeElement`);
  await disposedGate.release(disposedAck);
  await sleep(700);
  check(nextWork!==disposedWork && await page.eval(`location.pathname==='/ban-viet/${nextWork}' && document.activeElement===__newDraftTitle && document.activeElement.checkVisibility() && !document.querySelector('#editor-save-details').open`), 'disposed draft late ACK cannot steal title focus or open tools in a new draft');
  await disposedGate.disable();
  // The rest of mainPhase uses the desktop rail after opening a fresh reader document.
  await page.viewport(1440);
  await page.eval(`document.documentElement.style.fontSize=''; document.documentElement.dataset.theme='light'`);
}

async function mainPhase(page) {
  await serverHtml();
  await readerBeforeHydration(page, { args, check });
  await conditionalPublicReads(page);
  await tracePublicReadLifecycle(page);
  await expressionPilot(page, args, check);
  await discussionReadingAndWrites(page);
  await endorsementAsAuthor(page, { args, check });
  await adaptiveShell(page);
  await bottomNavigationRegression(page);
  // The nav matrix persists its last page-theme choice; later main steps start from the
  // system preference that adaptiveShell restored, not from a leaked explicit theme.
  await page.viewport(1440, 900);
  await page.eval(`localStorage.removeItem('vink.ui.theme'); document.documentElement.style.fontSize=''; delete document.documentElement.dataset.theme; scrollTo(0, 0)`);
  await authorWorks(page, {args,check,DraftGate,sleep});
  await draftDashboard(page);
  // Session-generation fixtures must not broadcast into subsequent real Desk scenarios.
  const { browserContextId: emptyContext } = await page.cdp.send('Target.createBrowserContext');
  try {
    const emptyPage = await openPage(page.cdp, emptyContext);
    await deskEmptyState(emptyPage, {args,check,DraftGate,textContrast});
    check(emptyPage.errors.length === 0, '#259 isolated empty-state context has no console errors');
  } finally {
    await page.cdp.send('Target.disposeBrowserContext', { browserContextId: emptyContext });
  }
  await draftR6Hierarchy(page);
  await timestampPresentation(page);
  await draftPagination(page, { args, check, DraftGate, sleep });
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
  await page.waitFor(`/^\\/ban-viet\\/[0-9a-f-]{36}$/.test(location.pathname) && ${EDITOR_READY} && ${TAGS_READY}`, 'new draft editor and tags');
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
  check(stored.document.format === 'default' && stored.document.source === BODY && JSON.stringify(paragraphs(stored.document)) === JSON.stringify([
    { lines: ['Sáng nay trời đổ mưa rào,', '  ngõ nhỏ ướt mềm từng viên gạch cũ.'] },
    { lines: ['Người về có nhớ hay sao —', 'ỹ ỷ ữ ợ Đ đ.'] },
  ]), 'server stored the exact typed source: stanzas, lines, indentation and diacritics');

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
    body: JSON.stringify({ expected_version: 2, title: 'Tab khác', document: { format: 'default', source: 'Bản của tab khác.' } }),
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
    check(afterLeave.version === 6 && paragraphs(afterLeave.document).at(-1).lines.at(-1) === `${LAST_LINE} Thêm Lui lại.`, 'left only after the server confirmed the latest text (version 6)');

    console.log('cancelling a save-and-leave in flight:');
    const gate = new SaveGate(page);
    try {
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

    console.log('persistent failure: explicit discard and late acknowledgement:');
    await page.typeInBody(' Mất kết nối.');
    await page.click('#navigation-primary a[href="/"]');
    await page.waitFor(`document.querySelector('[data-testid=leave-editor]').open`, 'leave dialog');
    await page.click('#leave-save');
    await gate.fail(await gate.next());
    await page.waitStatus('failed', 'failed save');
    check((await page.eval(`document.querySelector('#leave-editor-context').textContent`)).includes('Chưa lưu được'), 'dialog explains the failed save');
    check((await page.eval(`location.pathname`)) === editorPath && (await page.body()).endsWith(' Mất kết nối.'), 'failure keeps the route and the text');
    await page.click('#leave-save');
    const late = await gate.next();
    await page.click('#leave-discard');
    await page.waitFor(`document.querySelector('#discard-confirm')`, 'discard confirmation');
    await page.waitFor(`document.activeElement?.id === 'discard-back'`, 'confirmation focus after its animation frame');
    check(await page.eval(`document.activeElement?.id === 'discard-back'`), 'confirmation focuses “Quay lại”, not the loss');
    await page.click('#discard-confirm');
    await page.waitFor(`location.pathname === '/'`, 'left without saving');
    await page.click('#navigation-primary a[href="/ban-viet"]');
    await page.waitFor(`document.querySelector('#draft-search') && document.querySelector('[data-testid=draft-content]')?.getAttribute('aria-busy') === 'false'`, 'desk first page');
    // Other tests create more than one page. Find the intended draft through the real query.
    const otherDraft = await page.draft(SAMPLE_WORK);
    await page.eval(`document.querySelector('#draft-search').focus(); document.querySelector('#draft-search').select()`);
    await page.send('Input.insertText', { text: otherDraft.title });
    await page.key('Enter');
    await page.waitFor(`document.querySelector('a[href="/ban-viet/${SAMPLE_WORK}"]')`, 'desk list');
    await page.click(`a[href="/ban-viet/${SAMPLE_WORK}"]`);
    await page.waitFor(`location.pathname === '/ban-viet/${SAMPLE_WORK}' && ${EDITOR_READY}`, 'another draft');
    await page.typeInBody(' Bản khác.');
    await gate.release(late);
    await sleep(1000);
    check((await page.eval(`location.pathname`)) === `/ban-viet/${SAMPLE_WORK}`, 'the late acknowledgement did not navigate again');
    check((await page.status()) === 'unsaved', 'nor marked the other draft saved');
  } finally {
    await gate.disable();
  }

  await draftTags(page, workId);
  await deskRefusesLossyOpen(page);
  await markdownLivePreviewAndFormatPicker(page);
  await editorDeskRegressions(page, { args, check });
  await markdownBudget(page, { args, check });

  const kept = await page.draft(workId);
  const text = kept.document.source;
  writeFileSync(args.state, JSON.stringify({ workId, title: kept.title, text, version: kept.version }));
}

// #255/#263/#256: Markdown opens in Live Preview without a separate title box, while CRLF
// line endings that CodeMirror normalizes are still refused before opening.
async function deskRefusesLossyOpen(page) {
  console.log('desk opens Markdown cleanly and refuses lossy CRLF drafts:');
  {
    const doc = { format: 'markdown', source: '# Mưa đầu mùa\n\nThân bài.\n' };
    const created = await page.eval(`(async () => {
      const post = await fetch('/api/v1/drafts', { method: 'POST' });
      const draft = await post.json();
      const put = await fetch('/api/v1/drafts/' + draft.work_id, { method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ expected_version: draft.version, document: ${JSON.stringify(doc)} }) });
      return { id: draft.work_id, status: put.status };
    })()`);
    check(created.status === 200, 'Markdown draft saved through the API');
    await page.goto(`${args.base}/ban-viet/${created.id}`);
    await page.waitFor(EDITOR_READY, 'Markdown draft opens in Bàn viết');
    const mdDiag = await page.eval(`(() => ({
      unavailable: Boolean(document.querySelector('[data-testid=desk-format-unavailable]')),
      hasTitle: Boolean(document.querySelector('#draft-title')),
      dataFormat: document.querySelector('.vk-editor')?.dataset.format ?? null,
      hasToolbar: Boolean(document.querySelector('#editor-markdown-toolbar')),
      hydrated: document.documentElement.dataset.hydrated ?? null,
    }))()`);
    check(!mdDiag.unavailable && !mdDiag.hasTitle && mdDiag.dataFormat === 'markdown' && mdDiag.hasToolbar,
      `Markdown opens in single Live Preview editor without #draft-title (${JSON.stringify(mdDiag)})`);
    const stored = await page.draft(created.id);
    check(stored.version === 2 && JSON.stringify(stored.document) === JSON.stringify(doc)
      && stored.title === 'Mưa đầu mùa', 'Markdown source untouched on open; title derives from the first H1');
  }

  {
    const doc = defaultDocument('Dòng một\r\nDòng hai');
    const created = await page.eval(`(async () => {
      const post = await fetch('/api/v1/drafts', { method: 'POST' });
      const draft = await post.json();
      const put = await fetch('/api/v1/drafts/' + draft.work_id, { method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ expected_version: draft.version, title: 'CR', document: ${JSON.stringify(doc)} }) });
      return { id: draft.work_id, status: put.status };
    })()`);
    check(created.status === 200, 'CRLF draft saved through the API');
    await page.goto(`${args.base}/ban-viet/${created.id}`);
    await page.waitFor(`document.querySelector('[data-testid=desk-format-unavailable]')`, 'CRLF stop state');
    check(!(await page.eval(`!!document.querySelector('.cm-content')`)), 'CRLF: no editor, so nothing can be re-saved');
    const stored = await page.draft(created.id);
    check(stored.version === 2 && JSON.stringify(stored.document) === JSON.stringify(doc), 'CRLF: source untouched');
  }
}

// #256/#257/#262 + nut-gui-luu: Markdown format choice, single-editor Live Preview,
// AST Mục lục (Cách 1), Send/Save state icon transitions and conversion dialog.
async function markdownLivePreviewAndFormatPicker(page) {
  console.log('editor live preview (#256), outline (#257), format picker (#262) & send/save button:');
  const cmView = `document.querySelector('.cm-content').cmTile.root.view`;
  await page.viewport(1440, 900);
  await page.goto(`${args.base}/ban-viet`);
  await page.waitFor(`document.querySelector('#create-draft') && document.querySelector('#create-draft-format') && document.documentElement.dataset.hydrated === 'true'`, 'desk index with format selector');
  check(await page.eval(`Boolean(document.querySelector('#create-draft-format').closest('[data-select-variant="inset-16"]'))`), '#create-draft-format uses SelectVariant::Inset16');
  await page.eval(`const sel = document.querySelector('#create-draft-format'); sel.value = 'markdown'; sel.dispatchEvent(new Event('change', { bubbles: true }))`);
  await page.click('#create-draft');
  await page.waitFor(EDITOR_READY, 'created markdown draft editor');
  const workId = await page.eval(`location.pathname.split('/').pop()`);
  check(await page.eval(`document.querySelector('.vk-editor')?.dataset.format === 'markdown' && Boolean(document.querySelector('#editor-markdown-toolbar')) && !document.querySelector('#draft-title')`), 'created Markdown draft opens directly in Markdown Live Preview');
  await page.eval(`window.__mdView = ${cmView}; true`);

  const sample = '# Tiếng mưa qua hiên\n\n## Hiên nhà\n\nMái ngói **gạch ướt** và *thưa dần* bên [Vink](https://vink.vn).\n\nĐoạn văn thường giữ nguyên.\n\n```\n# Không phải heading\n```\n';
  await page.typeInBody(sample);
  await page.waitStatus('unsaved');

  // Verify v1 Gửi button (nut-gui-luu): 18px Lucide send icon + 8px gap + label "Gửi".
  const v1Metrics = await page.eval(`(() => {
    const btn = document.querySelector('#save');
    const content = btn.querySelector('.vk-draft-action-content');
    const svg = content.querySelector('svg');
    const root = document.documentElement;
    const saved = root.dataset.text;
    try {
      return ['100', '200'].map(scale => {
        root.dataset.text = scale;
        const {width, height} = svg.getBoundingClientRect();
        return {scale, text: btn.textContent, icon: content.dataset.draftIcon, gap: getComputedStyle(content).columnGap, width, height};
      });
    } finally {
      if (saved === undefined) delete root.dataset.text;
      else root.dataset.text = saved;
    }
  })()`);
  check(v1Metrics.every(m => m.text === 'Gửi' && m.icon === 'send' && m.gap === '8px' && m.width === 18 && m.height === 18),
    `v1 primary action renders Gửi with 18px send icon and 8px gap at both text scales (${JSON.stringify(v1Metrics)})`);

  const gate = new SaveGate(page);
  try {
    await gate.enable();
    await page.click('#save');
    const firstPut = await gate.next();
    await page.waitStatus('saving');
    check(await page.eval(`document.querySelector('#save')?.textContent === 'Đang gửi…' && document.querySelector('#save .vk-draft-action-content')?.dataset.draftIcon === 'spin' && document.querySelector('[data-testid=save-status]')?.textContent === 'Đang gửi…'`),
      'v1 pending state switches to Đang gửi… with spinner icon');
    await gate.release(firstPut);
    await page.waitStatus('saved');
    check(await page.eval(`document.querySelector('.vk-editor')?.dataset.justSaved === 'true'`), 'clean acknowledgment activates transient Đã lưu state');
    const savedV2 = await page.draft(workId);
    check(savedV2.version === 2 && savedV2.title === 'Tiếng mưa qua hiên' && savedV2.document.format === 'markdown',
      'v1 Gửi commits Markdown draft and derives title from first H1');

    // v2 edit transitions to Lưu + save floppy icon -> Đang lưu…
    await page.typeInBody('Dòng thêm ở bản 2.\n');
    await page.waitStatus('unsaved');
    check(await page.eval(`document.querySelector('#save')?.textContent === 'Lưu' && document.querySelector('#save .vk-draft-action-content')?.dataset.draftIcon === 'save'`),
      'v2 dirty action shows Lưu with floppy save icon');
    await page.click('#save');
    const secondPut = await gate.next();
    await page.waitStatus('saving');
    check(await page.eval(`document.querySelector('#save')?.textContent === 'Đang lưu…' && document.querySelector('#save .vk-draft-action-content')?.dataset.draftIcon === 'spin'`),
      'v2 pending state switches to Đang lưu… with spinner icon');
    await gate.release(secondPut);
    await page.waitStatus('saved');
  } finally {
    await gate.disable();
  }

  // #256 Live Preview: reveal ## when caret enters H2 line, hide when moving back to paragraph.
  await page.eval(`(() => {
    const v = ${cmView};
    const pos = v.state.doc.line(3).from + 4;
    v.focus();
    v.dispatch({ selection: { anchor: pos } });
  })()`);
  await page.waitFor(`(() => {
    const lines = [...document.querySelectorAll('.cm-line')];
    const h2 = lines.find(l => l.textContent.includes('Hiên nhà'));
    return Boolean(h2?.classList.contains('cm-writing-h2')) && [...(h2?.querySelectorAll('.cm-markdown-marker') || [])].some(m => m.textContent === '##');
  })()`, 'caret in H2 reveals ## marker');

  await page.eval(`(() => {
    const v = ${cmView};
    const pos = v.state.doc.line(7).from + 2;
    v.focus();
    v.dispatch({ selection: { anchor: pos } });
  })()`);
  await page.waitFor(`(() => {
    const lines = [...document.querySelectorAll('.cm-line')];
    const h2 = lines.find(l => l.textContent.includes('Hiên nhà'));
    return Boolean(h2?.classList.contains('cm-writing-h2')) && !h2?.textContent.includes('##');
  })()`, 'leaving H2 hides ## marker while keeping heading formatting');

  // Toolbar formatting buttons (B, I, Link, H menu), Undo/Redo & IME composition on the same EditorView.
  const beforeFmt = await page.eval(`${cmView}.state.doc.toString()`);
  await page.eval(`(() => {
    const v = ${cmView};
    const line = v.state.doc.line(7);
    v.focus();
    v.dispatch({ selection: { anchor: line.from, head: line.from + 4 } });
  })()`);
  await page.click('#md-bold');
  await page.waitFor(`${cmView}.state.doc.toString().includes('**Đoạn**')`, '#md-bold wraps selection');
  await new Promise(r => setTimeout(r, 600));
  await page.click('#md-heading-tool');
  await page.waitFor(`document.querySelector('#md-heading-menu')`, 'heading menu opens');
  await page.click('#md-heading-menu button:has([data-heading-level="2"])');
  await page.waitFor(`${cmView}.state.doc.toString().includes('## **Đoạn**')`, 'heading menu applies H2');
  await page.key('z', page.mod);
  await page.key('z', page.mod);
  await page.waitFor(`${cmView}.state.doc.toString() === ${JSON.stringify(beforeFmt)}`, 'Undo restores exact source before toolbar edits');
  await page.eval(`(() => { const v = ${cmView}; const p = v.state.doc.line(7).to; v.focus(); v.dispatch({ selection: { anchor: p } }); })()`);
  await page.send('Input.imeSetComposition', { text: ' d', selectionStart: 2, selectionEnd: 2 });
  await page.send('Input.insertText', { text: ' dấu' });
  await new Promise(r => setTimeout(r, 80));
  await page.key('z', page.mod);
  await page.waitFor(`${cmView}.state.doc.toString() === ${JSON.stringify(beforeFmt)}`, 'IME commit undoes cleanly back to saved buffer');
  check(await page.eval(`window.__mdView === ${cmView} && document.querySelectorAll('.cm-editor').length === 1`),
    'Live Preview formatting and IME preserve the single EditorView');

  // Save clean before Mục lục checks.
  if ((await page.status()) !== 'saved') {
    await page.click('#save');
    await page.waitStatus('saved');
  }

  // #257 Mục lục Cách 1: AST headings, fenced code excluded, click jump without dirtying, responsive & Escape.
  await page.click('#outline-toggle');
  await page.waitFor(`document.querySelector('#editor-outline-panel')`, 'Mục lục panel opens');
  const outlineItems = await page.eval(`[...document.querySelectorAll('.vk-editor-outline__item')].map(el => {
    const btn = el.closest('button');
    const cs = btn ? getComputedStyle(btn) : null;
    return {
      text: el.textContent.trim(),
      cls: el.className,
      current: el.closest('[aria-current]')?.getAttribute('aria-current') ?? null,
      font: cs?.fontFamily ?? '',
      weight: cs?.fontWeight ?? '',
      bg: cs?.backgroundColor ?? '',
    };
  })`);
  check(outlineItems.length === 2
    && outlineItems[0].text === 'Tiếng mưa qua hiên'
    && outlineItems[0].cls.includes('--title')
    && outlineItems[0].font.includes('Literata')
    && outlineItems[1].text === 'Hiên nhà'
    && outlineItems[1].cls.includes('--h2')
    && outlineItems[1].weight === '600',
    `Mục lục Cách 1 renders Literata title + semibold H2 and skips code-fence # (${JSON.stringify(outlineItems)})`);

  await page.click('#editor-outline-panel button:has(.vk-editor-outline__item--h2)');
  await page.waitFor(`(() => {
    const active = document.querySelector('#editor-outline-panel [aria-current="location"] .vk-editor-outline__item');
    const h2Line = [...document.querySelectorAll('.cm-line')].find(l => l.textContent.includes('Hiên nhà'));
    return active?.textContent.trim() === 'Hiên nhà' && [...(h2Line?.querySelectorAll('.cm-markdown-marker') || [])].some(m => m.textContent === '##');
  })()`, 'outline item click jumps caret, reveals ##, and highlights active heading');
  check((await page.status()) === 'saved', 'jumping through outline never marks draft dirty');

  try {
    for (const scene of [320, 390, 834, 1024, 1440].flatMap(width =>
      ['100', '200'].flatMap(scale => ['light', 'dark'].map(theme => ({
        width, scale, theme, overlay: width < 1024 || scale === '200',
      }))))) {
      await page.viewport(scene.width, 900);
      await page.eval(`document.documentElement.dataset.text = '${scene.scale}'; document.documentElement.dataset.theme = '${scene.theme}'; new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))`);
      const layout = await page.eval(`(() => {
        const panel = document.querySelector('#editor-outline-panel');
        return {
          overflow: document.documentElement.scrollWidth > innerWidth,
          pos: panel ? getComputedStyle(panel).position : null,
        };
      })()`);
      check(!layout.overflow && (!scene.overlay || layout.pos === 'fixed'),
        `Markdown + Mục lục scene ${scene.width}/${scene.scale}/${scene.theme} has no horizontal overflow (${JSON.stringify(layout)})`);
      if ((scene.width === 320 && scene.scale === '200' && scene.theme === 'dark') ||
          (scene.width === 1440 && scene.scale === '100' && scene.theme === 'light')) {
        await page.waitFor(APPEARANCE_SETTLED, 'outline theme and highlight colors settled');
        await textContrast(page, scene.theme, 'outline');
        await page.capture('markdown-outline-' + [scene.width, scene.scale, scene.theme].join('-'), {
          scenario: 'markdown-outline', target: '#editor-outline-title', boundary: 'Chrome, seeded private draft',
        });
      }
    }
    await page.waitFor(`document.querySelector('#editor-outline-panel:modal')?.contains(document.activeElement)`,
      'the compact outline takes focus when it becomes a modal sheet');
    check(await page.eval(`Boolean(document.querySelector('#editor-outline-panel li > ul > li button'))`),
      'H2 is a nested list item of the first H1 for assistive technology');
    await page.key('Tab');
    check(await page.eval(`document.querySelector('#editor-outline-panel').contains(document.activeElement)`),
      'Tab stays inside the outline sheet');
    await page.key('Escape');
    await page.waitFor(`!document.querySelector('#editor-outline-panel') && document.activeElement?.id === 'outline-toggle'`,
      'Escape closes mobile Mục lục sheet and restores focus to #outline-toggle');
  } finally {
    await page.viewport(1440, 900);
    await page.eval(`delete document.documentElement.dataset.text; document.documentElement.dataset.theme = 'light'; scrollTo(0, 0)`);
  }

  const beforeLongTitle = await page.draft(workId);
  await page.eval(`(() => { const v = ${cmView}; window.__sourceBeforeLong = v.state.doc.toString(); v.dispatch({changes:{from:0,to:v.state.doc.length,insert:'# ' + 'ế'.repeat(201)}}); })()`);
  await page.eval(`(() => { const s = document.querySelector('#editor-format-select'); s.focus(); s.value = 'default'; s.dispatchEvent(new Event('change', {bubbles:true})); })()`);
  await page.waitFor(`document.querySelector('#format-convert-dialog[open] [role=alert]')?.textContent.includes('200') && document.querySelector('#format-convert-apply')?.getAttribute('aria-disabled') === 'true'`,
    '201-scalar first H1 blocks conversion with a visible repair instruction');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('#format-convert-dialog')?.open`, 'cancel overlong H1 conversion');
  check((await page.draft(workId)).version === beforeLongTitle.version &&
    await page.eval(`${cmView}.state.doc.toString() === '# ' + 'ế'.repeat(201)`), 'blocked conversion preserves unsaved source and stored version');
  await page.eval(`(() => { const v = ${cmView}; v.dispatch({changes:{from:0,to:v.state.doc.length,insert:window.__sourceBeforeLong}}); })()`);

  // #262 Format switcher: preview dialog Cancel resets select cleanly; Apply persists Default cleanly on same EditorView.
  await page.eval(`(() => { const s = document.querySelector('#editor-format-select'); s.focus(); s.value = 'default'; s.dispatchEvent(new Event('change', { bubbles: true })); })()`);
  await page.waitFor(`document.querySelector('#format-convert-dialog')?.open === true`, 'format conversion preview opens');
  check(await page.eval(`document.querySelector('#format-convert-dialog').textContent.includes('Tiếng mưa qua hiên')`),
    'format conversion preview shows converted title');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('#format-convert-dialog')?.open`, 'cancel closes format conversion dialog');
  check(await page.eval(`document.activeElement?.id === 'editor-format-select' && document.querySelector('#editor-format-select').value === 'markdown' && document.querySelector('.vk-editor')?.dataset.format === 'markdown' && window.__mdView === ${cmView}`),
    'canceling conversion resets select and keeps Markdown EditorView untouched');

  await page.eval(`(() => { const s = document.querySelector('#editor-format-select'); s.value = 'default'; s.dispatchEvent(new Event('change', { bubbles: true })); })()`);
  await page.waitFor(`document.querySelector('#format-convert-dialog')?.open === true`, 'reopen format conversion dialog');
  const ack = new SaveGate(page, 'Response');
  try {
    await ack.enable();
    await page.click('#format-convert-apply');
    await ack.fail(await ack.next());
  } finally {
    await ack.disable();
  }
  await page.waitFor(`!document.querySelector('#format-convert-dialog')?.open && document.querySelector('#draft-title')`, 'apply converts to Default and shows #draft-title');
  await page.waitStatus('saved');
  const converted = await page.draft(workId);
  check(converted.document.format === 'default'
    && converted.title === 'Tiếng mưa qua hiên'
    && await page.eval(`document.querySelector('#draft-title')?.value === 'Tiếng mưa qua hiên' && !document.querySelector('#editor-markdown-toolbar') && window.__mdView === ${cmView}`),
    'applying format conversion persists Default, populates #draft-title and preserves the single EditorView');
  const chooseMarkdown = () => page.eval(`(() => { const s = document.querySelector('#editor-format-select'); s.focus(); s.value = 'markdown'; s.dispatchEvent(new Event('change', {bubbles:true})); })()`);
  // Establish a newer server version after preview, without changing the open editor.
  await chooseMarkdown();
  await page.waitFor(`document.querySelector('#format-convert-dialog')?.open`, 'conversion pending author confirmation');
  const advanced = await page.eval(`fetch('/api/v1/drafts/${workId}', {method:'PUT', headers:{'Content-Type':'application/json'}, body:JSON.stringify({expected_version:${converted.version},title:'Tên khác',document:{format:'default',source:'Sửa ở thẻ khác.'}})}).then(r=>r.status)`);
  check(advanced === 200, 'another tab saved a newer version');
  await page.click('#format-convert-apply');
  await page.waitStatus('conflict');
  check(await page.eval(`document.querySelector('.vk-editor').dataset.format === 'default' && document.querySelector('#draft-title').value === 'Tiếng mưa qua hiên' && Boolean(document.querySelector('[data-testid="conversion-notice"]')) && window.__mdView === ${cmView}`),
    'conversion 409 keeps the original title, format and editor with explicit recovery');
  const afterConflict = await page.draft(workId);
  check(afterConflict.version === converted.version + 1 && afterConflict.document.source === 'Sửa ở thẻ khác.',
    'conflicting conversion preserves the concurrent server save without replay');

  // Leave the conflicting fixture through the same explicit discard path as an author.
  await page.click('.vk-editor-heading__back');
  await page.waitFor("document.querySelector('#leave-editor')?.open", 'leaving a conflict is guarded');
  await page.click('#leave-discard');
  await page.waitFor("document.activeElement?.id === 'discard-back'", 'discard confirmation ready');
  await page.click('#discard-confirm');
  await page.waitFor("location.pathname === '/ban-viet'", 'discard kept the newer server version');
  // A browser Back can dispose a clean editor even while a conversion's reply is in transit.
  const newDraft = async () => {
    await page.waitFor("document.querySelector('#create-draft') && document.querySelector('#create-draft-format')", 'draft creation ready');
    await page.click('#create-draft');
    await page.waitFor(EDITOR_READY, 'new Default editor ready');
    return page.eval("location.pathname.split('/').pop()");
  };
  const closingWork = await newDraft();
  await chooseMarkdown();
  await page.waitFor("document.querySelector('#format-convert-dialog')?.open", 'clean conversion open');
  const lateReply = new SaveGate(page, 'Response');
  try {
    await lateReply.enable();
    await page.click('#format-convert-apply');
    const held = await lateReply.next();
    await page.eval('window.__conversionPage = true; history.back()');
    await page.waitFor("location.pathname === '/ban-viet' && !document.querySelector('.cm-editor')", 'conversion owner disposed');
    const nextWork = await newDraft();
    await page.typeInBody('Văn bản đang viết.');
    await page.waitStatus('unsaved');
    await lateReply.release(held);
    await page.eval('new Promise(r => setTimeout(r, 250))');
    check(await page.eval("window.__conversionPage && document.querySelector('.vk-editor').dataset.format === 'default'") &&
      await page.status() === 'unsaved' && await page.body() === 'Văn bản đang viết.' && nextWork !== closingWork,
      'a late conversion ACK leaves the next draft, format, buffer and save ticket intact in the same SPA');
    check((await page.draft(closingWork)).document.format === 'markdown' && (await page.draft(nextWork)).version === 1,
      'only the original draft committed a new version');
  } finally {
    await lateReply.disable();
  }
  await page.click('#save');
  await page.waitStatus('saved');

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
  check((await page.eval(`document.querySelector('#leave-editor-context').textContent`)).includes('Thẻ đã chọn'), 'the dialog names the unsaved tags');
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

  // A refusal belongs to the submitted chips. Respond after the author removes the
  // refused chip; the corrected selection must still be saveable with its original CAS.
  const pendingTags = new SaveGate(page);
  await pendingTags.enable();
  await page.send('Input.insertText', { text: 'Gia đình' });
  await page.waitFor(`document.getElementById('work-tags-status').textContent.includes('“Gia đình”')`, 'extra theme for a delayed refusal');
  await page.key('Enter');
  await page.waitFor(`document.querySelectorAll('#work-tags-themes .vk-tag-chip').length === 3`, 'third theme selected');
  const familyTag = (await page.eval(`fetch('/api/v1/tags?facet=theme').then((r) => r.json())`)).tags.find((tag) => tag.label === 'Gia đình');
  await page.click('#save');
  const refusedTagPut = await pendingTags.next();
  await page.click('#work-tags-theme-2-remove');
  await page.waitFor(`document.querySelector('#editor-save-details').textContent.includes('lựa chọn mới nhất vẫn cần được lưu tiếp')`, 'toolbar records the selection made while saving');
  const refusal = JSON.parse(readFileSync(new URL('../../../services/vink-app/contracts/http/v1/fixtures/error.tag_retired.json', import.meta.url), 'utf8'));
  refusal.error.tag_id = familyTag.tag_id;
  delete refusal.error.successor_id;
  await page.send('Fetch.fulfillRequest', {
    requestId: refusedTagPut, responseCode: 409,
    responseHeaders: [{name: 'Content-Type', value: 'application/json'}],
    body: Buffer.from(JSON.stringify(refusal)).toString('base64'),
  });
  await waitTagStatus(page, 'unsaved', 'a late refusal cannot block the corrected selection');
  await pendingTags.disable();
  check(puts === 1 && await page.eval(`!document.querySelector('[data-testid=tag-refused]') && document.querySelector('#save')?.textContent === 'Lưu'`), 'late refusal for a removed tag leaves one save action and no stale warning');
  puts = 0;

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
  check((await page.eval(`document.querySelector('[data-testid=save-status]').textContent`)) === 'Đã lưu chữ; chưa lưu thẻ', 'toolbar truthfully distinguishes saved text with unsaved tags');
  // Removing a chip restores focus in the next frame. Finish that interaction before
  // moving to the toolbar, then hold the request to observe focus before the 409.
  await page.eval(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))`);
  await pendingTags.enable();
  await page.eval(`document.querySelector('#save').focus()`);
  await page.key('Enter');
  const conflictPut = await pendingTags.next();
  await waitTagStatus(page, 'saving', 'tag conflict request in flight');
  check(await page.eval(`document.activeElement?.id === 'save'`), 'pending tag save retains the focused toolbar button');
  await pendingTags.release(conflictPut);
  await waitTagStatus(page, 'conflict', 'tag conflict');
  await pendingTags.disable();
  await page.waitFor(`document.activeElement?.id === 'save-recovery-jump'`, 'tag conflict hands keyboard focus back to the save slot');
  check((await page.eval(`document.querySelector('[data-testid=save-status]').textContent`)) === 'Đã lưu chữ; thẻ đã đổi ở nơi khác.' && await page.eval(`document.querySelector('#save') === null && document.querySelector('#save-recovery-jump')?.textContent.includes('Xử lý xung đột thẻ')`), 'tag conflict routes to tag recovery instead of blind save retry');
  check((await page.eval(`document.querySelector('[data-testid=tag-conflict]').textContent`)).includes('Đang lưu: Mùa'), 'the conflict shows the stored tags by name');
  stored = await draftTagsOf(page, workId);
  check(stored.version === 3 && stored.themes.map((t) => t.label).join() === 'Mùa', 'the stale tag save wrote nothing');
  check((await chipLabels(page)).join() === 'Đô thị', 'the conflict keeps the author\'s selection on the page');
  const tagConflictScene = { width: 320, height: 800, scale: 200, theme: 'dark' };
  const beforeScene = await page.eval(`({width:innerWidth,height:innerHeight,font:document.documentElement.style.fontSize,theme:document.documentElement.dataset.theme ?? null})`);
  await setEditorScene(page, tagConflictScene);
  check(await page.eval(`!document.querySelector('#export-tags') && document.querySelector('#tags-keep-mine')?.checkVisibility() && !document.querySelector('#tags-keep-mine').closest('details,dialog')`),
    'tag-only conflict removes export and keeps direct overwrite outside closed tools');
  await page.click('.vk-editor-heading__back');
  await page.waitFor(`document.querySelector('#leave-editor').open`, 'leave with tag conflict');
  await leaveActionScene(page, tagConflictScene, 'tag-conflict',
    ['leave-stay', 'leave-save', 'leave-discard'], ['leave-save']);
  check(await page.eval(`document.querySelector('#leave-editor-context').textContent.includes('Thẻ đã đổi ở nơi khác')`), 'leave prompt explains the tag conflict');
  await page.key('Tab');
  await page.key('Enter');
  check(await page.eval(`document.querySelector('#leave-editor').open`) && (await draftTagsOf(page, workId)).version === 3,
    'disabled save-and-leave preserves tag conflict and stored version');
  await page.key('Escape');
  await page.waitFor(`!document.querySelector('#leave-editor').open && document.activeElement.id === 'work-tags-query'`, 'Escape returns from tag conflict to its writing context');
  await page.viewport(beforeScene.width, beforeScene.height);
  await page.eval(`document.documentElement.style.fontSize=${JSON.stringify(beforeScene.font)};
    ${beforeScene.theme ? 'document.documentElement.dataset.theme=' + JSON.stringify(beforeScene.theme) : 'delete document.documentElement.dataset.theme'}`);
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
  check((await page.eval(`document.querySelector('[data-testid=save-status]').textContent`)) === 'Đã lưu chữ; chưa xác nhận lưu thẻ.', 'the toolbar says the text is saved and tag save is unconfirmed');
  stored = await draftTagsOf(page, workId);
  check(stored.version === 5, 'the dropped answer belonged to a committed save');
  check((await page.eval(`location.pathname`)) === editorPath, 'a lost tag acknowledgement keeps the author in the editor');
  await page.click('#leave-stay');
  check(await page.eval(`!document.querySelector('#export-tags') && document.querySelector('[data-testid=tag-failed] #tags-retry')?.checkVisibility() && !document.querySelector('#tags-retry').closest('details,dialog')`),
    'uncertain tag save removes export and keeps direct retry outside tools');
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
  check((await page.eval(`document.querySelector('#leave-editor-context').textContent`)).includes('Chữ và thẻ'), 'the dialog names both parts');
  await page.click('#leave-save');
  await page.waitFor(`location.pathname === '/ban-viet'`, 'left after both saves');
  stored = await draftTagsOf(page, workId);
  const text = await page.draft(workId);
  check(stored.version === 6 && stored.themes.map((t) => t.label).join() === 'Đô thị' && text.version === textVersion + 1 && paragraphs(text.document).at(-1).lines.at(-1).endsWith(' Cùng thẻ.'), 'left only after the text and the tags were both confirmed');
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
  await page.eval(`window.discussionComposerNode = document.querySelector('#root-composer')`);
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
      const recovery = '.vk-discussion-composer:has(#root-composer) .vk-discussion-delivery';
      const palette = await page.eval(`(() => {
        const card = document.querySelector(${JSON.stringify(recovery)});
        const probe = document.createElement('span');
        probe.style.cssText = 'color:var(--vk-on-info-container);background-color:var(--vk-info-container)';
        card.append(probe);
        const result = {
          tone: card.dataset.tone,
          text: getComputedStyle(card.querySelector('p')).color,
          background: getComputedStyle(card).backgroundColor,
          body: getComputedStyle(document.body).color,
          tokenText: getComputedStyle(probe).color,
          tokenBackground: getComputedStyle(probe).backgroundColor,
        };
        probe.remove();
        return result;
      })()`);
      console.log('uncertain recovery palette', JSON.stringify(palette));
      check(palette.tone === 'unknown' && palette.text === palette.tokenText && palette.background === palette.tokenBackground,
        'uncertain receipt uses the resolved on-info-container foreground and info-container background');
      await textContrast(page, 'dark', `discussion-uncertain-${width}-${scale}`, [`${recovery} [role="status"]`, `${recovery} > p`]);
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
    await page.waitFor(`document.querySelector('#own-comments [data-own-comment="${commentId}"]')`, 'the pending item appears in Bạn đã gửi');
    check(await page.eval(`(() => {
      const region = document.getElementById('own-comments');
      const head = region.querySelector('.vk-discussion-own-head').getBoundingClientRect();
      const card = region.querySelector('[data-own-comment]').getBoundingClientRect();
      return Math.abs(card.top - head.bottom - parseFloat(getComputedStyle(region).rowGap)) <= 1;
    })()`), 'empty live regions leave only one grid gap above the first own item');
    const publicHtml = await (await fetch(`${args.base}/tac-pham/${SAMPLE_WORK}`)).text();
    check(!publicHtml.includes('D2T-03 [Mẫu] Đọc khổ thơ') && !publicHtml.includes('own_pending') && !publicHtml.includes('own_rejected'), 'public SSR and hydration payload exclude fixture viewer-private comments');
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
  }
  await page.click('#discussion-mode-toggle');
  await page.waitFor(`document.querySelector('#discussion-mode-toggle')?.textContent==='Mở bình luận' && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'author closes comments with generation CAS');
  await page.send('Browser.grantPermissions', { permissions: ['clipboardReadWrite', 'clipboardSanitizedWrite'] });
  check(await page.eval(`window.discussionComposerNode === document.querySelector('#root-composer') && document.querySelector('#root-composer').value===${JSON.stringify(text + newer)} && document.querySelector('#root-composer').readOnly === true && !document.querySelector('#root-composer-send') && document.querySelector('.vk-discussion-closed-banner')?.textContent.includes('Bạn đã tắt bình luận cho bài viết này') && !!document.querySelector('[data-own-comment="${commentId}"]')`), 'closing keeps readonly draft, author notice, readable own projection and removes Gửi button');
  await page.eval(`[...document.querySelectorAll('.vk-discussion-closed-actions button')].find(b=>b.textContent.includes('Sao chép nháp'))?.click()`);
  await page.waitFor(`document.querySelector('.vk-discussion-closed-actions [data-copied="true"]')?.textContent.includes('Đã sao chép nháp')`, 'closed draft copy confirmation');
  check(await page.eval('navigator.clipboard.readText()') === text + newer, 'closed draft copy writes exact draft text to real clipboard');
  // A missing Clipboard API must leave exactly the author's native draft selected.
  await page.eval(`Object.defineProperty(navigator, 'clipboard', { configurable: true, value: undefined })`);
  try {
    await page.eval(`[...document.querySelectorAll('.vk-discussion-closed-actions button')].find(b=>b.textContent.includes('Sao chép nháp')).click()`);
    await page.waitFor(`(() => {
      const field = document.querySelector('#root-composer');
      const status = document.querySelector('.vk-discussion-closed-actions [role="status"]');
      return status?.textContent.includes('Không thể tự động sao chép') && !status.hasAttribute('data-copied')
        && document.activeElement === field && field.selectionStart === 0 && field.selectionEnd === field.value.length;
    })()`, 'missing clipboard exposes manual copy and selects the entire unchanged draft');
  } finally {
    await page.eval(`delete navigator.clipboard`);
  }

  const closed = await (await fetch(endpoint)).json();
  check(!closed.settings.comments_open && closed.settings.generation > 1, 'closed mode is a durable provider fact');
  await captureDiscussionMatrix(page, 'local-postgres-closed', '#feedback-heading', 'real-local-postgresql-with-fixture-identity');
  await page.click('#discussion-mode-toggle');
  await page.waitFor(`document.querySelector('#discussion-mode-toggle')?.textContent==='Tắt bình luận' && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true' && document.querySelector('#remove-comment-${commentId}')`, 'author reopens the same thread');
  check(await page.eval(`window.discussionComposerNode === document.querySelector('#root-composer') && !window.discussionComposerNode.readOnly`), 'reopening keeps the original editable textarea');
  // VDR-DISCUSSION-R4@r1 §2: asking opens an in-place confirmation; only “Gỡ” sends DELETE.
  await page.click(`#remove-comment-${commentId}`);
  await page.waitFor(`document.activeElement?.id==='cancel-remove-comment-${commentId}' && document.querySelector('#confirm-remove-comment-${commentId}')`, 'removal asks in place and focuses “Giữ lại”');
  check(await page.eval(`!!document.querySelector('[data-own-comment="${commentId}"] .vk-discussion-own-body')?.textContent && !document.querySelector('#remove-comment-${commentId}')`), 'nothing is removed before confirmation');
  await page.click(`#cancel-remove-comment-${commentId}`);
  await page.waitFor(`document.activeElement?.id==='remove-comment-${commentId}' && !document.querySelector('#confirm-remove-comment-${commentId}')`, '“Giữ lại” closes the confirmation and returns focus');
  await page.click(`#remove-comment-${commentId}`);
  await page.waitFor(`document.querySelector('#confirm-remove-comment-${commentId}')`, 'confirmation reopened');
  await page.click(`#confirm-remove-comment-${commentId}`);
  await page.waitFor(`!document.querySelector('[data-own-comment="${commentId}"] .vk-discussion-own-body')?.textContent && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'own removal refreshes the authorized projection');

  console.log('D2T-03: labelled provider fixture projection for flat contexts and responsive states:');
  const fixture = JSON.parse(readFileSync(new URL('../../../services/vink-app/contracts/http/v1/fixtures/discussion.public.get.json', import.meta.url), 'utf8'));
  fixture.work_id = SAMPLE_WORK;
  const rootId = fixture.roots[0].root.comment.comment_id;
  const replyId = fixture.roots[0].preview[0].comment_id;
  let currentFixture = structuredClone(fixture);
  let replyRequest;
  let heldInitialPublic = null;
  let holdInitialRead = true;
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
    if (holdInitialRead && cursor === null) {
      holdInitialRead = false;
      heldInitialPublic = message.params.requestId;
      return;
    }
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
    // Client navigation exercises the 300ms static skeleton and initial resource error cleanly.
    await page.goto(`${args.base}/`);
    await page.waitFor(`document.documentElement.dataset.hydrated==='true' && document.querySelector('a[href="/tac-pham/${SAMPLE_WORK}"]')`, 'reader link for initial-read failure');
    await page.click(`a[href="/tac-pham/${SAMPLE_WORK}"]`);
    const waitHeldDeadline = Date.now() + 5000;
    while (!heldInitialPublic && Date.now() < waitHeldDeadline) await sleep(20);
    check(!!heldInitialPublic, 'the first public GET is held before measuring the skeleton');
    await sleep(120);
    check(await page.eval(`!document.querySelector('.vk-discussion-skeleton-list')`), '300ms gate hides skeleton before threshold');
    await page.waitFor(`!!document.querySelector('.vk-discussion-skeleton-list')`, 'static skeleton appears after 300ms delay');
    check(await page.eval(`!document.querySelector('.vk-discussion-comment') && getComputedStyle(document.querySelector('.vk-discussion-skeleton-list .vk-ui-skeleton')).animationName==='none'`), 'skeleton is static and never creates comment items');
    await page.send('Fetch.fulfillRequest', { requestId: heldInitialPublic, responseCode: 503,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify({ error: { code: 'unavailable', message: 'fixture outage' } })).toString('base64'),
    });
    await page.waitFor(`document.querySelector('#discussion-load-retry')`, 'initial discussion read failed');
    check(await page.eval(`document.querySelector('.vk-discussion-state-card[data-tone="adverse"]')?.textContent.includes('Chưa tải được nhận xét') && !document.querySelector('#feedback-region').textContent.includes('Chưa có nhận xét nào')`), 'initial error is distinct from empty state');
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
    check(await page.eval(`document.querySelector('#comment-${replyId}')?.dataset.replyTarget==='true' && document.querySelector('#comment-${replyId} .vk-discussion-target-badge')?.textContent.includes('Đang là đích trả lời')`), 'targeted reply highlights inside the branch');
    check(await page.eval(`document.querySelectorAll('.vk-discussion-replies .vk-discussion-replies').length===0 && document.querySelectorAll('#reply-composer-${rootId}').length===1`), 'reply-to-reply mounts one composer in its root and no third indentation level');
    await page.eval(`document.querySelector('.vk-discussion-target-chip button')?.click()`);
    await page.waitFor(`document.activeElement?.id==='reply-composer-${rootId}' && document.querySelector('.vk-discussion-target-chip')?.dataset.target==='root'`, 'clearing target chip returns to root target while keeping draft and focus');
    check(await page.eval(`document.querySelector('#reply-composer-${rootId}')?.value.includes('Giữ nguyên')`), 'clearing target preserves draft');
    await page.click(`#reply-to-${replyId}`);
    await page.waitFor(`document.activeElement?.id==='reply-composer-${rootId}'`, 're-select reply target');
    await captureDiscussionMatrix(page, 'contract-fixture-flat', `#comment-${rootId}`, 'intercepted-provider-contract-fixture-no-persistence');
    await page.click(`#reply-composer-${rootId}-send`);
    const deadline = Date.now() + 10_000;
    while (!replyRequest) {
      if (Date.now() > deadline) throw new Error('timed out waiting for held fixture reply intent');
      await sleep(50);
    }
    check(await page.eval(`(() => { const b = document.querySelector('#reply-composer-${rootId}-send'); const f = document.querySelector('#reply-composer-${rootId}'); return b?.getAttribute('aria-busy')==='true' && getComputedStyle(b.querySelector(':scope > .vk-ui-icon')).display==='none' && !!b.querySelector('.vk-ui-button__status svg') && f?.readOnly===false && b.querySelector('.vk-ui-button__status').getBoundingClientRect().right <= b.querySelector('.vk-ui-button__label').getBoundingClientRect().left; })()`), 'R4 Sending keeps the field editable and places the progress indicator before its label');
    await page.eval(`window.discussionReplyComposerNode = document.querySelector('#reply-composer-${rootId}');
      window.discussionReplyComposerNode.setSelectionRange(1, 4, 'backward')`);
    await page.eval(`[...document.querySelectorAll('.vk-discussion-reply-composer button')].find(b=>b.textContent.trim()==='Thu ô · Giữ chữ').click()`);
    await page.waitFor(`document.querySelector('#reply-composer-${rootId}') === window.discussionReplyComposerNode && window.discussionReplyComposerNode.getClientRects().length === 0 && document.activeElement?.id==='reply-to-${replyId}' && document.querySelector('.vk-discussion-reply-kept')?.textContent.includes('Bản trả lời được giữ lại')`, 'reply composer collapsed cleanly with R4 kept pill');
    await page.click(`#reopen-reply-${rootId}`);
    await page.waitFor(`document.activeElement === window.discussionReplyComposerNode &&
      document.querySelector('#reply-composer-${rootId}') === window.discussionReplyComposerNode &&
      window.discussionReplyComposerNode.selectionStart === 1 && window.discussionReplyComposerNode.selectionEnd === 4 &&
      window.discussionReplyComposerNode.selectionDirection === 'backward' &&
      document.querySelector('#reply-composer-${rootId}-send')?.dataset.state === 'pending'`,
      'reopening restores focus and preserves the native textarea, selection and pending transport');
    await page.send('Fetch.fulfillRequest', { requestId: replyRequest, responseCode: 201,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
      body: Buffer.from(JSON.stringify({ comment_id: '0192f3a1-0000-7000-8000-000000000099', status: 'pending', replayed: false })).toString('base64'),
    });
    await page.waitFor(`document.querySelector('#reply-composer-${rootId}')?.value==='' && document.querySelector('.vk-discussion-reply-composer [data-testid=discussion-receipt]')?.textContent.includes('Chờ duyệt')`, 'unchanged reply clears only when its pending receipt arrives after reopening');
    await page.typeAtEnd(`#reply-composer-${rootId}`, 'Giữ nguyên bản trả lời mới 👩‍💻');
    check(true, 'intercepted fixture only: collapse/reopen does not cancel the feature transport or strand Sending');
    currentFixture = structuredClone(fixture);
    currentFixture.roots[0].preview = currentFixture.roots[0].preview.filter(comment => comment.comment_id !== replyId);
    currentFixture.roots[0].preview[0].reply_to = { kind: 'unavailable' };
    await page.click('#discussion-refresh');
    await page.waitFor(`!document.querySelector('#comment-${replyId}') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'context revocation fixture refresh');
    check(await page.eval(`document.querySelector('#reply-composer-${rootId}')?.value.includes('Giữ nguyên') && document.querySelector('#reply-composer-${rootId}-send')?.getAttribute('aria-disabled')==='true' && document.querySelector('.vk-discussion-target-lost')?.textContent.includes('Bình luận được trả lời không còn hiển thị') && document.querySelectorAll('.vk-discussion-replies .vk-discussion-comment').length===3`), 'revoked context shows warning card with copy/reselect without claiming deletion');
    await page.eval(`[...document.querySelectorAll('.vk-discussion-target-lost button')].find(b=>b.textContent.includes('Sao chép nháp'))?.click()`);
    await page.waitFor(`document.querySelector('.vk-discussion-target-lost [data-copied="true"]')?.textContent.includes('Đã sao chép nháp')`, 'revoked context copy feedback');
    await captureDiscussionMatrix(page, 'revoked-target', '.vk-discussion-target-lost', 'intercepted-provider-contract-fixture-no-persistence');
    const revokedDraft = await page.eval(`document.querySelector('#reply-composer-${rootId}').value`);
    check(await page.eval('navigator.clipboard.readText()') === revokedDraft, 'revoked context copy writes exact draft to real clipboard');
    await page.eval(`[...document.querySelectorAll('.vk-discussion-target-lost button')].find(b=>b.textContent.includes('Chọn bình luận khác'))?.click()`);
    await page.waitFor(`document.activeElement?.id==='reply-to-${rootId}'`, 'Chọn bình luận khác moves focus cleanly inside branch');
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
    await page.click('#discussion-refresh');
    await page.waitFor(`!document.querySelector('#comment-${nextRootId}') && document.querySelector('[data-testid=discussion-chain-restarted]') && document.querySelector('#root-pagination button')`, 'head refresh discards the previous tail and restarts continuation');
    check(await page.eval(`document.querySelector('#reply-composer-${rootId}')?.value.includes('Giữ nguyên')`), 'chain restart keeps the unsent reply draft');
    await page.eval(`document.querySelector('[data-testid=discussion-chain-restarted]').scrollIntoView({ block: 'center' })`);
    await page.capture('discussion-chain-restarted', { scenario: '#202-head-refresh', boundary: 'intercepted-provider-contract-fixture-no-persistence' });
    await page.viewport(390);
    await page.eval(`document.documentElement.style.fontSize='200%'; document.querySelector('[data-testid=discussion-chain-restarted]').scrollIntoView({block:'center'})`);
    check(await page.eval(`document.documentElement.scrollWidth <= innerWidth + 1`), 'chain restart notice reflows at 390px / text 200%');
    await page.capture('discussion-chain-restarted-390-200', { scenario: '#202-head-refresh', viewport: {width:390}, textScale:200, boundary: 'intercepted-provider-contract-fixture-no-persistence' });
    currentFixture = { ...fixture, roots: [], next_cursor: null };
    await page.click('#discussion-refresh');
    await page.waitFor(`document.querySelector('.vk-discussion-empty') && document.querySelector('#discussion-refresh')?.getAttribute('aria-disabled')!=='true'`, 'authoritative empty discussion state');
    check(await page.eval(`document.querySelector('.vk-discussion-empty')?.textContent.includes('Chưa có nhận xét nào được hiển thị.') && !document.querySelector('#discussion-load-retry') && !document.querySelector('#feedback-region')?.textContent.includes('Đã hết danh sách nhận xét')`), 'R4 empty state renders empty card cleanly without error button or list end');
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
    const segment = root.querySelector('.vk-discussion-actions .vk-discussion-action-segment[data-root="true"]');
    const rootEndorse = segment?.querySelector('.vk-endorsement .vk-ui-toggle__control');
    const rootReply = segment?.querySelector('[id^="reply-to-"]');
    const nameEl = root.querySelector('.vk-discussion-byline .vk-ui-byline__name');
    const rect = e => { const r=e.getBoundingClientRect(); return {left:r.left, right:r.right}; };
    const controls = [...pattern.querySelectorAll('.vk-ui-toggle__control')];
    const luminance = color => {
      const rgb=color.match(/[0-9.]+/g).slice(0,3).map(Number).map(v=>v/255).map(v=>v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4);
      return rgb[0]*0.2126+rgb[1]*0.7152+rgb[2]*0.0722;
    };
    const ratio=(a,b)=>{const x=luminance(a),y=luminance(b);return (Math.max(x,y)+0.05)/(Math.min(x,y)+0.05);};
    const surface=getComputedStyle(group).backgroundColor;
    const fieldBg=getComputedStyle(composer.querySelector('.vk-ui-field__control')).backgroundColor;
    const ratios=[
      ratio(getComputedStyle(group.querySelector('.vk-discussion-body')).color,surface),
      ratio(getComputedStyle(group.querySelector('time')).color,surface),
      ratio(getComputedStyle(composer.querySelector('label')).color,fieldBg),
      ratio(getComputedStyle(rootEndorse).color, getComputedStyle(rootEndorse).backgroundColor),
      ratio(getComputedStyle(rootReply).color, getComputedStyle(rootReply).backgroundColor),
    ];
    const reasons = [...pattern.querySelectorAll('.vk-discussion-shared-reason')].filter(e=>!e.hidden);
    const segmentOrderOk = rootEndorse && rootReply && !!(rootEndorse.compareDocumentPosition(rootReply) & Node.DOCUMENT_POSITION_FOLLOWING);
    const segmentCornersOk = rootEndorse && rootReply &&
      getComputedStyle(rootEndorse).borderTopRightRadius === '6px' &&
      getComputedStyle(rootReply).borderTopLeftRadius === '6px' &&
      getComputedStyle(segment).columnGap === '2px';
    const headingStatic = document.querySelector('.vk-discussion-head')?.textContent.includes('Nhận xét') &&
      document.querySelector('.vk-discussion-order')?.textContent.trim() === 'Cũ đến mới' &&
      !document.querySelector('#feedback-region select');
    return {root:rect(root), group:rect(group), composer:rect(composer), body:rect(body),
      segment:rect(segment), avatarLeft:rect(root.querySelector('.vk-discussion-byline .vk-ui-avatar')).left,
      nameLeft:rect(nameEl).left,
      fieldPadLeft:rect(composer.querySelector('.vk-ui-field__control')).left - rect(composer).left,
      fieldPadRight:rect(composer).right - rect(composer.querySelector('.vk-ui-field__control')).right,
      gap:parseFloat(getComputedStyle(group.querySelector('ul')).rowGap),
      contrast:Math.min(...ratios),
      segmentOrderOk, segmentCornersOk, headingStatic,
      reasons:reasons.length, targetReasons:pattern.querySelectorAll('.vk-ui-action__note').length,
      descriptions:controls.every(b=>(b.getAttribute('aria-describedby')||'').split(' ').filter(Boolean).every(id=>!!document.getElementById(id))),
      shared:controls.length>=2 && controls.every(b=>(b.getAttribute('aria-describedby')||'').includes(reasons[0]?.id)),
      field:composer.querySelectorAll('textarea').length===1 && !!composer.querySelector('label[for]'),
      radius:getComputedStyle(composer).borderTopLeftRadius,
      rootSeg48:[rootEndorse, rootReply].every(b=>{const r=b.getBoundingClientRect();return r.width>=48 && r.height>=48;}),
      buttons:[...thread.querySelectorAll('button')].every(b=>{const r=b.getBoundingClientRect();return r.width>=44 && r.height>=44 && r.left>=0 && r.right<=innerWidth+1;})};
  })()`);
  const close = (a,b)=>Math.abs(a-b)<=1.5;
  const isDesktop = width >= 1024;
  const expectedLeft = isDesktop ? geometry.nameLeft : geometry.avatarLeft;
  check(
    close(geometry.body.left, expectedLeft)
      && close(geometry.segment.left, expectedLeft)
      && close(geometry.group.left, expectedLeft)
      && close(geometry.group.right, geometry.root.right)
      && close(geometry.composer.left, expectedLeft)
      && close(geometry.composer.right, geometry.root.right),
    `${scenario}: R4 alignment (${isDesktop ? 'desktop name-track indent' : 'mobile full width'}) (${JSON.stringify(geometry)})`
  );
  check(close(geometry.fieldPadLeft, geometry.fieldPadRight) && geometry.gap===16, `${scenario}: symmetrical composer inset and 16px reply spacing`);
  check(geometry.headingStatic && geometry.segmentOrderOk && geometry.segmentCornersOk, `${scenario}: R4 heading “Cũ đến mới” static and root segmented Ưng|Trả lời 6px inner corners`);
  check(geometry.reasons===1 && geometry.targetReasons===0 && geometry.descriptions && geometry.shared, `${scenario}: one uniform branch reason describes every separate target`);
  check(geometry.field && geometry.radius==='28px' && geometry.rootSeg48 && geometry.buttons, `${scenario}: one native field, 28px tonal surface, 48px root segment and >=44px targets`);
  check(geometry.contrast>=4.5, `${scenario}: resolved body/metadata/label/button contrast >=4.5 (${geometry.contrast.toFixed(2)})`);
  if (width === 390 || width === 360 || width === 1440 || (width === 320 && scale === 200)) {
    const scroll = await page.eval('({x:scrollX,y:scrollY})');
    await page.eval(`document.querySelector('.vk-discussion-reply-composer').scrollIntoView({block:'start'})`);
    const currentTheme = await page.eval('document.documentElement.dataset.theme');
    await page.capture(`discussion-composer-${width}-${scale}-${currentTheme}`, {
      scenario: 'D2T-03-composer',
      requestedScene: { width, height: 900, scale, theme: currentTheme },
      textScaleLane: 'dom-override-geometry-only',
      requiredTargets: ['.vk-discussion-reply-composer', '.vk-feedback-composer'],
      boundary: 'intercepted-provider-contract-fixture-no-persistence',
    });
    await page.eval(`window.scrollTo({left:${scroll.x},top:${scroll.y},behavior:'instant'})`);
  }
}

async function checkDiscussionThemeFrames(page, theme) {
  const minimum = await page.eval(`(async () => {
    const field = document.querySelector('.vk-discussion-reply-composer .vk-ui-field__control');
    const segment = document.querySelector('.vk-discussion-action-segment[data-root="true"]');
    const toggle = segment.querySelector('.vk-ui-toggle__control');
    const reply = segment.querySelector('[id^="reply-to-"]');
    const targets = [[field.querySelector('label'), field], [toggle, toggle], [reply, reply]];
    const luminance = color => {
      const rgb = color.match(/[0-9.]+/g).slice(0,3).map(Number).map(v => v/255)
        .map(v => v <= 0.04045 ? v/12.92 : ((v+0.055)/1.055)**2.4);
      return rgb[0]*0.2126 + rgb[1]*0.7152 + rgb[2]*0.0722;
    };
    const minima = targets.map(() => Infinity);
    document.documentElement.dataset.theme = ${JSON.stringify(theme)};
    // Cover the first style update and the full 160 ms token in normal motion.
    const begin = performance.now();
    do {
      targets.forEach(([fg, bg], index) => {
        const a = luminance(getComputedStyle(fg).color);
        const b = luminance(getComputedStyle(bg).backgroundColor);
        minima[index] = Math.min(minima[index], (Math.max(a,b)+0.05)/(Math.min(a,b)+0.05));
      });
      await new Promise(requestAnimationFrame);
    } while (performance.now() - begin < 200);
    return minima;
  })()`);
  check(minimum.every(ratio => ratio >= 4.5),
    `discussion ${theme}: field label, Ưng and Trả lời keep 4.5:1 through the theme change (${minimum.map(ratio => ratio.toFixed(2)).join(', ')})`);
}

async function captureDiscussionMatrix(page, name, anchor, boundary) {
  for (const theme of ['light', 'dark']) {
    if (name === 'contract-fixture-flat') await checkDiscussionThemeFrames(page, theme);
    else await page.eval(`document.documentElement.dataset.theme=${JSON.stringify(theme)}`);
    for (const width of [320, 360, 390, 834, 1024, 1280, 1440]) {
      await page.viewport(width, 900);
      await page.eval(`document.documentElement.style.fontSize=''; document.querySelector(${JSON.stringify(anchor)}).scrollIntoView({block:'start'})`);
      const ov100 = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({
        containers: ['.vk-discussion-root', '.vk-discussion-thread'],
      })));
      check(ov100.ok && await page.eval(`document.documentElement.scrollWidth<=innerWidth+1 && document.querySelectorAll('.vk-discussion-replies .vk-discussion-replies').length===0`),
        `${name}/${theme}/${width}: reflows without unintended overflow or a third reply tier (${ov100.reasons.join('; ')})`);
      await page.capture(`discussion-${name}-${width}-${theme}`, {
        scenario: 'D2T-03-responsive',
        state: `text100-${theme}`,
        boundary,
        build: BUILD_SHA,
        textScale: 100,
        requestedScene: { width, height: 900, scale: 100, theme },
        textScaleLane: 'dom-override-geometry-only',
        requiredTargets: [anchor],
        overflow: ov100,
      });
      if (name === 'contract-fixture-flat') await discussionR31Geometry(page, width, 100, `${name}/${theme}/${width}`);
      if (width <= 390 || width >= 1280) {
        await page.eval(`document.documentElement.style.fontSize='200%'; document.querySelector(${JSON.stringify(anchor)}).scrollIntoView({block:'start'})`);
        const ov200 = evaluateOverflowReport(await page.eval(buildOverflowInspectionScript({
          containers: ['.vk-discussion-root', '.vk-discussion-thread'],
        })));
        check(ov200.ok && await page.eval(`document.documentElement.scrollWidth<=innerWidth+1`),
          `${name}/${theme}/${width}/200%: text reflows without unintended overflow (${ov200.reasons.join('; ')})`);
        if (name === 'contract-fixture-flat') await discussionR31Geometry(page, width, 200, `${name}/${theme}/${width}/200%`);
        await page.capture(`discussion-${name}-${width}-200-${theme}`, {
          scenario: 'D2T-03-large-text',
          state: `text200-${theme}`,
          boundary,
          build: BUILD_SHA,
          textScale: 200,
          requestedScene: { width, height: 900, scale: 200, theme },
          textScaleLane: 'dom-override-geometry-only',
          requiredTargets: [anchor],
          overflow: ov200,
        });
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

  console.log('unsaved edit after restart keeps live buffer without export:');
  await page.typeInBody(STANZA);
  const current = await page.body();
  check(current === state.text + STANZA, 'unsaved stanza typed after the restart');
  check(!await page.eval(`Boolean(document.querySelector('#export'))`), 'no export button after restart');
  check((await page.status()) === 'unsaved', 'typing after restart marks draft unsaved');
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
  else if (args.phase === 'production-edge') {
    const { productionEdge } = await import('../../../infra/cloudflare/production/browser.mjs');
    await productionEdge(page, { args, check });
  }
  else if (args.phase === 'document-budget') await documentBudget(page, { args, check });
  else if (args.phase === 'reader-intent-cost') await readerIntentCost(page, { args, check });
  else if (args.phase === 'perf-baseline') await webPerfBaseline(page, { args, check });
  else if (args.phase === 'reader-containment') await readerContainment(page, { args, check });
  else if (args.phase === 'return-position') await discoveryReturnPosition(page, { args, check });
  else if (args.phase === 'select-chevron') await selectChevron(page);
  else if (args.phase === 'editor-chrome') await editorChrome(page);
  else if (args.phase === 'editor-first-draft') await firstDraftFormat(page, { args, check });
  else if (args.phase === 'reader-guest-history') await readerGuestHistory(page, { args, check });
  else if (args.phase === 'main') await mainPhase(page);
  else if (args.phase === 'public-storage') { /* exercised below, with profile reopen */ }
  else if (args.phase === 'after-restart') await afterRestartPhase(page);
  else if (args.phase === 'endorsement') await endorsementAsReader(page, { args, check, captureMatrix: captureDiscussionMatrix });
  else if (args.phase === 'review-withdrawn') {
    await reviewWithdrawnPage(page, { args, check, captureMatrix: captureDiscussionMatrix });
    await moderationQueues(page, { args, check });
    await markdownReader(page, { args, check, textContrast });
  }
  else if (args.phase === 'auth') await authWeb(page, { args, check, openPage: (isolated) => openPage(cdp, isolated) });
  else if (args.phase === 'reader-auth-closed') await readerAuthClosed(page, { args, check });
  else if (args.phase === 'auth-password') await passwordAuth(page, { args, check, textContrast, openPage: (isolated) => openPage(cdp, isolated) });
  else if (args.phase === 'publication-author') await publicationAuthor(page, { args, check, openPage: (isolated) => openPage(cdp, isolated), capture: (target, name, evidence) => target.capture(name, evidence) });
  else if (args.phase === 'discovery-corpus') await discoveryCorpus(page, { args, check });
  else if (args.phase === 'discovery-chip-geometry') await discoveryChipGeometry(page, { args, check });
  else if (args.phase === 'public-search') await publicReading(page);
  else if (args.phase === 'public-cache') await tracePublicReadLifecycle(page);
  else if (args.phase === 'author-works') await authorWorks(page, {args,check,DraftGate,sleep});
  else if (args.phase === 'desk-empty') await deskEmptyState(page, {args,check,DraftGate,textContrast});
  else if (args.phase === 'desk-guest') await deskGuest(page, {args,check}, false);
  else if (args.phase === 'desk-empty-context') { await draftDashboard(page); await deskEmptyState(page, {args,check,DraftGate,textContrast}); }
  else if (args.phase === 'desk-layout') { await draftDashboard(page); await draftR6Hierarchy(page); await timestampPresentation(page); await publicReading(page); }
  else if (args.phase === 'bottom-navigation') await bottomNavigationRegression(page);
  else if (args.phase === 'appearance') await adaptiveShell(page);
  else if (args.phase === 'first-paint') {
    await page.goto(`${args.base}/toi`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'appearance hydration');
    await appearanceFirstPaint(page);
  }
  else throw new Error(`unknown phase ${args.phase}`);
  if (args.phase === 'main' || args.phase === 'public-storage') {
    const state = await publicStorage(page, { args, check, openPage: () => openPage(cdp) });
    const stopped = new Promise(resolve => chrome.child.once('exit', resolve));
    chrome.child.kill(); await stopped;
    Object.assign(chrome, await launchChrome(chrome.profile));
    const reopenedCdp = new Cdp(chrome.url); await reopenedCdp.open();
    const reopened = await openPage(reopenedCdp); activePage = reopened;
    await storageReopened(reopened, args, check, state);
    check(reopened.errors.length === 0, 'reopened storage page has no console errors');
  }
  check(page.errors.length === 0, `no console errors or exceptions${page.errors.length ? `: ${page.errors.join(' | ')}` : ''}`);
  console.log(`browser-smoke: PASS (${args.phase})`);
} catch (error) {
  if (activePage) {
    try {
      await activePage.capture('failure', { allowIncomplete: true, skipSceneValidation: true, state: 'failure-capture' });
      console.error('browser context:', await activePage.eval(`({ path: location.pathname, readyState: document.readyState, preferences: { ...document.documentElement.dataset }, active: { tag: document.activeElement?.tagName, id: document.activeElement?.id }, selectionLength: getSelection().toString().length, view: document.querySelector('.cm-editor')?.__vinkView, status: document.querySelector('[data-testid=save-status]')?.dataset.status, wasmPerf: performance.getEntriesByType('resource').filter(e => e.name.includes('/pkg/')).map(e => ({ name: e.name.split('/').pop(), dur: Math.round(e.duration), status: e.responseStatus, size: e.transferSize })), errors: ${JSON.stringify(activePage.errors)} })`));
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
