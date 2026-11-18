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
