// #234: opt-in evidence phase hosted by the existing browser-smoke CDP Page.
// Measurements are lab observations, not field RUM, an SLA, or a timing gate.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { brotliCompressSync, constants as zlibConstants, gzipSync } from 'node:zlib';
import { assertNoHydrationRead, countTransport, recordTransport } from './web-request-ledger.mjs';

export const COMPRESSION_PARAMETERS = Object.freeze({
  gzip: { library: 'node:zlib', level: 6 },
  brotli_dynamic: { library: 'node:zlib', quality: 6 },
  brotli_static: { library: 'node:zlib', quality: 11 },
});

export function measureCompressedBytes(input) {
  const buf = Buffer.isBuffer(input) ? input : Buffer.from(input);
  return {
    raw: buf.length,
    gzip_6: gzipSync(buf, { level: COMPRESSION_PARAMETERS.gzip.level }).length,
    brotli_6: brotliCompressSync(buf, {
      params: { [zlibConstants.BROTLI_PARAM_QUALITY]: COMPRESSION_PARAMETERS.brotli_dynamic.quality },
    }).length,
    brotli_11: brotliCompressSync(buf, {
      params: { [zlibConstants.BROTLI_PARAM_QUALITY]: COMPRESSION_PARAMETERS.brotli_static.quality },
    }).length,
    sha256: createHash('sha256').update(buf).digest('hex'),
  };
}

export function measureBundleArtifacts(pkgDir = 'target/site/pkg') {
  const files = {};
  for (const name of ['vink.css', 'vink_bg.wasm', 'vink.js', 'vink-editor.js']) {
    const file = join(pkgDir, name);
    if (existsSync(file)) files[name] = measureCompressedBytes(readFileSync(file));
  }
  return { compression: COMPRESSION_PARAMETERS, files };
}

import { measureReaderLifecycle, measureUncachedReader, readOwnerCounts } from './web-perf-controls.mjs';

const WAIT_READY = "document.documentElement.dataset.hydrated==='true'";
const LIST = "!!document.querySelector('.vk-public-row__link')";
const READER = "!!document.querySelector('.vk-reader .vk-document')";
const QUERY = '/?q=kiem%20thu';
const ITERATIONS = 10;
// md5('vink-100-work-1')::uuid: the #100 work that carries the 1,000-comment hot thread and,
// by the fixture's publication order, the first filtered discovery row.
const HOT_WORK = 'e4202094-9563-02e5-519d-6f8fa6a1af4c';
// The endpoint's default page is 20 roots, each with up to 3 inline preview replies.
const ROOT_PAGE_SIZE = 20;
const MAX_MOUNTED_COMMENTS = ROOT_PAGE_SIZE * (1 + 3);
const ROOTS = "document.querySelectorAll('#root-list > .vk-discussion-root').length";
const COMMENTS = "document.querySelectorAll('.vk-discussion-comment').length";

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

