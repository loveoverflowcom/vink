// Type-checks the runtime and bundles it into one ES module for the browser.
//
//   node scripts/build.mjs <outfile>
//
// Run by `cargo xtask build` after `npm ci`; the output is served at /pkg/vink-editor.js.

import { spawnSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const outfile = process.argv[2];
if (!outfile) {
  console.error('usage: node scripts/build.mjs <outfile>');
  process.exit(2);
}

const tsc = spawnSync(process.execPath, [join(root, 'node_modules/typescript/bin/tsc'), '-p', root], {
  stdio: 'inherit',
});
if (tsc.status !== 0) {
  console.error('vink-editor: type check failed');
  process.exit(1);
}

await build({
  absWorkingDir: root,
  entryPoints: ['src/editor.ts'],
  bundle: true,
  format: 'esm',
  minify: true,
  target: 'es2022',
  legalComments: 'eof',
  outfile: resolve(outfile),
});
