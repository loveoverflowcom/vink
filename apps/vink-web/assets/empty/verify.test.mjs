import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, cpSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { inspect, verify } from './verify.mjs';

test('original issue #258 derivatives satisfy the runtime contract', () => verify());
test('reject an animated flag, truncated RIFF and lossless substitution', () => {
  const original = readFileSync(new URL('./vink-empty-no-results.webp', import.meta.url));
  const animated = Buffer.from(original); animated[20] |= 2;
  assert.throws(() => inspect(animated), /alpha only/);
  assert.throws(() => inspect(original.subarray(0, -1)), /complete RIFF/);
  const lossless = Buffer.from(original); lossless.write('VP8L', lossless.indexOf('VP8 '));
  assert.throws(() => inspect(lossless), /static lossy/);
});
test('reject hash drift even when dimensions and chunks are still valid', () => {
  const dir = mkdtempSync(join(tmpdir(), 'vink-empty-contract-'));
  try {
    cpSync(new URL('.', import.meta.url), dir, { recursive: true });
    const path = join(dir, 'vink-empty-no-results.webp'), bytes = readFileSync(path);
    bytes[bytes.length - 10] ^= 1;
    writeFileSync(path, bytes);
    assert.throws(() => verify(dir), /original supplied bytes/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
