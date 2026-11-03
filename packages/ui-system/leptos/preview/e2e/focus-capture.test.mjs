import { test } from 'node:test';
import assert from 'node:assert/strict';
import { specimenClips } from './focus-capture.mjs';

test('fractional specimen bounds retain every edge, pad and clamp to the real document', () => {
  assert.deepEqual(specimenClips({ x: 0.5, y: 59.5, width: 319, height: 34.25 }, { width: 320, height: 100 }),
    [{ x: 0, y: 47, width: 320, height: 53, scale: 1 }]);
});

test('a specimen beyond the old 6000px raw cap is fully tiled at original scale', () => {
  const clips = specimenClips({ x: 12, y: 7200, width: 366, height: 9000 }, { width: 390, height: 17000 });
  assert.equal(clips.length, 3);
  assert.equal(clips[0].y, 7188);
  assert.equal(clips.at(-1).y + clips.at(-1).height, 16212);
  assert.equal(clips.reduce((sum, clip) => sum + clip.height, 0), 9024);
  for (let i = 1; i < clips.length; i++) assert.equal(clips[i].y, clips[i - 1].y + clips[i - 1].height);
  assert.ok(clips.every(clip => clip.width === 390 && clip.height <= 4096 && clip.scale === 1));
});

test('nonfinite, empty and off-document geometry fails instead of yielding a misleading image', () => {
  const document = { width: 390, height: 900 };
  for (const bounds of [
    { x: NaN, y: 0, width: 100, height: 100 }, { x: 0, y: 0, width: 0, height: 100 },
    { x: 1000, y: 0, width: 10, height: 10 }, { x: 0, y: -1000, width: 10, height: 10 },
  ]) assert.throws(() => specimenClips(bounds, document));
});

test('viewport-sized tiles balance the final part and cover the heading through the last pixel', () => {
  const clips = specimenClips({ x: 16, y: 12000, width: 343, height: 1801 }, { width: 375, height: 15000 }, 12, 900);
  assert.equal(clips.length, 3);
  assert.equal(clips[0].y, 11988);
  assert.equal(clips.at(-1).y + clips.at(-1).height, 13813);
  assert.ok(clips.every(clip => clip.height >= 600 && clip.height <= 900));
});
