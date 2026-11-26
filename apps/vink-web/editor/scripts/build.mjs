// Type-checks the runtime, runs Markdown AST corpus checks, and bundles into one ES module.
//
//   node scripts/build.mjs <outfile>
//
// Run by `cargo xtask build` after `npm ci`; the output is served at /pkg/vink-editor.js.

import { writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const mode = process.argv[2];
const outfile = mode === '--bundle' ? process.argv[3] : mode;
if (!outfile) {
  console.error('usage: node scripts/build.mjs <outfile>');
  process.exit(2);
}

if (mode !== '--bundle') {
  const tsc = spawnSync(process.execPath, [join(root, 'node_modules/typescript/bin/tsc'), '-p', root], {
    stdio: 'inherit',
  });
  if (tsc.status !== 0) {
    console.error('vink-editor: type check failed');
    process.exit(1);
  }

  const mdTest = spawnSync(process.execPath, [join(root, 'scripts/test-markdown.mjs')], {
    stdio: 'inherit',
  });
  if (mdTest.status !== 0) {
    console.error('vink-editor: markdown corpus checks failed');
    process.exit(1);
  }
}
if (mode === '--check') process.exit(0);

const result = await build({
  absWorkingDir: root,
  entryPoints: ['src/editor.ts'],
  bundle: true,
  format: 'esm',
  minify: true,
  target: 'es2022',
  legalComments: 'eof',
  metafile: mode === '--bundle',
  outfile: resolve(outfile),
});
if (mode === '--bundle') writeFileSync(process.argv[4], JSON.stringify(Object.keys(result.metafile.inputs)));
