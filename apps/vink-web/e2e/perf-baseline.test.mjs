import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  buildGoNoGoLedger,
  classifyRequest,
  evaluateColdHydration,
  evaluateDisposalBounds,
  evaluateLateResponseFence,
  evaluateStaleBurst,
  evaluateWarmBack,
  summarizeDistribution,
  summarizeStepLedger,
} from './perf-baseline.mjs';

test('classifyRequest keeps catalog, discussion, and own reads distinct from public list/reader', () => {
  assert.equal(classifyRequest('http://127.0.0.1:8080/pkg/vink-web.wasm'), 'asset_bundle');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works?limit=12'), 'public_works_list');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works/abc-1/reader'), 'public_reader');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works/abc-1/publication'), 'public_publication_legacy');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works/abc-1/publication/set-private', 'POST'), 'author_mutation');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/tags'), 'public_catalog');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works/abc-1/discussion/public?limit=20'), 'public_discussion');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works/abc-1/discussion/me'), 'own_discussion');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works/abc-1/discussion'), 'capability_discussion');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works/abc-1/endorsements/me'), 'own_endorsements');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/works/abc-1/endorsements'), 'public_endorsements');
  assert.equal(classifyRequest('http://127.0.0.1:8080/api/v1/auth/session'), 'auth_account');
  assert.equal(classifyRequest('http://127.0.0.1:8080/'), 'document_ssr');
  assert.equal(classifyRequest('http://127.0.0.1:8080/tac-pham/abc-1'), 'document_ssr');
  assert.equal(classifyRequest('http://127.0.0.1:8080/favicon.ico'), 'other');
});

test('summarizeStepLedger separates logical reads, transport attempts, 304 validators, and raw-304 fallback pairs', () => {
  const ledger = summarizeStepLedger(
    [
      {
        seq: 1,
        url: 'http://127.0.0.1:8080/api/v1/tags',
        pathnameWithSearch: '/api/v1/tags',
        method: 'GET',
        ifNoneMatch: false,
        wireStatus: 200,
        appStatus: 200,
        encodedDataLength: 512,
      },
      {
        seq: 2,
        url: 'http://127.0.0.1:8080/api/v1/works?limit=12',
        pathnameWithSearch: '/api/v1/works?limit=12',
        method: 'GET',
        ifNoneMatch: false,
        wireStatus: 304,
        appStatus: 304,
        encodedDataLength: 0,
      },
      {
        seq: 3,
        url: 'http://127.0.0.1:8080/api/v1/works?limit=12',
        pathnameWithSearch: '/api/v1/works?limit=12',
        method: 'GET',
        ifNoneMatch: false,
        wireStatus: 200,
        appStatus: 200,
        encodedDataLength: 2048,
      },
      {
        seq: 4,
        url: 'http://127.0.0.1:8080/api/v1/works?limit=12',
        pathnameWithSearch: '/api/v1/works?limit=12',
        method: 'GET',
        ifNoneMatch: true,
        wireStatus: 304,
        appStatus: 304,
        encodedDataLength: 0,
      },
      {
        seq: 5,
        url: 'http://127.0.0.1:8080/api/v1/works/w1/discussion/public?limit=20',
        pathnameWithSearch: '/api/v1/works/w1/discussion/public?limit=20',
        method: 'GET',
        ifNoneMatch: false,
        wireStatus: 200,
        appStatus: 200,
        encodedDataLength: 1024,
      },
      {
        seq: 6,
        url: 'http://127.0.0.1:8080/api/v1/works/w1/discussion/me',
        pathnameWithSearch: '/api/v1/works/w1/discussion/me',
        method: 'GET',
        ifNoneMatch: false,
        failed: true,
        encodedDataLength: 0,
      },
    ],
    'stale_revalidation',
  );

  assert.equal(ledger.reason, 'stale_revalidation');
  assert.equal(ledger.transport_attempts, 6);
  assert.equal(ledger.logical_reads, 5);
  assert.equal(ledger.validator_304_count, 1);
  assert.equal(ledger.total_encoded_bytes, 3584);

  assert.equal(ledger.by_category.public_works_list.transport_attempts, 3);
  assert.equal(ledger.by_category.public_works_list.logical_reads, 2);
  assert.equal(ledger.by_category.public_works_list.validator_304_count, 1);
  assert.equal(ledger.by_category.public_catalog.logical_reads, 1);
  assert.equal(ledger.by_category.public_discussion.logical_reads, 1);
  assert.equal(ledger.by_category.own_discussion.logical_reads, 1);
  assert.deepEqual(ledger.by_category.own_discussion.wire_statuses, ['failed']);
});

