// #256/#257: measure separate Markdown stages with the pinned parser and the actual served
// runtime. The fixture CodeMirror view has no save session or Leptos outline DOM.
import { writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

function corpus(size) {
  const start = '# Vệt mưa\n\n👩🏽‍💻 Vie\u0323\u0302t\n\n';
  const end = '\n\n## Cuối tài liệu\n';
  const paragraph = 'Mưa qua **mái nhà** và *hiên cũ* bên [Vink](/).\n\n'.repeat(12);
  const block = '## Tiếng mưa\n\n' + paragraph + '### Phố cũ\n\n';
  const count = size - [...(start + end)].length;
  const width = [...block].length;
  return start + block.repeat(Math.floor(count / width)) + 'x'.repeat(count % width) + end;
}

export async function markdownBudget(page, { args, check }) {
  const pkg = new URL('../editor/package.json', import.meta.url);
  const editorRoot = dirname(fileURLToPath(pkg));
  const { build } = createRequire(pkg)('esbuild');
  const { outputFiles } = await build({
    absWorkingDir: editorRoot,
    stdin: {
      contents: "export { commonmarkLanguage } from '@codemirror/lang-markdown'; " +
        "export { EditorState } from '@codemirror/state'; export { outlineFromTree } from './src/markdown.ts';",
      loader: 'ts',
      resolveDir: editorRoot,
    },
    bundle: true, format: 'esm', minify: true, target: 'es2022', write: false,
  });
  const harnessUrl = 'data:text/javascript;base64,' + Buffer.from(outputFiles[0].contents).toString('base64');
  await page.send('Page.bringToFront');
  const browser = (await page.cdp.send('Browser.getVersion')).product;
  const samplesBySize = [];
  for (const size of [10_000, 50_000, 100_000, 200_000]) {
    const source = corpus(size);
    const result = await page.eval(`(async () => {
      const { commonmarkLanguage, EditorState, outlineFromTree } = await import(${JSON.stringify(harnessUrl)});
      const { createEditor, destroyEditor } = await import('/pkg/vink-editor.js');
      const source = ${JSON.stringify(source)};
      const samples = { parse: [], traversal: [], typing: [], caret: [] };
      const parser = commonmarkLanguage.parser;
      const full = parser.parse(source);
      const state = EditorState.create({ doc: source });
      const headings = outlineFromTree(state, full, 1, 'ready').items;
      const host = document.createElement('div');
      host.className = 'vk-editor-body';
      host.style.cssText = 'position:fixed;inset:10px auto auto 10px;width:720px;height:425px;overflow:auto';
      document.body.append(host);
      const handle = createEditor(host, {
        text: source, format: 'markdown', labelledBy: '', describedBy: '',
        onChange() {}, onOutline(json) { JSON.parse(json); },
      });
      const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
      const time = (key, index, operation) => {
        const start = performance.now();
        operation();
        const elapsed = performance.now() - start;
        if (index >= 0) samples[key].push(elapsed);
      };
      try {
        const view = handle.view;
        const locations = headings.slice(1, 7);
        for (let i = -5; i < 40; i++) {
          time('parse', i, () => parser.parse(source));
          time('traversal', i, () => outlineFromTree(state, full, 1, 'ready'));
          await frame();
          const heading = locations[(i + 5) % locations.length];
          const at = view.state.doc.lineAt(heading.from).to;
          time('caret', i, () => view.dispatch({ selection: { anchor: at } }));
          time('typing', i, () => view.dispatch({ changes: { from: at, insert: 'a' }, userEvent: 'input.type' }));
          view.dispatch({ changes: { from: at, to: at + 1 } });
        }
        const percentile = values => {
          const sorted = values.toSorted((a, b) => a - b);
          return Number(sorted[Math.ceil(values.length * .95) - 1].toFixed(3));
        };
        return {
          scalars: [...source].length, utf16: source.length, headings: headings.length,
          atEnd: headings.at(-1)?.title === 'Cuối tài liệu', unchanged: view.state.doc.toString() === source,
          samples, p95Ms: Object.fromEntries(Object.entries(samples).map(([key, values]) => [key, percentile(values)])),
        };
      } finally {
        destroyEditor(handle);
        host.remove();
      }
    })()`);
    check(result.scalars === size && result.atEnd && result.unchanged &&
      Object.values(result.samples).every(samples => samples.length === 40 && samples.every(Number.isFinite)),
    `#257 Chrome Markdown budget ${size} scalars, p95 ms ${JSON.stringify(result.p95Ms)}`);
    samplesBySize.push(result);
  }
  writeFileSync(join(dirname(args.state), 'editor-markdown-p95.json'), JSON.stringify({
    browser, buildProfile: args['build-profile'] ?? 'unspecified', warmups: 5,
    measures: {
      parse: 'cold full tree without fragments',
      traversal: 'production outline extraction on an already parsed full tree',
      typing: 'synchronous one-character CodeMirror transaction, including synchronous listeners',
      caret: 'synchronous selection-only CodeMirror transaction across headings',
    },
    samplesBySize,
  }, null, 2));
}

/** Runs in the page before hydration. Record the same ready boundary regardless of CDP polling. */
function observeReady() {
  const observer = new MutationObserver(() => {
    if (!document.querySelector('[data-testid=editor-body][data-state=ready] .cm-content')) return;
    window.editorReadyMs = performance.now();
    observer.disconnect();
  });
  observer.observe(document, { childList: true, subtree: true, attributes: true });
}

async function sampleDeskEdits() {
  const view = document.querySelector('[data-testid=editor-body] .cm-content').cmTile.root.view;
  view.focus();
  const source = view.state.doc.toString();
  const samples = { typing: [], caret: [], frameIntervals: [], longTasks: [] };
  const recordTasks = entries => samples.longTasks.push(...entries.map(e => e.duration));
  const observer = new PerformanceObserver(list => recordTasks(list.getEntries()));
  const frame = () => new Promise(resolve => requestAnimationFrame(resolve));
  const intervals = [];
  let last = await frame();
  for (let i = 0; i < 15; i++) {
    const now = await frame();
    intervals.push(now - last);
    last = now;
  }
  const framePeriod = intervals.toSorted((a, b) => a - b)[7];
  const at = view.state.doc.line(3).to;
  for (let i = -5; i < 40; i++) {
    if (i === 0) observer.observe({ type: 'longtask' });
    const start = performance.now();
    view.dispatch({ selection: { anchor: at - (i & 1) } });
    const caret = performance.now() - start;
    const before = performance.now();
    view.dispatch({ changes: { from: at, insert: 'ế' }, userEvent: 'input.type' });
    const typing = performance.now() - before;
    view.dispatch({ changes: { from: at, to: at + 1 } });
    const now = await frame();
    if (i >= 0) {
      samples.typing.push(typing);
      samples.caret.push(caret);
      samples.frameIntervals.push(now - last);
    }
    last = now;
  }
  recordTasks(observer.takeRecords());
  observer.disconnect();
  return { samples, framePeriod, unchanged: view.state.doc.toString() === source,
    estimatedMissedFrames: samples.frameIntervals.reduce((sum, v) =>
      sum + Math.max(0, Math.round(v / framePeriod) - 1), 0) };
}

// Measure browser input on the real Product contenteditable, outside CDP round-trip time.
// rAF runs before paint. The following task is a rendering-opportunity proxy, not a display timestamp.
function observeInputFrame() {
  const el = document.querySelector('[data-testid=editor-body] .cm-content');
  el.focus();
  window.inputFrameSample = new Promise(resolve => {
    el.addEventListener('beforeinput', event => {
      const start = event.timeStamp;
      requestAnimationFrame(() => {
        const nextFrameMs = performance.now() - start;
        setTimeout(() => resolve({ nextFrameMs, afterFrameTaskMs: performance.now() - start }), 0);
      });
    }, { once: true });
  });
}

async function createBenchmarkDraft(source) {
  const headers = { 'Content-Type': 'application/json' };
  const create = await fetch('/api/v1/drafts', { method: 'POST', headers,
    body: JSON.stringify({ format: 'markdown' }) });
  if (create.status !== 201) throw Error('create benchmark draft ' + create.status);
  const { work_id: id } = await create.json();
  const save = await fetch('/api/v1/drafts/' + id, { method: 'PUT', headers,
    body: JSON.stringify({ expected_version: 1, document: { format: 'markdown', source } }) });
  if (!save.ok) throw Error('save benchmark draft ' + save.status);
  return id;
}

function readLoadSample() {
  const assets = performance.getEntriesByType('resource')
    .filter(e => new URL(e.name).pathname.endsWith('/vink-editor.js'))
    .map(e => ({ duration: e.duration, transferSize: e.transferSize }));
  return { readyMs: window.editorReadyMs, assets };
}

export async function deskBudget(page, { args, check }) {
  await page.viewport(1440, 900);
  const source = corpus(10_000);
  const id = await page.eval('(' + createBenchmarkDraft + ')(' + JSON.stringify(source) + ')');
  const route = args.base + '/ban-viet/' + id;
  const hook = await page.send('Page.addScriptToEvaluateOnNewDocument', { source: '(' + observeReady + ')()' });
  const loads = { cold: [], warm: [] };
  try {
    for (let i = -2; i < 40; i++) {
      for (const mode of ['cold', 'warm']) {
        if (mode === 'cold') await page.send('Network.clearBrowserCache');
        await page.goto(route);
        await page.waitFor('window.editorReadyMs', 'hydrated desk mounts the editor');
        const sample = await page.eval('(' + readLoadSample + ')()');
        check(sample.assets.length === 1, 'one lazy editor import per Desk document');
        if (i >= 0) loads[mode].push(sample);
      }
    }
  } finally {
    await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: hook.identifier });
  }
  const edits = await page.eval('(' + sampleDeskEdits + ')()');
  check(edits.unchanged && edits.samples.typing.length === 40, '40 paired edits on the mounted Product desk preserve the source');
  const inputFrames = [];
  for (let i = -5; i < 40; i++) {
    await page.eval('(' + observeInputFrame + ')()');
    await page.send('Input.insertText', { text: 'ế' });
    const measured = await page.eval('window.inputFrameSample');
    if (i >= 0) inputFrames.push(measured);
  }
  check(await page.eval("document.querySelector('.cm-content').cmTile.root.view.state.doc.length") === source.length + 45,
    'Product contenteditable accepted all 45 CDP committed characters');
  const lifecycle = await page.eval('(' + sampleLifecycle + ')()');
  check(lifecycle.remainingViews === 0 && lifecycle.remainingHosts === 0 && lifecycle.staleCallbacks === 0,
    '40 runtime mount/destroy cycles release editor DOM and callback owners');
  writeFileSync(join(dirname(args.state), 'desk-core-budget.json'), JSON.stringify({
    browser: (await page.cdp.send('Browser.getVersion')).product, buildProfile: args['build-profile'],
    viewport: [1440, 900], scalars: 10_000, loads, edits, inputFrames, lifecycle,
  }, null, 2));
  // Flush the last edit before leaving this measured scratch draft.
  await page.waitStatus('saved', 'autosave settled after the measured scratch edits');
  for (const [width, scale, theme] of [[320, '200', 'dark'], [390, '100', 'light'],
    [834, '200', 'light'], [1440, '100', 'dark']]) {
    await page.viewport(width);
    await page.eval("document.documentElement.dataset.text = " + JSON.stringify(scale) +
      ";document.documentElement.dataset.theme = " + JSON.stringify(theme) +
      ";new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))");
    await page.eval("document.querySelector('[data-testid=editor-body]').scrollIntoView({block:'start',behavior:'instant'})");
    await page.capture('core-desk-' + [width, scale, theme].join('-'), {
      scenario: 'editor-format', target: '[data-testid=editor-body] .cm-content',
      textScaleLane: 'dom-override-geometry-only', boundary: 'Product Desk, persisted Markdown, Chrome CDP',
    });
  }
  await page.viewport(1440, 900);
  await page.goto(args.base + '/ban-viet');
  await page.waitFor("document.documentElement.dataset.hydrated === 'true'", 'Desk list after budget');
}

async function sampleLifecycle() {
  const {createEditor, destroyEditor} = await import('/pkg/vink-editor.js');
  const host = document.createElement('div');
  document.body.append(host);
  let staleCallbacks = 0;
  try {
    for (let i = 0; i < 40; i++) {
      let ended = false;
      const handle = createEditor(host, { preset: 'document', format: 'markdown', text: '# Mưa',
        labelledBy: '', describedBy: '', onChange() {}, onOutline() { if (ended) staleCallbacks++; } });
      handle.view.dispatch({ changes: {from: 5, insert: '!'} });
      destroyEditor(handle);
      ended = true;
    }
    await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
    return { remainingViews: host.querySelectorAll('.cm-editor').length,
      remainingHosts: Number(host.hasAttribute('data-vink-editor')), staleCallbacks };
  } finally {
    host.remove();
  }
}