function newDocumentObserver() {
  const data = window.__vinkPerfBaseline = {
    observer_cpu_ms: 0, observer_callbacks: 0, clicked_at: null, article_at: null,
    hydration_ms: null, meaningful_ms: null, long_tasks: null, layout_shifts: null, interactions: null,
  };
  const measured = fn => (...args) => {
    const start = performance.now();
    fn(...args);
    data.observer_callbacks++;
    data.observer_cpu_ms += performance.now() - start;
  };
  document.addEventListener('click', event => {
    if (event.target.closest?.('.vk-public-row__link')) {
      data.clicked_at = performance.now();
      data.article_at = null;
    }
  }, true);
  const record = measured(() => {
    if (data.clicked_at !== null && data.article_at === null && document.querySelector('.vk-reader .vk-document'))
      data.article_at = performance.now();
    if (data.meaningful_ms === null &&
      document.querySelector('.vk-public-row__link, .vk-reader .vk-document')) {
      data.meaningful_ms = performance.now();
    }
    if (data.hydration_ms === null &&
      document.documentElement?.dataset.hydrated === 'true') {
      data.hydration_ms = performance.now();
    }
  });
  const observe = (type, field, sample, options = {}) => {
    if (!PerformanceObserver.supportedEntryTypes.includes(type)) return;
    new PerformanceObserver(measured(list => {
      for (const entry of list.getEntries()) {
        const row = sample(entry);
        if (row) data[field].push(row);
      }
    })).observe({ type, buffered: true, ...options });
    data[field] = [];
  };
  observe('longtask', 'long_tasks', e => ({ start_ms: e.startTime, duration_ms: e.duration }));
  observe('layout-shift', 'layout_shifts', e => ({
    start_ms: e.startTime, value: e.value, had_recent_input: e.hadRecentInput,
  }));
  observe('event', 'interactions', e => e.interactionId && ({
    id: e.interactionId, name: e.name, start_ms: e.startTime, duration_ms: e.duration,
    processing_ms: e.processingEnd - e.processingStart,
  }), { durationThreshold: 16 });
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
    'observer_cpu_ms:p.observer_cpu_ms,observer_callbacks:p.observer_callbacks,' +
    'click_to_article_ms:p.article_at==null?null:p.article_at-p.clicked_at,' +
    'now_ms:performance.now(),long_tasks:p.long_tasks??null,layout_shifts:p.layout_shifts??null,' +
    'interactions:p.interactions??null,' +
    'loaded_bundle_transfer_bytes:bytes,loaded_resource_count:res.length,' +
    'row_count:document.querySelectorAll(".vk-public-row").length,' +
    'discussion_roots:' + ROOTS + ',' +
    'comment_nodes:document.querySelectorAll(".vk-discussion-comment").length,' +
    'body_characters:document.querySelector(".vk-reader .vk-document")?.textContent.length??null,' +
    'root_font_px:getComputedStyle(document.documentElement).fontSize,' +
    'dpr:devicePixelRatio,viewport:{width:innerWidth,height:innerHeight},' +
    'fonts_status:document.fonts.status,font_family:getComputedStyle(document.body).fontFamily,' +
    'fonts:[...document.fonts].filter(f=>f.status==="loaded").map(f=>f.family)};})()');
  const get = async (method) => { try { return await page.send(method); } catch { return null; } };
  return {
    ...details,
    public_read_owner: await readOwnerCounts(page),
    dom_counters: await get('Memory.getDOMCounters'),
    v8_heap: await get('Runtime.getHeapUsage'),
  };
}

function fixtureDigest() {
  const hash = createHash('sha256');
  for (const path of [
    'apps/vink-web/e2e/fixtures/discovery-publications.sql',
    'apps/vink-web/e2e/fixtures/web-perf-journey.sql',
  ]) hash.update(readFileSync(path));
  return hash.digest('hex');
}