test('summarizeDistribution reports range, median, lab_p95, noise and leaves field_p75 null', () => {
  const empty = summarizeDistribution([null, undefined, Number.NaN]);
  assert.equal(empty.n, 0);
  assert.equal(empty.field_p75, null);

  const dist = summarizeDistribution([10, 12, 14, 16, 18, 20, 22, 24, 26, 50]);
  assert.equal(dist.n, 10);
  assert.equal(dist.min, 10);
  assert.equal(dist.max, 50);
  assert.equal(dist.range, 40);
  assert.equal(dist.median, 19);
  assert.equal(dist.lab_p95, 50);
  assert.ok(dist.iqr > 0);
  assert.ok(dist.cv_pct > 0);
  assert.equal(dist.field_p75, null);
});

test('cold hydration, warm back, stale burst, late response, and disposal oracles reject negative controls', () => {
  const limits = { discovery_default_page_size: 12, discussion_default_root_page: 20 };

  const cleanExplore = summarizeStepLedger(
    [{ url: 'http://127.0.0.1:8080/api/v1/tags', pathnameWithSearch: '/api/v1/tags', wireStatus: 200 }],
    'cold_ssr_hydration',
  );
  assert.equal(
    evaluateColdHydration({ routeKind: 'explore', ledger: cleanExplore, dom: { visible_work_rows: 12 }, limits }).ok,
    true,
  );
  const dupExplore = summarizeStepLedger(
    [{ url: 'http://127.0.0.1:8080/api/v1/works?limit=12', pathnameWithSearch: '/api/v1/works?limit=12', wireStatus: 200 }],
    'cold_ssr_hydration',
  );
  assert.equal(
    evaluateColdHydration({ routeKind: 'explore', ledger: dupExplore, dom: { visible_work_rows: 13 }, limits }).ok,
    false,
  );

  const cleanReader = summarizeStepLedger(
    [
      { url: 'http://127.0.0.1:8080/api/v1/works/w1/discussion/public?limit=20', wireStatus: 200 },
      { url: 'http://127.0.0.1:8080/api/v1/works/w1/discussion/me', wireStatus: 401 },
    ],
    'cold_ssr_hydration',
  );
  assert.equal(
    evaluateColdHydration({ routeKind: 'reader', ledger: cleanReader, dom: { visible_comment_roots: 20 }, limits }).ok,
    true,
  );
  const oversizedReader = evaluateColdHydration({
    routeKind: 'reader',
    ledger: cleanReader,
    dom: { visible_comment_roots: 21 },
    limits,
  });
  assert.equal(oversizedReader.ok, false);

  assert.equal(
    evaluateWarmBack({
      ledger: summarizeStepLedger([], 'warm_client_nav'),
      sawLoading: false,
      restoredFocusOk: true,
    }).ok,
    true,
  );
  assert.equal(
    evaluateWarmBack({
      ledger: dupExplore,
      sawLoading: true,
      restoredFocusOk: false,
    }).ok,
    false,
  );

  const validBurst = summarizeStepLedger(
    [
      {
        url: 'http://127.0.0.1:8080/api/v1/works?limit=12',
        pathnameWithSearch: '/api/v1/works?limit=12',
        ifNoneMatch: true,
        wireStatus: 304,
      },
    ],
    'stale_revalidation',
  );
  assert.equal(evaluateStaleBurst({ ledger: validBurst }).ok, true);

  const doubleBurst = summarizeStepLedger(
    [
      {
        url: 'http://127.0.0.1:8080/api/v1/works?limit=12',
        pathnameWithSearch: '/api/v1/works?limit=12',
        ifNoneMatch: true,
        wireStatus: 304,
      },
      {
        url: 'http://127.0.0.1:8080/api/v1/works?limit=12',
        pathnameWithSearch: '/api/v1/works?limit=12',
        ifNoneMatch: true,
        wireStatus: 304,
      },
    ],
    'stale_revalidation',
  );
  assert.equal(evaluateStaleBurst({ ledger: doubleBurst }).ok, false);

  assert.equal(
    evaluateLateResponseFence({ expectedLatestActive: true, leakedObsoleteContent: false }).ok,
    true,
  );
  assert.equal(
    evaluateLateResponseFence({ expectedLatestActive: true, leakedObsoleteContent: true }).ok,
    false,
  );

  const stableCycles = Array.from({ length: 25 }, () => ({
    active_timers: 0,
    cdp_js_event_listeners: 12,
    active_window_doc_listeners: 4,
  }));
  assert.equal(evaluateDisposalBounds({ cycles: stableCycles }).ok, true);

  const leakyCycles = stableCycles.map((c, idx) =>
    idx === 24 ? { active_timers: 3, cdp_js_event_listeners: 25, active_window_doc_listeners: 11 } : c,
  );
  const leakyVerdict = evaluateDisposalBounds({ cycles: leakyCycles });
  assert.equal(leakyVerdict.ok, false);
  assert.equal(leakyVerdict.reasons.length, 3);
});

