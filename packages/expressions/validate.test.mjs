import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { validate, sha256, generate } from './generate.mjs';
const manifest = JSON.parse(readFileSync(new URL('./shared/catalog.json', import.meta.url)));
const asset = name => readFileSync(new URL(`./assets/${name}`, import.meta.url));

test('pinned inventory and generated outputs match', () => generate());
test('unknown/corrupt/mislicensed/duplicate resources fail closed', () => {
  for (const mutate of [
    m => m.assets[1].id = m.assets[0].id,
    m => m.assets[1].lookup_sequences.push(m.assets[0].unicode),
    m => m.assets[0].poster.sha256 = '0'.repeat(64),
    m => m.assets[0].animation.license = 'MIT',
    m => m.assets[0].poster.file = '../outside.svg',
    m => m.static_source.revision = 'latest',
  ]) { const changed = structuredClone(manifest); mutate(changed); assert.throws(() => validate(changed, asset)); }
});
test('active SVG and external Lottie resources cannot be blessed by a new checksum', () => {
  for (const kind of ['poster', 'animation']) {
    const changed = structuredClone(manifest), resource = changed.assets[0][kind];
    const bytes = Buffer.from(kind === 'poster' ? '<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>' : JSON.stringify({ ...JSON.parse(asset(resource.file)), assets: [{ id: 'image', p: 'https://outside.invalid/p.png' }] }));
    const old = resource.file;
    resource.sha256 = sha256(bytes); resource.file = old.replace(/[a-f0-9]{16}(?=\.)/, resource.sha256.slice(0, 16)); resource.bytes = bytes.length;
    assert.throws(() => validate(changed, name => name === resource.file ? bytes : asset(name)), /active\/external SVG|embedded\/external images forbidden/);
  }
});