async function measureControls(page, results, phase, origin, check) {
  const controls = results.controls = {};
  phase.current = 'negative-extra-hydration-get';
  // Exercise the same oracle against a real extra list GET. A regression to <= 1 would
  // make this negative control fail, even if normal hydration happens to send none.
  check(await page.eval(`fetch('/api/v1/works${QUERY.slice(1)}', {cache:'no-store'}).then(r=>r.ok)`),
    'extra GET reached the real App');
  try {
    assertNoHydrationRead(results.transport, phase.current, 'public-list');
  } catch (error) {
    controls.extra_hydration_get = { expected_failure: error.message };
  }
  check(controls.extra_hydration_get && countTransport(results.transport, phase.current, 'public-list') === 1,
    'SSR handoff oracle rejects one extra GET');

  // Same-document Back isn't a back_forward SSR seed. Force a full document history traversal
  // (the opt-in Chrome launch disables BFCache), then require one verification for unknown age.
  phase.current = 'history-prepare';
  await page.goto('about:blank');
  await page.goto(origin + QUERY);
  await page.waitFor(WAIT_READY + ' && ' + LIST, 'history source document hydrated');
  const oldDocument = await page.eval('performance.timeOrigin');
  await page.goto(origin + '/tac-pham/' + HOT_WORK);
  await page.waitFor(WAIT_READY + ' && ' + READER, 'history destination document hydrated');
  phase.current = 'history-ssr-age-unknown';
  results.intended_reads.push({ phase: phase.current, reason: 'document_history_revalidate', representation: 'discovery-filtered' });
  await page.eval('history.back()');
  await page.waitFor(`location.pathname==='/' && performance.timeOrigin!==${oldDocument} && ${WAIT_READY} && ${LIST} && performance.getEntriesByType('navigation')[0]?.type==='back_forward'`,
    'history opens a new back_forward document');
  await page.waitFor("document.querySelector('#public-collection')?.dataset.refreshing==='false' && performance.getEntriesByType('resource').some(e=>new URL(e.name).pathname==='/api/v1/works')", 'unknown-age SSR seed is verified');
  check(countTransport(results.transport, phase.current, 'public-list') === 1,
    'a back_forward document verifies its unknown-age list once');
  controls.history = { navigation: 'back_forward', new_document: true };

  phase.current = 'secondary-initial-hold';
  const secondary = '/api/v1/works/' + HOT_WORK + '/discussion/public';
  results.intended_reads.push({ phase: phase.current, reason: 'initial_secondary', representation: 'public-discussion' });
  let releasePause;
  const paused = new Promise(resolve => { releasePause = resolve; });
  const hold = event => {
    if (event.sessionId === page.sessionId && event.method === 'Fetch.requestPaused') releasePause(event.params.requestId);
  };
  page.cdp.listeners.push(hold);
  try {
    await page.send('Fetch.enable', { patterns: [{ urlPattern: origin + secondary + '*', requestStage: 'Request' }] });
    const holdStarted = performance.now();
    await page.click('.vk-public-row__link');
    await page.waitFor(READER + ' && location.pathname.endsWith(' + JSON.stringify(HOT_WORK) + ')',
      'real article mounts while discussion is held');
    let timeout;
    const requestId = await Promise.race([paused, new Promise((_, reject) => {
      timeout = setTimeout(() => reject(new Error('no paused secondary transport')), 10000);
    })]).finally(() => clearTimeout(timeout));
    check(await page.eval(COMMENTS + ' === 0'), 'held public discussion has not mounted prematurely');
    await page.eval("window.__baselinePaper = document.querySelector('#reader-paper')");
    await new Promise(resolve => setTimeout(resolve, 200));
    await page.send('Fetch.fulfillRequest', { requestId, responseCode: 503,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify({ error: { code: 'dependency_unavailable' } })).toString('base64') });
    await page.waitFor("document.querySelector('#discussion-load-retry')?.dataset.state==='enabled'", 'failed secondary has a retry');
    check(await page.eval("document.querySelector('#reader-paper') === window.__baselinePaper && !!document.querySelector('.vk-document')"),
      'failing the secondary transport leaves the authoritative article mounted');
    const heldMs = performance.now() - holdStarted;
    await page.send('Fetch.disable');
    phase.current = 'secondary-manual-retry';
    results.intended_reads.push({ phase: phase.current, reason: 'manual_retry', representation: 'public-discussion' });
    await page.click('#discussion-load-retry');
    await page.waitFor(COMMENTS + ' > 0', 'manual secondary retry reaches real App');
    check(countTransport(results.transport, phase.current, 'public-discussion') === 1 &&
      await page.eval(`${ROOTS} === ${ROOT_PAGE_SIZE} && ${COMMENTS} <= ${MAX_MOUNTED_COMMENTS}`),
      'retry requests and mounts one bounded public discussion page');
    controls.secondary = { injected_status: 503, minimum_hold_ms: 200, click_to_retry_control_ms: heldMs, recovered: true };
  } finally {
    await page.send('Fetch.disable');
    page.cdp.listeners = page.cdp.listeners.filter(listener => listener !== hold);
    await page.eval('delete window.__baselinePaper');
  }
}