test('buildGoNoGoLedger emits baseline and conditional downstream decisions', () => {
  const fastLedger = buildGoNoGoLedger({
    configStats: {
      desktop_1440_dpr1: {
        warm_back_ms: { median: 22 },
        cold_explore: { lcp_ms: { median: 140 } },
        cold_reader_200k: { hydrated_ms: { median: 180 } },
      },
    },
    ledgerSummary: {},
    disposalOk: true,
  });
  const byId = Object.fromEntries(fastLedger.map((r) => [r.id, r.decision]));
  assert.equal(byId['WEB-PERF-01'], 'KEEP_BASELINE');
  assert.equal(byId['WEB-PERF-02'], 'HOLD_ASYNC_UNTIL_BOTTLENECK');
  assert.equal(byId['WEB-PERF-03'], 'PREFER_A_URL_ANCHOR_FIRST');
  assert.equal(byId['WEB-PERF-04'], 'NO_CONTAINMENT_NEEDED_YET');
  assert.equal(byId['WEB-PERF-07'], 'MEASURE_BYTE_PRESSURE_NEXT');
  assert.equal(byId['WEB-PERF-06'], 'NO_GO_BEFORE_BUDGET_AND_SSR_DECISION');

  const slowLedger = buildGoNoGoLedger({
    configStats: {
      desktop_1440_dpr1: {
        warm_back_ms: { median: 140 },
        cold_explore: { lcp_ms: { median: 720 } },
        cold_reader_200k: { hydrated_ms: { median: 680 } },
      },
    },
    ledgerSummary: {},
    disposalOk: false,
  });
  const slowById = Object.fromEntries(slowLedger.map((r) => [r.id, r.decision]));
  assert.equal(slowById['WEB-PERF-02'], 'SPIKE_CONDITIONAL');
  assert.equal(slowById['WEB-PERF-03'], 'EVALUATE_ANCHOR_SPIKE');
  assert.equal(slowById['WEB-PERF-04'], 'SPIKE_ON_SECONDARY_ONLY');
  assert.equal(slowById['WEB-PERF-07'], 'INVESTIGATE_DISPOSAL_BOUND');
});
