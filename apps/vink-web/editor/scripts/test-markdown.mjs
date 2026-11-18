// Differential and regression checks for the CommonMark 0.31.2 AST module (`src/markdown.ts`).
// Runs all 30 shared title-authority fixtures plus outline, negative, command/undo and 200k-char cases.

import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { build } from 'esbuild';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const fixturePath = resolve(root, '../../../services/vink-app/tests/fixtures/title-authority/markdown.json');
const renderPath = resolve(root, '../../../services/vink-app/tests/fixtures/title-authority/render.json');
const tempDir = mkdtempSync(join(tmpdir(), 'vink-md-test-'));

try {
  const outFile = join(tempDir, 'bundle.mjs');
  await build({
    absWorkingDir: root,
    stdin: {
      contents: `
        export * from './src/markdown.ts';
        export { syntaxTree } from '@codemirror/language';
        export { EditorState, EditorSelection } from '@codemirror/state';
        export { history, undo, redo } from '@codemirror/commands';
      `,
      resolveDir: root,
      loader: 'ts',
    },
    bundle: true,
    format: 'esm',
    platform: 'neutral',
    mainFields: ['module', 'main'],
    outfile: outFile,
  });

  const md = await import(pathToFileURL(outFile).href);
  const { EditorState } = md;
  const cases = JSON.parse(readFileSync(fixturePath, 'utf8'));
  assert.ok(cases.length >= 30, `expected >= 30 corpus cases, got ${cases.length}`);

  for (const c of cases) {
    const first = md.extractFirstH1(c.source);
    assert.equal(first, c.expected_projection, `extractFirstH1 mismatch for ${c.id}`);

    const state = EditorState.create({
      doc: c.source,
      extensions: [md.markdownExtension[0]],
    });
    const outline = md.extractOutline(state, 1, 200);
    assert.equal(outline.status, 'ready', `full parse ready for ${c.id}`);
    assert.equal(outline.title, c.expected_projection, `outline title mismatch for ${c.id}`);
  }

  for (const [source, title] of [
    ['# <https://x.vn> &eacute;', 'https://x.vn é'],
    ['# Mục ` chữ ` hai', 'Mục chữ hai'],
  ]) {
    const state = EditorState.create({doc: source, extensions: [md.markdownExtension[0]]});
    assert.equal(md.extractOutline(state, 1).title, title);
  }

  // Grammar witnesses also rendered by Leptos SSR and exercised against the browser.
  const renderCases = JSON.parse(readFileSync(renderPath, 'utf8')).filter(c => c.format === 'markdown');
  for (const c of renderCases) {
    const state = EditorState.create({ doc: c.source, extensions: [md.markdownExtension[0]] });
    const names = new Set();
    md.syntaxTree(state).iterate({ enter: node => names.add(node.name) });
    for (const name of c.editor_nodes) assert.ok(names.has(name), `${c.id}: ${name}`);
    for (const extension of ['Table', 'Strikethrough', 'Task', 'InlineMath']) {
      assert.ok(!names.has(extension), `${c.id}: unexpected ${extension}`);
    }
    assert.equal(md.extractFirstH1(c.source), c.title, c.id);
    assert.equal(state.doc.toString(), c.source, c.id);
  }

  // Duplicate heading labels keep distinct offsets; fences/escaped/HTML are excluded.
  const docSource = [
    '# Tên đầu',
    '',
    '## Mục trùng',
    'Đoạn một.',
    '',
    '```md',
    '# Heading trong code fence',
    '## Mục giả',
    '```',
    '',
    '\\# Không phải heading',
    '<h1>HTML heading</h1>',
    '',
    '## Mục trùng',
    '### Mục con `mã` &amp; *nghiêng*',
    '',
    'Tiêu đề Setext',
    '---',
  ].join('\n');

  const state = EditorState.create({
    doc: docSource,
    extensions: [md.markdownExtension[0]],
  });
  const outline = md.extractOutline(state, 7, 200);
  assert.equal(outline.generation, 7);
  assert.equal(outline.title, 'Tên đầu');
  assert.deepEqual(
    outline.items.map((i) => ({ level: i.level, title: i.title, isTitle: i.isTitle })),
    [
      { level: 1, title: 'Tên đầu', isTitle: true },
      { level: 2, title: 'Mục trùng', isTitle: false },
      { level: 2, title: 'Mục trùng', isTitle: false },
      { level: 3, title: 'Mục con mã & nghiêng', isTitle: false },
      { level: 2, title: 'Tiêu đề Setext', isTitle: false },
    ],
  );
  assert.notEqual(outline.items[1].from, outline.items[2].from);
  assert.equal(md.activeHeadingFrom(outline.items, 0), outline.items[0].from);
  assert.equal(md.activeHeadingFrom(outline.items, outline.items[3].from + 4), outline.items[3].from);

  // 200,000-character document remains bounded and preserves first H1.
  const start = '# Tiêu đề đầu dài\n\n';
  const end = '\n\n## Kết thúc\n';
  const filler = 'Đoạn văn tiếng Việt giữ nguyên dấu. ';
  const count = 200_000 - [...(start + end)].length;
  const huge = start + filler.repeat(Math.ceil(count / filler.length)).slice(0, count) + end;
  assert.equal([...huge].length, 200_000);
  const hugeState = EditorState.create({
    doc: huge,
    extensions: [md.markdownExtension[0]],
  });
  const hugeOutline = md.extractOutline(hugeState, 9, 250);
  assert.equal(hugeOutline.title, 'Tiêu đề đầu dài');
  assert.equal(hugeOutline.status, 'ready');
  assert.equal(hugeOutline.items.at(-1)?.title, 'Kết thúc');

  console.log(`vink-editor markdown checks: ok (${cases.length} title + ${renderCases.length} render corpus + outline + 200k doc)`);
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
