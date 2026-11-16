// Validate committed derivatives before copying them into the real app bundle.
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { resolve, basename, dirname, join } from 'node:path';
import assert from 'node:assert/strict';

export function inspect(raw) {
  assert.equal(raw.toString('ascii', 0, 4), 'RIFF');
  assert.equal(raw.toString('ascii', 8, 12), 'WEBP');
  assert.equal(raw.readUInt32LE(4) + 8, raw.length, 'complete RIFF');
  const chunks = [];
  let header;
  for (let pos = 12; pos < raw.length;) {
    assert.ok(pos + 8 <= raw.length, 'complete chunk header');
    const tag = raw.toString('ascii', pos, pos + 4), size = raw.readUInt32LE(pos + 4);
    assert.ok(pos + 8 + size + size % 2 <= raw.length, 'complete chunk payload');
    chunks.push(tag);
    if (tag === 'VP8X') header = raw.subarray(pos + 8, pos + 8 + size);
    pos += 8 + size + size % 2;
  }
  assert.deepEqual(chunks, ['VP8X', 'ALPH', 'VP8 '], 'static lossy VP8 + alpha, no metadata/animation');
  assert.equal(header.length, 10);
  assert.equal(header[0], 0x10, 'alpha only, no animation');
  return { width: 1 + header.readUIntLE(4, 3), height: 1 + header.readUIntLE(7, 3), chunks };
}

export function verify(directory = dirname(fileURLToPath(import.meta.url))) {
  const manifest = JSON.parse(readFileSync(join(directory, 'asset-manifest.json')));
  assert.deepEqual(manifest.runtime_assets.map(a => a.id), ['no-results', 'no-public-works', 'first-draft']);
  for (const asset of manifest.runtime_assets) {
    assert.equal(asset.file, `assets/vink-empty-${asset.id}.webp`);
    const raw = readFileSync(join(directory, basename(asset.file)));
    assert.ok(raw.length <= 30 * 1024, '30 KiB budget');
    assert.equal(raw.length, asset.bytes);
    assert.equal(createHash('sha256').update(raw).digest('hex'), asset.sha256, 'original supplied bytes');
    const actual = inspect(raw);
    assert.equal(actual.width, 512);
    assert.equal(actual.height, 427);
    assert.equal(asset.width, actual.width);
    assert.equal(asset.height, actual.height);
    assert.equal(asset.frames, 1);
    assert.equal(asset.alpha, true);
    console.log(`PASS ${asset.id}: ${raw.length} bytes, 512x427, VP8 + alpha, SHA-256 ${asset.sha256}`);
  }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) verify();