export async function webPerfBaseline(page, { args, check }) {
  // The test harness must never be pointed at a deployed or private service.
  if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(args.base)) throw new Error('#234 lab requires loopback scratch App');
  const out = join(dirname(args.state), 'web-perf-baseline.json');
  const phase = { current: 'init' };
  const tool = (name, ...argv) => execFileSync(name, argv, { encoding: 'utf8' }).trim();
  const sourceSha = tool('git', 'rev-parse', 'HEAD');
  if (!['dev', 'release'].includes(args['build-profile'])) throw new Error('Missing xtask build profile');
  const transport = recordTransport(page, args.base, () => phase.current);
  const results = {
    issue: 234, status: 'RUNNING', source_sha: sourceSha,
    dirty: tool('git', 'status', '--porcelain'),
    corpus: { labelled_public_works: 200, comments: 1000, digest_sha256: fixtureDigest(),
      source: 'App/PostgreSQL scratch, canonical format/source public projection, no production user text' },
    configuration: {
      origin: args.base, browser: await page.cdp.send('Browser.getVersion'),
      viewer: 'guest (no local fixture principal)',
      node: process.version, platform: process.platform, arch: process.arch,
      rustc: tool('rustc', '--version'), wasm_bindgen: tool('wasm-bindgen', '--version'),
      build_profile: args['build-profile'],
      wasm_file_bytes: statSync('target/site/pkg/vink_bg.wasm').size,
      bundle_artifacts: measureBundleArtifacts('target/site/pkg'),
      ssr_html_artifacts: {},
      network: 'loopback / no throttling', cpu_throttle_rate: 1,
      cache_cold: 'CDP clearBrowserCache before each full-document navigation; OS/PG process remain warm',
      viewport: '1280x900 CSS px / DPR 1 (see measured values)', text_scale: 'browser default',
      cold_iterations: ITERATIONS, warm_iterations: ITERATIONS,
      instrumentation: 'Page.addScriptToEvaluateOnNewDocument: PerformanceObserver + MutationObserver; scenario opt-in',
    },
    intended_reads: [], transport: transport.rows, cold: [], warm: [],
    verification: {}, limitations: [
      'No field p75; p95 is a lab nearest-rank statistic over 10 iterations.',
      'No browser-process RSS, WASM heap attribution, OS page-cache cold start, production latency or RUM.',
      'The cold route is filtered Explore SSR. Reader measurements here start with warm client navigation.',
      'Client ready times in warm samples include CDP/poll overhead; event samples use Event Timing (>=16 ms).',
      'CDP transport is browser-visible only: server-side SSR SQL reads are not inferred as client GETs.',
      'Owner counts cover READ-02 only. Observer CPU measures callback bodies, excluding browser scheduling, CDP and profiler overhead.',
      'Lifecycle and injected transport cases are separated from measured iterations.',
    ],
  };
  const flush = () => writeFileSync(out, JSON.stringify(results, null, 2));
  const observer = await page.send('Page.addScriptToEvaluateOnNewDocument', {
    source: '(' + newDocumentObserver.toString() + ')()',
  });
  try {
    await page.viewport(1280, 900);
    for (const [label, routePath] of [['discovery_filtered', QUERY], ['reader_hot', '/tac-pham/' + HOT_WORK]]) {
      const response = await fetch(args.base + routePath, { headers: { Accept: 'text/html' } });
      if (response.ok) {
        const html = Buffer.from(await response.arrayBuffer());
        results.configuration.ssr_html_artifacts[label] = {
          route: routePath, status: response.status, ...measureCompressedBytes(html),
        };
      }
    }
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
      const wall = performance.now() - start;
      check(await page.eval("!document.querySelector('meta[name=vink-local-discussion-fixture]')"), 'baseline renders the guest shell');
      const snapshot = await browserSnapshot(page);
      results.cold.push({ iteration: i, wall_ms: wall, ...snapshot });
      // Keep the observation window separate from lab timings. A single client list GET is
      // already surplus: the SSR list is not visible in the CDP ledger as an API request.
      await new Promise(resolve => setTimeout(resolve, 300));
      assertNoHydrationRead(results.transport, phase.current, 'public-list');
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
      // The discussion is a secondary read: snapshot only after the hot thread's first page
      // is mounted, otherwise the bounded-pagination assertion is vacuous (0 nodes).
      await page.waitFor(COMMENTS + ' > 0', 'hot thread first discussion page #' + i, 120000);
      const discussionTime = (await page.eval('performance.now()')) - before;
      const reader = await browserSnapshot(page);
      check(await page.eval('performance.timeOrigin') === timeOrigin,
        '#234 warm Reader uses client navigation without a full-document reload #' + i);
      check(reader.path === '/tac-pham/' + HOT_WORK,
        '#234 warm journey opens the 1,000-comment hot thread #' + i);
      check(reader.body_characters > 0, '#234 real App Reader body stays available #' + i);
      // Only a limited page may be mounted, never all 1,000 comments from the hot thread.
      check(reader.discussion_roots === ROOT_PAGE_SIZE && reader.comment_nodes > 0 && reader.comment_nodes <= MAX_MOUNTED_COMMENTS,
        '#234 hot thread mounts one bounded page (' + reader.comment_nodes + ' nodes) #' + i);
      await page.waitFor("document.querySelector('#comments-mode')?.getAttribute('aria-busy')==='false'",
        'discussion ready #' + i);
      const readyTime = (await page.eval('performance.now()')) - before;
      phase.current = 'return-' + i;
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
      results.warm.push({ iteration: i, started_ms: before, return_started_ms: beforeReturn,
        reader_ms: readerTime, discussion_ms: discussionTime, discussion_control_ms: readyTime,
        return_ms: returnTime, reader, back });
      flush();
    }
    await measureControls(page, results, phase, args.base, check);
    await measureUncachedReader(page, { origin: args.base, query: QUERY,
      secondary: '/api/v1/works/' + HOT_WORK + '/discussion/public', check, phase, results });
    await measureReaderLifecycle(page, { origin: args.base, work: HOT_WORK, results, phase, check });
    const cold = results.cold;
    const warm = results.warm;
    results.distributions = {
      cold_observer_cpu_ms: distribution(cold.map(x => x.observer_cpu_ms)),
      warm_click_to_article_ms: distribution(warm.map(x => x.reader.click_to_article_ms)),
      cold_key_reader_ms: distribution(results.uncached_reader.map(x => x.reader_duration_ms)),
      cold_key_article_ms: distribution(results.uncached_reader.map(x => x.click_to_article_ms)),
      secondary_layout_style_ms: distribution(results.uncached_reader.map(x =>
        x.secondary_window_ms.LayoutDuration + x.secondary_window_ms.RecalcStyleDuration)),
      cold_ttfb_ms: distribution(cold.map(x => x.navigation?.ttfb_ms)),
      cold_first_content_ms: distribution(cold.map(x => x.first_content_ms)),
      cold_hydration_ms: distribution(cold.map(x => x.hydration_ms)),
      cold_wall_ms: distribution(cold.map(x => x.wall_ms)),
      reader_client_nav_ms: distribution(warm.map(x => x.reader_ms)),
      reader_discussion_page_ms: distribution(warm.map(x => x.discussion_ms)),
      reader_discussion_control_ms: distribution(warm.map(x => x.discussion_control_ms)),
      warm_mounted_comments: distribution(warm.map(x => x.reader.comment_nodes)),
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
      hydration_duplicate_control: 'PASS', secondary_failover: 'PASS',
      history_ssr_unknown_age: 'PASS', stale_manual_offline_late: 'PASS',
      release_profile: args['build-profile'] === 'release' ? 'PASS' : 'NOT_RUN',
    };
    results.interpretation_owner = 'docs/development/web-perf-baseline.md';
    results.status = 'MEASURED';
    flush();
  } catch (error) {
    results.status = 'FAIL';
    results.error = error.message;
    flush();
    throw error;
  } finally {
    transport.stop();
    await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: observer.identifier });
  }
}
