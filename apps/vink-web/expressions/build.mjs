import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdirSync, rmSync, copyFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { validate, sha256 } from '../../../packages/expressions/generate.mjs';

const dir = dirname(fileURLToPath(import.meta.url)), root = resolve(dir, '../../..');
const out = resolve(process.argv[2] ?? join(root, 'target/site/pkg/expressions'));
const catalogDir = join(root, 'packages/expressions');
const manifest = validate(JSON.parse(readFileSync(join(catalogDir, 'shared/catalog.json'))), name => readFileSync(join(catalogDir, 'assets', name)));
const provenance = JSON.parse(readFileSync(join(dir, 'runtime-provenance.json')));
const packageDir = join(dir, 'node_modules/@lottiefiles/dotlottie-web');
const metadata = JSON.parse(readFileSync(join(packageDir, 'package.json')));
assert.equal(metadata.version, provenance.version);
assert.equal(metadata.license, 'MIT');
for (const [file, hash] of Object.entries(provenance.sha256)) assert.equal(sha256(readFileSync(join(packageDir, file))), hash, `runtime integrity: ${file}`);
const wasm = readFileSync(join(packageDir, 'dist/dotlottie-player.wasm'));
const wasmFile = `dotlottie-player-${sha256(wasm).slice(0, 16)}.wasm`;
const wasmPath = `/pkg/expressions/runtime/${wasmFile}`;
const result = await build({
  stdin: { contents: 'export { DotLottie } from "@lottiefiles/dotlottie-web";', resolveDir: dir },
  bundle: true, format: 'esm', platform: 'browser', target: 'es2022', minify: true, write: false,
  legalComments: 'inline',
  plugins: [{ name: 'self-host-only', setup(builder) {
    builder.onLoad({ filter: /dotlottie-web\/dist\/index\.js$/ }, async args => {
      const source = readFileSync(args.path, 'utf8');
      const upstream = /https:\/\/(?:cdn\.jsdelivr\.net\/npm|unpkg\.com)\/@lottiefiles\/dotlottie-web@0\.82\.1\/dist\/dotlottie-player\.wasm/g;
      assert.equal([...source.matchAll(upstream)].length, 2, 'review pinned loader when upstream changes');
      return { contents: source.replace(upstream, wasmPath), loader: 'js' };
    });
  } }],
});
const engine = result.outputFiles[0].contents;
assert(!/cdn\.jsdelivr|unpkg\.com|new Worker\(/.test(Buffer.from(engine).toString()), 'no CDN or worker in this software pilot');
const engineFile = `dotlottie-${sha256(engine).slice(0, 16)}.js`;
const glue = readFileSync(join(dir, 'pilot.mjs'), 'utf8')
  .replace('__ANIMATION_FILES__', JSON.stringify(manifest.assets.map(a => a.animation.file)))
  .replace('__ENGINE_FILE__', engineFile).replace('__WASM_FILE__', wasmFile);
rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, 'runtime'), { recursive: true }); mkdirSync(join(out, 'assets'));
writeFileSync(join(out, 'runtime', engineFile), engine);
writeFileSync(join(out, 'runtime', wasmFile), wasm);
writeFileSync(join(out, 'pilot.js'), glue);
for (const a of manifest.assets) for (const type of ['poster', 'animation']) copyFileSync(join(catalogDir, 'assets', a[type].file), join(out, 'assets', a[type].file));
for (const file of ['NOTICE.txt', 'LICENSE-DOTLOTTIE.txt', 'LICENSE-THIRD-PARTY.txt']) copyFileSync(join(dir, file), join(out, file));
copyFileSync(join(catalogDir, 'LICENSE-STATIC.txt'), join(out, 'LICENSE-STATIC.txt'));
copyFileSync(join(catalogDir, 'LICENSE-ANIMATED.md'), join(out, 'LICENSE-ANIMATED.md'));
const sizes = { version: metadata.version, backend: 'software', worker: false, engine: { file: engineFile, bytes: engine.length }, wasm: { file: wasmFile, bytes: wasm.length }, glue_bytes: Buffer.byteLength(glue), posters_bytes: manifest.assets.reduce((s, a) => s + a.poster.bytes, 0), animation_bytes: manifest.assets.reduce((s, a) => s + a.animation.bytes, 0) };
writeFileSync(join(out, 'build.json'), JSON.stringify(sizes, null, 2) + '\n');
console.log('PASS self-hosted expression runtime', JSON.stringify(sizes));
