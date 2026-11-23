// Pure invariants for #234's opt-in lab analyzer (no server, Chrome or database required).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { COMPRESSION_PARAMETERS, distribution, measureCompressedBytes } from './web-perf-baseline.mjs';
import { assertNoHydrationRead, classifyTransport, countTransport, createTransportLedger } from './web-request-ledger.mjs';

test('lab summary reports the raw sample size, true median, nearest-rank p95 and spread', () => {
  assert.deepEqual(distribution([10, 3, 9, 1, 7, 4, 2, 8, 6, 5]), {
    count: 10, min: 1, max: 10, median: 5.5, p95_lab: 10, spread_ms: 9,
  });
  assert.deepEqual(distribution([]), {
    count: 0, min: null, max: null, median: null, p95_lab: null, spread_ms: null,
  });
  assert.equal(distribution([1, 2, NaN, Infinity]).count, 2);
});

test('ledger joins wire extras delivered before the request and after completion without retaining values', () => {
  const { rows, feed } = createTransportLedger('http://127.0.0.1:80', () => 'stale');
  feed('Network.requestWillBeSentExtraInfo', { requestId: 'a', headers: { 'If-None-Match': 'private-validator' } });
  feed('Network.requestWillBeSent', { requestId: 'a', type: 'Fetch', request: {
    method: 'GET', url: 'http://127.0.0.1/api/v1/works?q=private-query',
  } });
  feed('Network.responseReceived', { requestId: 'a', response: { status: 200 } });
  feed('Network.loadingFinished', { requestId: 'a', encodedDataLength: 230 });
  feed('Network.responseReceivedExtraInfo', { requestId: 'a', statusCode: 304 });
  assert.equal(countTransport(rows, 'stale', 'public-list'), 1);
  assert.equal(rows[0].validator_sent, true);
  assert.equal(rows[0].response_status, 200);
  assert.equal(rows[0].wire_status, 304);
  assert.equal(rows[0].encoded_bytes, 230);
  assert.deepEqual(rows[0].query_parameter_keys, ['q']);
  assert.ok(!JSON.stringify(rows).includes('private-'));
});

test('redirects do not overwrite earlier attempts or attribute ambiguous extra info to either hop', () => {
  const { rows, feed } = createTransportLedger('http://127.0.0.1', () => 'navigation');
  const hop = (path, redirectResponse) => feed('Network.requestWillBeSent', {
    requestId: 'redirect', type: 'Document', redirectResponse, request: {
      method: 'GET', url: 'http://127.0.0.1' + path,
    },
  });
  hop('/the/alias');
  feed('Network.responseReceivedExtraInfo', { requestId: 'redirect', statusCode: 308 });
  hop('/the/canonical', { status: 308 });
  feed('Network.responseReceived', { requestId: 'redirect', response: { status: 200 } });
  feed('Network.responseReceivedExtraInfo', { requestId: 'redirect', statusCode: 200 });
  assert.deepEqual(rows.map(r => [r.kind, r.response_status, r.wire_status]), [
    ['document', 308, null], ['document', 200, null],
  ]);
});

test('a single excess list GET trips the SSR oracle independently of catalog and own overlays', () => {
  for (const [path, kind] of [
    ['/api/v1/tags', 'catalog'],
    ['/api/v1/works/a/reader', 'reader'],
    ['/api/v1/works/a/discussion/me', 'own-discussion'],
    ['/api/v1/works/a/discussion/public', 'public-discussion'],
    ['/api/v1/works/a/endorsements/me', 'own-endorsement'],
    ['/api/v1/works/a/endorsements', 'public-endorsement'],
  ]) assert.equal(classifyTransport({ path }), kind);
  const sideReads = ['catalog', 'own-discussion', 'own-endorsement'].map(kind => ({ phase: 'cold-0', kind, method: 'GET' }));
  assert.doesNotThrow(() => assertNoHydrationRead(sideReads, 'cold-0', 'public-list'));
  assert.throws(() => assertNoHydrationRead([
    ...sideReads, { phase: 'cold-0', kind: 'public-list', method: 'GET' },
  ], 'cold-0', 'public-list'), /1 unexpected public-list GET/);
});

test('artifact compression helper reports exact raw, gzip-6, brotli-6, brotli-11 and sha256 deterministically', () => {
  const sample = Buffer.from('.bg-vk-surface{background-color:var(--vk-surface);}\n'.repeat(20));
  const first = measureCompressedBytes(sample);
  const second = measureCompressedBytes(sample);
  assert.deepEqual(first, second);
  assert.equal(first.raw, sample.length);
  assert.ok(first.gzip_6 > 0 && first.gzip_6 < first.raw);
  assert.ok(first.brotli_6 > 0 && first.brotli_6 < first.raw);
  assert.ok(first.brotli_11 > 0 && first.brotli_11 < first.raw);
  assert.equal(first.sha256.length, 64);
  assert.equal(COMPRESSION_PARAMETERS.gzip.level, 6);
  assert.equal(COMPRESSION_PARAMETERS.brotli_dynamic.quality, 6);
  assert.equal(COMPRESSION_PARAMETERS.brotli_static.quality, 11);
});
