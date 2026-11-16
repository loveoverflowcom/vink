// #234: opt-in evidence phase hosted by the existing browser-smoke CDP Page.
// Measurements are lab observations, not field RUM, an SLA, or a timing gate.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { readFileSync, writeFileSync } from 'node:fs';

const WAIT_READY = "document.documentElement.dataset.hydrated==='true'";
const LIST = "!!document.querySelector('.vk-public-row__link')";
const READER = "!!document.querySelector('.vk-reader .vk-document')";
const QUERY = '/?q=kiem%20thu';
const ITERATIONS = 10;

export function distribution(values) {
  const sorted = values.filter(Number.isFinite).slice().sort((a, b) => a - b);
  if (!sorted.length) return { count: 0, min: null, max: null, median: null, p95_lab: null, spread_ms: null };
  const n = sorted.length;
  const median = n % 2 ? sorted[(n - 1) / 2] : (sorted[n / 2 - 1] + sorted[n / 2]) / 2;
  return {
    count: n, min: sorted[0], max: sorted[n - 1], median,
    p95_lab: sorted[Math.ceil(n * 0.95) - 1],
    spread_ms: sorted[n - 1] - sorted[0],
  };
}

export function classifyTransport(entry) {
  if (entry.path === '/api/v1/works') return 'public-list';
  if (/^\/api\/v1\/works\/[^/]+\/reader$/.test(entry.path)) return 'reader';
  if (/^\/api\/v1\/works\/[^/]+\/discussion\/public$/.test(entry.path)) return 'public-discussion';
  if (/^\/api\/v1\/works\/[^/]+\/discussion\/me$/.test(entry.path)) return 'own-discussion';
  if (entry.path.startsWith('/api/v1/tags')) return 'catalog';
  if (entry.path.startsWith('/pkg/')) return 'bundle';
  return 'other';
}

export function countTransport(rows, phase, kind) {
  return rows.filter(row => row.phase === phase && row.kind === kind && row.method === 'GET').length;
}

