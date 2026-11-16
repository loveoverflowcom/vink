// Pure invariants for #234's opt-in lab analyzer (no server, Chrome or database required).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { distribution, classifyTransport, countTransport } from './web-perf-baseline.mjs';

test('lab summary reports the raw sample size, true median, nearest-rank p95 and spread', () => {
  assert.deepEqual(distribution([10, 3, 9, 1, 7, 4, 2, 8, 6, 5]), {
    count: 10, min: 1, max: 10, median: 5.5, p95_lab: 10, spread_ms: 9,
  });
  assert.deepEqual(distribution([]), {
    count: 0, min: null, max: null, median: null, p95_lab: null, spread_ms: null,
  });
  assert.equal(distribution([1, 2, NaN, Infinity]).count, 2);
});

test('request reason ledger keeps conditional reads, 304 and transport separate', () => {
  assert.equal(classifyTransport({ path: '/api/v1/works' }), 'public-list');
  assert.equal(classifyTransport({ path: '/api/v1/works/abc/reader' }), 'reader');
  assert.equal(classifyTransport({ path: '/api/v1/works/abc/discussion/public' }), 'public-discussion');
  assert.equal(classifyTransport({ path: '/api/v1/works/abc/discussion/me' }), 'own-discussion');
  assert.equal(classifyTransport({ path: '/api/v1/tags' }), 'catalog');
  const rows = [
    { phase: 'cold-0', kind: 'public-list', method: 'GET', validator_sent: false, response_status: 200 },
    { phase: 'cold-1', kind: 'public-list', method: 'GET', validator_sent: true, wire_status: 304, response_status: 200 },
  ];
  assert.equal(countTransport(rows, 'cold-0', 'public-list'), 1);
  assert.equal(countTransport(rows, 'cold-1', 'public-list'), 1);
  assert.equal(rows[1].wire_status, 304);
  assert.notEqual(rows[1].wire_status, rows[1].response_status);
});

test('negative duplicate transport control breaks the structural <=1 predicate', () => {
  const one = [{ phase: 'cold-0', kind: 'public-list', method: 'GET' }];
  const duplicate = [...one, { ...one[0] }];
  assert.ok(countTransport(one, 'cold-0', 'public-list') <= 1);
  assert.equal(countTransport(duplicate, 'cold-0', 'public-list') <= 1, false);
  assert.equal(countTransport(duplicate, 'cold-0', 'catalog'), 0);
});