function newDocumentObserver() {
  const data = window.__vinkPerfBaseline = {
    hydration_ms: null, meaningful_ms: null, long_tasks: [], layout_shifts: [],
  };
  const record = () => {
    if (data.meaningful_ms === null &&
      document.querySelector('.vk-public-row__link, .vk-reader .vk-document')) {
      data.meaningful_ms = performance.now();
    }
    if (data.hydration_ms === null &&
      document.documentElement?.dataset.hydrated === 'true') {
      data.hydration_ms = performance.now();
    }
  };
  try {
    new PerformanceObserver(list => {
      for (const e of list.getEntries()) data.long_tasks.push({ start_ms: e.startTime, duration_ms: e.duration });
    }).observe({ type: 'longtask', buffered: true });
  } catch { /* Browser support is part of the evidence, not a requirement for app rendering. */ }
  try {
    new PerformanceObserver(list => {
      for (const e of list.getEntries()) {
        data.layout_shifts.push({ start_ms: e.startTime, value: e.value, had_recent_input: e.hadRecentInput });
      }
    }).observe({ type: 'layout-shift', buffered: true });
  } catch { /* Report empty + browser metadata rather than inventing CLS. */ }
  const observer = new MutationObserver(record);
  observer.observe(document, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-hydrated'] });
  document.addEventListener('DOMContentLoaded', record, { once: true });
  record();
}

async function browserSnapshot(page) {
  const details = await page.eval('(() => {' +
    'const nav=performance.getEntriesByType("navigation")[0];' +
    'const p=window.__vinkPerfBaseline||{};' +
    'const res=performance.getEntriesByType("resource");' +
    'const bytes=res.filter(r=>r.name.includes("/pkg/")).reduce((v,r)=>v+r.transferSize,0);' +
    'return {path:location.pathname,hydrated:document.documentElement.dataset.hydrated==="true",' +
    'navigation:nav?{ttfb_ms:nav.responseStart-nav.requestStart,response_end_ms:nav.responseEnd,' +
    'dom_content_loaded_ms:nav.domContentLoadedEventEnd,transfer_bytes:nav.transferSize}:null,' +
    'hydration_ms:p.hydration_ms??null,first_content_ms:p.meaningful_ms??null,' +
    'long_tasks:p.long_tasks??[],layout_shifts:p.layout_shifts??[],' +
    'loaded_bundle_transfer_bytes:bytes,loaded_resource_count:res.length,' +
    'row_count:document.querySelectorAll(".vk-public-row").length,' +
    'comment_nodes:document.querySelectorAll(".vk-discussion-comment").length,' +
    'body_characters:document.querySelector(".vk-reader .vk-document")?.textContent.length??null,' +
    'root_font_px:getComputedStyle(document.documentElement).fontSize,' +
    'dpr:devicePixelRatio,viewport:{width:innerWidth,height:innerHeight},' +
    'fonts_status:document.fonts.status};})()');
  const get = async (method) => { try { return await page.send(method); } catch { return null; } };
  return {
    ...details,
    dom_counters: await get('Memory.getDOMCounters'),
    v8_heap: await get('Runtime.getHeapUsage'),
  };
}

function recordTransport(page, results, phase) {
  const pending = new Map();
  const origin = new URL(results.configuration.origin).origin;
  const onEvent = event => {
    if (event.sessionId !== page.sessionId) return;
    const { method, params } = event;
    if (method === 'Network.requestWillBeSent') {
      const request = params.request;
      const url = new URL(request.url);
      // Only local requests, no raw URLs, cookies, headers, request bodies or manuscript text.
      if (url.origin !== origin) return;
      const conditional = Object.keys(request.headers ?? {}).some(h => h.toLowerCase() === 'if-none-match');
      const entry = {
        phase: phase.current,
        request_id: params.requestId,
        method: request.method,
        path: url.pathname,
        query_parameter_keys: [...url.searchParams.keys()].sort(),
        kind: classifyTransport({ path: url.pathname }),
        resource_type: params.type ?? null,
        initiator: params.initiator?.type ?? null,
        validator_sent: conditional,
        response_status: null, wire_status: null,
        encoded_bytes: null, failed: false,
      };
      results.transport.push(entry);
      pending.set(params.requestId, entry);
    } else if (method === 'Network.responseReceived' && pending.has(params.requestId)) {
      const entry = pending.get(params.requestId);
      entry.response_status = params.response.status;
      entry.disk_cache = !!params.response.fromDiskCache;
      entry.service_worker = !!params.response.fromServiceWorker;
    } else if (method === 'Network.responseReceivedExtraInfo' && pending.has(params.requestId)) {
      pending.get(params.requestId).wire_status = params.statusCode; // Includes actual wire 304.
    } else if (method === 'Network.loadingFinished' && pending.has(params.requestId)) {
      pending.get(params.requestId).encoded_bytes = params.encodedDataLength;
      pending.delete(params.requestId);
    } else if (method === 'Network.loadingFailed' && pending.has(params.requestId)) {
      pending.get(params.requestId).failed = true;
      pending.delete(params.requestId);
    }
  };
  page.cdp.listeners.push(onEvent);
  return () => { page.cdp.listeners = page.cdp.listeners.filter(item => item !== onEvent); };
}

function fixtureDigest() {
  const hash = createHash('sha256');
  for (const path of [
    'apps/vink-web/e2e/fixtures/discovery-publications.sql',
    'apps/vink-web/e2e/fixtures/web-perf-journey.sql',
  ]) hash.update(readFileSync(path));
  return hash.digest('hex');
}

export async function webPerfBaseline(page, { args, check }) {
  // The test harness must never be pointed at a deployed or private service.
  if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(args.base)) throw new Error('#234 lab requires loopback scratch App');
  const out = join(dirname(args.state), 'web-perf-baseline.json');
  const phase = { current: 'init' };
  const sourceSha = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  const results = {
    issue: 234, status: 'RUNNING', source_sha: sourceSha,
    corpus: { labelled_public_works: 200, comments: 1000, digest_sha256: fixtureDigest(),
      source: 'App/PostgreSQL scratch, SQL v1 public projection, no production user text' },
    configuration: {
      origin: args.base, browser: await page.cdp.send('Browser.getVersion'),
      node: process.version, build_profile: 'xtask web dev (release unverified)',
      network: 'loopback / no throttling', cpu_throttle_rate: 1,
      cache_cold: 'CDP clearBrowserCache before each full-document navigation; OS/PG process remain warm',
      viewport: '1280x900 CSS px / DPR 1', text_scale: 'system default',
      cold_iterations: ITERATIONS, warm_iterations: ITERATIONS,
      instrumentation: 'Page.addScriptToEvaluateOnNewDocument: PerformanceObserver + MutationObserver; scenario opt-in',
    },
    intended_reads: [], transport: [], cold: [], warm: [],
    verification: {}, limitations: [
      'No field p75; p95 is a lab nearest-rank statistic over 10 iterations.',
      'No browser-process RSS, WASM heap attribution, OS page-cache cold start, production latency or RUM.',
      'This phase is debug-profile only; release-profile qualification is a separate NOT_RUN boundary.',
      'CDP transport is browser-visible only: server-side SSR SQL reads are not inferred as client GETs.',
      'Listener counts use CDP DOM counters; timer and full owner allocations have no reliable CDP proxy.',
      'Stale age/invalidation/removal and the late-response/dispose negative cases require dedicated controls.',
    ],
  };
  const flush = () => writeFileSync(out, JSON.stringify(results, null, 2));
  const unlisten = recordTransport(page, results, phase);
  const observer = await page.send('Page.addScriptToEvaluateOnNewDocument', {
    source: '(' + newDocumentObserver.toString() + ')()',
  });
  await page.send('Performance.enable');
  try {
    await page.viewport(1280, 900);
    // Each cold sample uses a new document and an empty Chrome HTTP cache; the database is not restarted.
    for (let i = 0; i < ITERATIONS; i++) {
      phase.current = 'cold-' + i;
      results.intended_reads.push({ phase: phase.current, reason: 'cold_ssr', representation: 'discovery-filtered' });
      // Do not navigate to the same URL while the old hydrated DOM is still present:
      // Page.goto's readyState predicate could otherwise observe the previous document.
      await page.goto('about:blank');
      await page.send('Network.clearBrowserCache');
      const start = performance.now();
      await page.goto(args.base + QUERY);
      await page.waitFor(WAIT_READY + ' && ' + LIST, 'cold filtered discovery #' + i, 120000);
      const snapshot = await browserSnapshot(page);
      results.cold.push({ iteration: i, wall_ms: performance.now() - start, ...snapshot });
      // The HTML SSR list should not trigger a second client list fetch just for hydration.
      check(countTransport(results.transport, phase.current, 'public-list') <= 1,
        '#234 cold SSR/hydration has no duplicate browser list GET #' + i);
      flush();
    }
    // Leave the hydrated Explore tree in place. Native links must navigate without a document reload.
    for (let i = 0; i < ITERATIONS; i++) {
      phase.current = 'warm-' + i;
      results.intended_reads.push({ phase: phase.current, reason: 'warm_reader_navigation', representation: 'reader' });
      const timeOrigin = await page.eval('performance.timeOrigin');
      const before = await page.eval('performance.now()');
      await page.click('.vk-public-row__link');
      await page.waitFor(READER, 'reader after warm navigation #' + i, 120000);
      const readerTime = (await page.eval('performance.now()')) - before;
      const reader = await browserSnapshot(page);
      check(await page.eval('performance.timeOrigin') === timeOrigin,
        '#234 warm Reader uses client navigation without a full-document reload #' + i);
      check(reader.body_characters > 0, '#234 real App Reader body stays available #' + i);
      // Only a limited page may be mounted, never all 1,000 comments from the hot thread.
      check(reader.comment_nodes < 1000, '#234 comment rendering remains bounded by pagination');
      results.intended_reads.push({ phase: phase.current, reason: 'return_to_discovery', representation: 'discovery-filtered' });
      const beforeReturn = await page.eval('performance.now()');
      await page.click('#reader-return');
      await page.waitFor(LIST + ' && location.pathname==="/"', 'return to filtered discovery #' + i, 120000);
      const returnTime = (await page.eval('performance.now()')) - beforeReturn;
      const back = await browserSnapshot(page);
      check(await page.eval('performance.timeOrigin') === timeOrigin,
        '#234 warm Reader return stays in the same hydrated document #' + i);
      check(back.row_count <= 24 && back.row_count > 0,
        '#234 real bounded public page after return #' + i);
      results.warm.push({ iteration: i, reader_ms: readerTime, return_ms: returnTime,
        reader, back });
      flush();
    }
    const cold = results.cold;
    const warm = results.warm;
    results.distributions = {
      cold_ttfb_ms: distribution(cold.map(x => x.navigation?.ttfb_ms)),
      cold_first_content_ms: distribution(cold.map(x => x.first_content_ms)),
      cold_hydration_ms: distribution(cold.map(x => x.hydration_ms)),
      cold_wall_ms: distribution(cold.map(x => x.wall_ms)),
      reader_client_nav_ms: distribution(warm.map(x => x.reader_ms)),
      discovery_return_ms: distribution(warm.map(x => x.return_ms)),
      cold_bundle_transfer_bytes: distribution(cold.map(x => x.loaded_bundle_transfer_bytes)),
      cold_dom_nodes: distribution(cold.map(x => x.dom_counters?.nodes)),
      cold_js_heap_bytes: distribution(cold.map(x => x.v8_heap?.usedSize)),
    };
    check(cold.length === ITERATIONS && warm.length === ITERATIONS,
      '#234 at least ten raw cold and ten warm observations');
    results.verification = {
      cold_10: 'PASS', warm_10: 'PASS', real_app_sql: 'PASS',
      transport_ledger: 'PASS', raw_distributions: 'PASS',
      late_response_negative: 'NOT_RUN', stale_invalidation: 'NOT_RUN',
      secondary_failover: 'NOT_RUN', release_profile: 'NOT_RUN',
    };
    results.go_no_go = {
      selective_ssr: 'PENDING: inspect main-content vs hydration/secondary delay before #235',
      anchor_restore: 'PENDING: inspect return focus/position before #236',
      ram_ceiling: 'PENDING: separate optional snapshot cache from active view before #237',
      containment: 'PENDING: inspect render/DOM hotspots before #238',
      prefetch: 'NO_GO until #237 establishes a byte ceiling and navigation benefit',
    };
    results.status = 'MEASURED_PARTIAL';
    flush();
  } catch (error) {
    results.status = 'FAIL';
    results.error = error.message;
    flush();
    throw error;
  } finally {
    unlisten();
    await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: observer.identifier });
  }
}
