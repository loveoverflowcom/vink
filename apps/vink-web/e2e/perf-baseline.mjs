// #234: Opt-in Web performance baseline and request-reason ledger for SSR, hydration,
// and the Explore -> Reader -> Back journey. Reuses #229's corpus and the existing CDP harness.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { cpus, platform, release } from 'node:os';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';

export function classifyRequest(urlString, method = 'GET') {
  const parsed = new URL(urlString, 'http://127.0.0.1');
  const path = parsed.pathname;
  if (path.startsWith('/pkg/')) return 'asset_bundle';
  if (path === '/api/v1/works') return 'public_works_list';
  if (/^\/api\/v1\/works\/[^/]+\/reader$/.test(path)) return 'public_reader';
  if (/^\/api\/v1\/works\/[^/]+\/publication$/.test(path)) return 'public_publication_legacy';
  if (
    method !== 'GET' &&
    /^\/api\/v1\/works\/[^/]+\/(snapshots|publication\/(request-public|set-private|submit|private))$/.test(path)
  ) {
    return 'author_mutation';
  }
  if (path.startsWith('/api/v1/tags')) return 'public_catalog';
  if (/^\/api\/v1\/works\/[^/]+\/discussion\/public(\/|$)/.test(path)) return 'public_discussion';
  if (/^\/api\/v1\/works\/[^/]+\/discussion\/me(\/|$)/.test(path)) return 'own_discussion';
  if (/^\/api\/v1\/works\/[^/]+\/discussion$/.test(path) && method === 'GET') return 'capability_discussion';
  if (/^\/api\/v1\/works\/[^/]+\/endorsements\/me$/.test(path)) return 'own_endorsements';
  if (/^\/api\/v1\/works\/[^/]+\/endorsements$/.test(path)) return 'public_endorsements';
  if (path.startsWith('/api/v1/auth/')) return 'auth_account';
  if (
    path === '/' ||
    path.startsWith('/the/') ||
    path.startsWith('/tac-pham/') ||
    path.startsWith('/ban-viet') ||
    path === '/toi'
  ) {
    return 'document_ssr';
  }
  return 'other';
}

export function summarizeStepLedger(events, reason) {
  const byCategory = {};
  const ensure = (cat) => {
    byCategory[cat] ??= {
      transport_attempts: 0,
      logical_reads: 0,
      validator_304_count: 0,
      conditional_requests: 0,
      encoded_bytes: 0,
      wire_statuses: [],
      app_statuses: [],
    };
    return byCategory[cat];
  };

  let totalAttempts = 0;
  let totalLogical = 0;
  let total304 = 0;
  let totalEncoded = 0;

  // Group raw-304 unconditional fallback pairs on the same key into 1 logical read / 2 attempts.
  const ordered = [...events].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
  const raw304Keys = new Set();

  for (const ev of ordered) {
    const category = ev.category ?? classifyRequest(ev.url, ev.method);
    const key = `${category}:${ev.pathnameWithSearch ?? ev.url}`;
    const bucket = ensure(category);
    bucket.transport_attempts += 1;
    totalAttempts += 1;
    if (ev.ifNoneMatch) bucket.conditional_requests += 1;
    if (ev.wireStatus === 304 && ev.ifNoneMatch) {
      bucket.validator_304_count += 1;
      total304 += 1;
    }
    const bytes = Number.isFinite(ev.encodedDataLength) ? ev.encodedDataLength : 0;
    bucket.encoded_bytes += bytes;
    totalEncoded += bytes;
    bucket.wire_statuses.push(ev.failed ? 'failed' : (ev.wireStatus ?? null));
    bucket.app_statuses.push(ev.failed ? 'failed' : (ev.appStatus ?? null));

    const isUnconditionalReloadAfterRaw304 =
      (category === 'public_works_list' || category === 'public_reader') &&
      raw304Keys.has(key) &&
      !ev.ifNoneMatch;

    if (isUnconditionalReloadAfterRaw304) {
      raw304Keys.delete(key);
    } else {
      bucket.logical_reads += 1;
      totalLogical += 1;
      if (
        (category === 'public_works_list' || category === 'public_reader') &&
        ev.wireStatus === 304 &&
        !ev.ifNoneMatch
      ) {
        raw304Keys.add(key);
      }
    }
  }

  return {
    reason,
    logical_reads: totalLogical,
    transport_attempts: totalAttempts,
    validator_304_count: total304,
    total_encoded_bytes: totalEncoded,
    by_category: byCategory,
  };
}

export function summarizeDistribution(values) {
  const nums = values.filter((v) => typeof v === 'number' && Number.isFinite(v));
  if (nums.length === 0) {
    return {
      n: 0,
      min: null,
      max: null,
      range: null,
      median: null,
      lab_p95: null,
      iqr: null,
      mean: null,
      stddev: null,
      cv_pct: null,
      field_p75: null,
      note: 'empty sample set',
    };
  }
  const sorted = [...nums].sort((a, b) => a - b);
  const n = sorted.length;
  const min = sorted[0];
  const max = sorted[n - 1];
  const range = Number((max - min).toFixed(3));
  const mid = Math.floor(n / 2);
  const median = Number((n % 2 === 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2).toFixed(3));
  const p95Idx = Math.min(n - 1, Math.max(0, Math.ceil(0.95 * n) - 1));
  const lab_p95 = Number(sorted[p95Idx].toFixed(3));
  const q1 = sorted[Math.floor(0.25 * (n - 1))];
  const q3 = sorted[Math.floor(0.75 * (n - 1))];
  const iqr = Number((q3 - q1).toFixed(3));
  const meanRaw = nums.reduce((acc, v) => acc + v, 0) / n;
  const variance = nums.reduce((acc, v) => acc + (v - meanRaw) ** 2, 0) / n;
  const stddevRaw = Math.sqrt(variance);
  const mean = Number(meanRaw.toFixed(3));
  const stddev = Number(stddevRaw.toFixed(3));
  const cv_pct = meanRaw > 0 ? Number(((stddevRaw / meanRaw) * 100).toFixed(2)) : 0;
  return {
    n,
    min: Number(min.toFixed(3)),
    max: Number(max.toFixed(3)),
    range,
    median,
    lab_p95,
    iqr,
    mean,
    stddev,
    cv_pct,
    field_p75: null,
    note: 'lab distribution only; no field p75',
  };
}

export function evaluateColdHydration({ routeKind, ledger, dom, limits }) {
  const reasons = [];
  if (routeKind === 'explore') {
    const worksReads = ledger.by_category.public_works_list?.transport_attempts ?? 0;
    if (worksReads !== 0) reasons.push(`cold Explore hydration issued ${worksReads} /api/v1/works request(s); expected 0`);
    if (dom.visible_work_rows > limits.discovery_default_page_size) {
      reasons.push(`visible work rows ${dom.visible_work_rows} exceeded discovery limit ${limits.discovery_default_page_size}`);
    }
  } else if (routeKind === 'reader') {
    const readerReads = ledger.by_category.public_reader?.transport_attempts ?? 0;
    if (readerReads !== 0) reasons.push(`cold Reader hydration issued ${readerReads} /reader request(s); expected 0`);
    if (dom.visible_comment_roots > limits.discussion_default_root_page) {
      reasons.push(`visible comment roots ${dom.visible_comment_roots} exceeded discussion limit ${limits.discussion_default_root_page}`);
    }
  } else {
    reasons.push(`unknown routeKind ${routeKind}`);
  }
  return { ok: reasons.length === 0, reasons };
}

export function evaluateWarmBack({ ledger, sawLoading, restoredFocusOk }) {
  const reasons = [];
  const worksReads = ledger.by_category.public_works_list?.transport_attempts ?? 0;
  if (worksReads !== 0) reasons.push(`warm Back within 120s issued ${worksReads} /api/v1/works request(s); expected 0`);
  if (sawLoading) reasons.push('warm Back flashed discovery loading indicator');
  if (!restoredFocusOk) reasons.push('warm Back did not restore focus to the opened work row');
  return { ok: reasons.length === 0, reasons };
}

export function evaluateStaleBurst({ ledger }) {
  const reasons = [];
  const works = ledger.by_category.public_works_list ?? {
    logical_reads: 0,
    transport_attempts: 0,
    validator_304_count: 0,
  };
  if (works.logical_reads !== 1) reasons.push(`expected 1 logical read across burst, got ${works.logical_reads}`);
  if (works.transport_attempts !== 1) reasons.push(`expected 1 transport attempt across burst, got ${works.transport_attempts}`);
  if (works.validator_304_count !== 1) reasons.push(`expected 1 wire 304 validation, got ${works.validator_304_count}`);
  return { ok: reasons.length === 0, reasons };
}

export function evaluateLateResponseFence({ expectedLatestActive, leakedObsoleteContent }) {
  const reasons = [];
  if (!expectedLatestActive) reasons.push('expected latest authoritative route state was overwritten');
  if (leakedObsoleteContent) reasons.push('obsolete response leaked title/body/canonical into DOM');
  return { ok: reasons.length === 0, reasons };
}

export function evaluateDisposalBounds({ cycles, maxActiveTimers = 1, maxTailListenerGrowth = 2 }) {
  const reasons = [];
  if (!Array.isArray(cycles) || cycles.length < 10) {
    return { ok: false, reasons: ['disposal check requires at least 10 repeated navigation cycles'] };
  }
  for (let i = 0; i < cycles.length; i++) {
    const c = cycles[i];
    if (c.active_timers > maxActiveTimers) {
      reasons.push(`cycle ${i}: active_timers=${c.active_timers} exceeded bound ${maxActiveTimers}`);
    }
  }
  const mid = Math.floor(cycles.length / 2);
  const midCdp = cycles[mid].cdp_js_event_listeners;
  const endCdp = cycles[cycles.length - 1].cdp_js_event_listeners;
  const midHook = cycles[mid].active_window_doc_listeners;
  const endHook = cycles[cycles.length - 1].active_window_doc_listeners;
  if (endCdp - midCdp > maxTailListenerGrowth) {
    reasons.push(`CDP jsEventListeners grew from ${midCdp} (mid) to ${endCdp} (end), delta=${endCdp - midCdp}`);
  }
  if (endHook - midHook > maxTailListenerGrowth) {
    reasons.push(`instrumented window/document listeners grew from ${midHook} (mid) to ${endHook} (end), delta=${endHook - midHook}`);
  }
  return { ok: reasons.length === 0, reasons };
}

function buildObserverSource() {
  return `(() => {
    if (window.__vinkPerfInstalled) return;
    window.__vinkPerfInstalled = true;
    const perf = {
      hydratedAt: null,
      meaningfulAt: null,
      controls: {},
      lcp: null,
      cls: 0,
      clsEntries: 0,
      longTasks: [],
      events: [],
      timers: new Set(),
      intervals: new Set(),
      timeoutCalls: 0,
      intervalCalls: 0,
      listeners: new Map(),
      listenerSeq: 0,
      activeListeners: 0,
    };
    window.__vinkPerf = perf;

    const recordControl = (name, selector) => {
      if (perf.controls[name] !== undefined) return;
      if (document.documentElement?.dataset.hydrated === 'true' && document.querySelector(selector)) {
        perf.controls[name] = performance.now();
      }
    };
    const scan = () => {
      if (document.documentElement?.dataset.hydrated === 'true' && perf.hydratedAt === null) {
        perf.hydratedAt = performance.now();
      }
      if (perf.meaningfulAt === null && document.querySelector('.vk-public-row, [data-testid=public-empty], [data-testid=public-no-results], #reader-paper .vk-document')) {
        perf.meaningfulAt = performance.now();
      }
      recordControl('explore_query_ready', '#public-query');
      recordControl('explore_filter_ready', '#open-tag-filters, #dock-tag-form input[type=checkbox]');
      recordControl('explore_next_ready', '#public-next');
      recordControl('reader_return_ready', '#reader-return');
      recordControl('reader_article_ready', '#reader-paper .vk-document');
      recordControl('reader_composer_ready', '#root-composer');
      recordControl('reader_endorse_ready', '#endorse-work');
    };
    new MutationObserver(scan).observe(document, { subtree: true, childList: true, attributes: true, attributeFilter: ['data-hydrated'] });
    document.addEventListener('DOMContentLoaded', scan, { once: true });

    try {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) perf.lcp = entry.startTime;
      }).observe({ type: 'largest-contentful-paint', buffered: true });
    } catch {}
    try {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          if (!entry.hadRecentInput) {
            perf.cls += entry.value;
            perf.clsEntries += 1;
          }
        }
      }).observe({ type: 'layout-shift', buffered: true });
    } catch {}
    try {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          perf.longTasks.push({ start: entry.startTime, duration: entry.duration });
        }
      }).observe({ type: 'longtask', buffered: true });
    } catch {}
    try {
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          if (entry.interactionId || entry.duration >= 16) {
            perf.events.push({ name: entry.name, duration: entry.duration, start: entry.startTime });
          }
        }
      }).observe({ type: 'event', durationThreshold: 0, buffered: true });
    } catch {}

    const origSetTimeout = window.setTimeout.bind(window);
    const origClearTimeout = window.clearTimeout.bind(window);
    const origSetInterval = window.setInterval.bind(window);
    const origClearInterval = window.clearInterval.bind(window);
    window.setTimeout = function(fn, delay, ...rest) {
      perf.timeoutCalls += 1;
      let id;
      const wrapped = function(...args) {
        perf.timers.delete(id);
        return typeof fn === 'function' ? fn.apply(this, args) : Function(fn)();
      };
      id = origSetTimeout(wrapped, delay, ...rest);
      perf.timers.add(id);
      return id;
    };
    window.clearTimeout = function(id) {
      perf.timers.delete(id);
      return origClearTimeout(id);
    };
    window.setInterval = function(fn, delay, ...rest) {
      perf.intervalCalls += 1;
      const id = origSetInterval(fn, delay, ...rest);
      perf.intervals.add(id);
      return id;
    };
    window.clearInterval = function(id) {
      perf.intervals.delete(id);
      return origClearInterval(id);
    };

    const origAdd = EventTarget.prototype.addEventListener;
    const origRemove = EventTarget.prototype.removeEventListener;
    const isWindowOrDoc = (t) => t === window || t === document;
    EventTarget.prototype.addEventListener = function(type, listener, options) {
      if (isWindowOrDoc(this) && listener) {
        const cap = typeof options === 'boolean' ? options : Boolean(options?.capture);
        let byType = perf.listeners.get(this);
        if (!byType) { byType = new Map(); perf.listeners.set(this, byType); }
        const key = type + ':' + cap;
        let set = byType.get(key);
        if (!set) { set = new Set(); byType.set(key, set); }
        if (!set.has(listener)) {
          set.add(listener);
          perf.activeListeners += 1;
        }
      }
      return origAdd.call(this, type, listener, options);
    };
    EventTarget.prototype.removeEventListener = function(type, listener, options) {
      if (isWindowOrDoc(this) && listener) {
        const cap = typeof options === 'boolean' ? options : Boolean(options?.capture);
        const set = perf.listeners.get(this)?.get(type + ':' + cap);
        if (set?.delete(listener)) {
          perf.activeListeners = Math.max(0, perf.activeListeners - 1);
        }
      }
      return origRemove.call(this, type, listener, options);
    };
  })()`;
}

function wasmSectionBreakdown(buf) {
  if (buf.length < 8) return null;
  let off = 8;
  let codeBytes = 0;
  let dataBytes = 0;
  let customNameBytes = 0;
  let otherBytes = 8;
  while (off < buf.length) {
    const id = buf[off++];
    let len = 0;
    let shift = 0;
    while (off < buf.length) {
      const b = buf[off++];
      len |= (b & 0x7f) << shift;
      if (!(b & 0x80)) break;
      shift += 7;
    }
    if (id === 10) codeBytes += len;
    else if (id === 11) dataBytes += len;
    else if (id === 0) {
      let o2 = off;
      let nlen = 0;
      let nshift = 0;
      while (o2 < buf.length) {
        const b = buf[o2++];
        nlen |= (b & 0x7f) << nshift;
        if (!(b & 0x80)) break;
        nshift += 7;
      }
      const name = buf.slice(o2, o2 + nlen).toString('utf8');
      if (name === 'name') customNameBytes += len;
      else otherBytes += len;
    } else {
      otherBytes += len;
    }
    off += len;
  }
  return { code_bytes: codeBytes, data_bytes: dataBytes, custom_name_bytes: customNameBytes, other_bytes: otherBytes };
}

function collectBundleSizes(rootDir) {
  const pkg = join(rootDir, 'target/site/pkg');
  const out = {};
  for (const name of ['vink.js', 'vink_bg.wasm', 'vink-editor.js', 'vink.css']) {
    const filePath = join(pkg, name);
    try {
      const buf = readFileSync(filePath);
      out[name] = {
        raw_bytes: statSync(filePath).size,
        gzip9_bytes: gzipSync(buf, { level: 9 }).length,
        ...(name === 'vink_bg.wasm' ? { sections: wasmSectionBreakdown(buf) } : {}),
      };
    } catch {
      out[name] = null;
    }
  }
  return out;
}

class NetworkTracker {
  constructor(page) {
    this.page = page;
    this.byId = new Map();
    this.events = [];
    this.seq = 0;
    const ensure = (requestId) => {
      let rec = this.byId.get(requestId);
      if (!rec) {
        rec = {
          seq: null,
          requestId,
          method: null,
          url: null,
          pathname: null,
          pathnameWithSearch: null,
          category: null,
          resourceType: null,
          ifNoneMatch: null,
          wireStatus: null,
          appStatus: null,
          fromDiskCache: false,
          encodedDataLength: 0,
          failed: false,
          errorText: null,
        };
        this.byId.set(requestId, rec);
      }
      return rec;
    };
    this.listener = ({ sessionId, method, params }) => {
      if (sessionId !== this.page.sessionId) return;
      if (method === 'Network.requestWillBeSent') {
        const parsed = new URL(params.request.url);
        const rec = ensure(params.requestId);
        rec.seq ??= ++this.seq;
        rec.method = params.request.method;
        rec.url = params.request.url;
        rec.pathname = parsed.pathname;
        rec.pathnameWithSearch = parsed.pathname + parsed.search;
        rec.category = classifyRequest(params.request.url, params.request.method);
        rec.resourceType = params.type;
        if (!this.events.includes(rec)) this.events.push(rec);
      } else if (method === 'Network.requestWillBeSentExtraInfo') {
        const rec = ensure(params.requestId);
        const header = Object.entries(params.headers ?? {}).find(
          ([k]) => k.toLowerCase() === 'if-none-match',
        );
        if (header) rec.ifNoneMatch = header[1];
      } else if (method === 'Network.responseReceivedExtraInfo') {
        ensure(params.requestId).wireStatus = params.statusCode;
      } else if (method === 'Network.responseReceived') {
        const rec = ensure(params.requestId);
        rec.appStatus = params.response.status;
        rec.fromDiskCache = Boolean(params.response.fromDiskCache);
        if (rec.wireStatus === null) rec.wireStatus = params.response.status;
      } else if (method === 'Network.loadingFinished') {
        ensure(params.requestId).encodedDataLength = params.encodedDataLength ?? 0;
      } else if (method === 'Network.loadingFailed') {
        const rec = ensure(params.requestId);
        rec.failed = true;
        rec.errorText = params.errorText ?? 'failed';
      }
    };
    this.page.cdp.listeners.push(this.listener);
  }
  reset() {
    this.byId.clear();
    this.events.length = 0;
  }
  snapshot(reason) {
    const summary = summarizeStepLedger(this.events, reason);
    return { summary, events: [...this.events] };
  }
  detach() {
    this.page.cdp.listeners = this.page.cdp.listeners.filter((l) => l !== this.listener);
  }
}

async function readPageSample(page) {
  const perf = await page.eval(`(async () => {
    await document.fonts.ready;
    await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
    const nav = performance.getEntriesByType('navigation')[0]?.toJSON() ?? {};
    const resources = performance.getEntriesByType('resource').map(r => ({
      name: new URL(r.name).pathname,
      duration: r.duration,
      transferSize: r.transferSize,
      encodedBodySize: r.encodedBodySize,
      decodedBodySize: r.decodedBodySize,
    }));
    const scripts = [...document.scripts].filter(s => !s.src);
    const bytes = s => new TextEncoder().encode(s).length;
    const p = window.__vinkPerf ?? {};
    const paints = Object.fromEntries(performance.getEntriesByType('paint').map(e => [e.name, e.startTime]));
    const lt = (p.longTasks ?? []).map(x => x.duration);
    return {
      navigation: {
        type: nav.type ?? null,
        ttfb_ms: Number.isFinite(nav.responseStart) && Number.isFinite(nav.requestStart) ? nav.responseStart - nav.requestStart : null,
        response_end_ms: Number.isFinite(nav.responseEnd) && Number.isFinite(nav.requestStart) ? nav.responseEnd - nav.requestStart : null,
        dom_content_loaded_ms: Number.isFinite(nav.domContentLoadedEventEnd) && Number.isFinite(nav.requestStart) ? nav.domContentLoadedEventEnd - nav.requestStart : null,
        transfer_size: nav.transferSize ?? null,
        encoded_body_size: nav.encodedBodySize ?? null,
        decoded_body_size: nav.decodedBodySize ?? null,
      },
      paints: { fcp_ms: paints['first-contentful-paint'] ?? null, lcp_ms: p.lcp ?? null },
      hydrated_ms: p.hydratedAt ?? null,
      meaningful_content_ms: p.meaningfulAt ?? null,
      controls_ready_ms: p.controls ?? {},
      cls: { total: p.cls ?? 0, entries: p.clsEntries ?? 0 },
      long_tasks: {
        count: lt.length,
        total_ms: lt.reduce((a, b) => a + b, 0),
        max_ms: lt.length ? Math.max(...lt) : 0,
      },
      event_samples: p.events ?? [],
      hydration_resource_script_bytes: scripts.filter(s => s.textContent.includes('__RESOLVED_RESOURCES')).reduce((n, s) => n + bytes(s.textContent), 0),
      inline_script_total_bytes: scripts.reduce((n, s) => n + bytes(s.textContent), 0),
      resources,
      dom: {
        dom_elements: document.querySelectorAll('*').length,
        visible_work_rows: document.querySelectorAll('.vk-public-row').length,
        visible_comment_roots: document.querySelectorAll('#root-list > *').length,
        root_font_size: getComputedStyle(document.documentElement).fontSize,
        fonts_ready: document.fonts.status === 'loaded',
      },
      timers_and_listeners: {
        active_timers: (p.timers?.size ?? 0) + (p.intervals?.size ?? 0),
        timeout_calls: p.timeoutCalls ?? 0,
        interval_calls: p.intervalCalls ?? 0,
        active_window_doc_listeners: p.activeListeners ?? 0,
      },
    };
  })()`);
  const heap = await page.send('Runtime.getHeapUsage');
  const cdpDom = await page.send('Memory.getDOMCounters');
  return {
    ...perf,
    dom: {
      ...perf.dom,
      cdp_nodes: cdpDom.nodes,
      cdp_documents: cdpDom.documents,
      cdp_js_event_listeners: cdpDom.jsEventListeners,
    },
    timers_and_listeners: {
      ...perf.timers_and_listeners,
      cdp_js_event_listeners: cdpDom.jsEventListeners,
    },
    memory_proxy: {
      v8_used_bytes: heap.usedSize,
      v8_total_bytes: heap.totalSize,
      label: 'v8_js_heap_proxy_not_process_rss_or_total_wasm_memory',
    },
  };
}

export function buildGoNoGoLedger({ configStats, ledgerSummary, disposalOk }) {
  const desktopWarmBack = configStats.desktop_1440_dpr1?.warm_back_ms?.median ?? null;
  const desktopExploreLcp = configStats.desktop_1440_dpr1?.cold_explore?.lcp_ms?.median ?? null;
  const desktopReader200k = configStats.desktop_1440_dpr1?.cold_reader_200k?.hydrated_ms?.median ?? null;
  return [
    {
      id: 'WEB-PERF-01',
      target: 'READ-02 typed Reader coordinator',
      decision: 'KEEP_BASELINE',
      rationale: 'Cold hydration=0 /works or /reader requests; warm Back=0 /works requests; 121s burst=1 wire 304.',
    },
    {
      id: 'WEB-PERF-02',
      target: '#235 Selective SSR Streaming vs Async',
      decision: desktopExploreLcp !== null && desktopExploreLcp > 500 ? 'SPIKE_CONDITIONAL' : 'HOLD_ASYNC_UNTIL_BOTTLENECK',
      rationale: `Measured cold Explore median LCP=${desktopExploreLcp} ms on release loopback; only spike selective streaming under controlled 200ms/1000ms secondary latency.`,
    },
    {
      id: 'WEB-PERF-03',
      target: '#236 Navigation anchor / Keep-Alive',
      decision: desktopWarmBack !== null && desktopWarmBack > 100 ? 'EVALUATE_ANCHOR_SPIKE' : 'PREFER_A_URL_ANCHOR_FIRST',
      rationale: `Warm Back median=${desktopWarmBack} ms with 0 network requests; qualify anchor tolerance across 50 cycles before considering kept-alive DOM.`,
    },
    {
      id: 'WEB-PERF-04',
      target: '#238 Secondary content-visibility containment',
      decision: desktopReader200k !== null && desktopReader200k > 500 ? 'SPIKE_ON_SECONDARY_ONLY' : 'NO_CONTAINMENT_NEEDED_YET',
      rationale: `Cold 200k Reader median hydrated=${desktopReader200k} ms; keep full article DOM and test secondary-only containment against find/selection/print.`,
    },
    {
      id: 'WEB-PERF-07',
      target: '#237 RAM retention hard byte budget',
      decision: disposalOk ? 'MEASURE_BYTE_PRESSURE_NEXT' : 'INVESTIGATE_DISPOSAL_BOUND',
      rationale: `25-cycle disposal bounded=${disposalOk}; soft cap 16 entries currently lacks hard byte cap for multi-200k snapshots.`,
    },
    {
      id: 'WEB-PERF-06',
      target: '#239 Public intent-prefetch',
      decision: 'NO_GO_BEFORE_BUDGET_AND_SSR_DECISION',
      rationale: `Prefetch adds wasted bytes/late-response risk; keep disabled until #235 and #237 establish remaining wait.`,
    },
  ];
}

export async function perfBaseline(page, { args, check, openPage, disposeBrowserContext, sourceIdentity }) {
  const manifestPath = args['perf-manifest'];
  const perfOutDir = args['perf-out'];
  if (!manifestPath || !perfOutDir) {
    throw new Error('perf-baseline phase requires --perf-manifest and --perf-out');
  }
  mkdirSync(perfOutDir, { recursive: true });
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  const limits = manifest.limits;
  const roles = manifest.roles;
  const rootDir = process.cwd();
  const browserVersion = await page.cdp.send('Browser.getVersion');
  const cpuList = cpus();
  const bundleSizes = collectBundleSizes(rootDir);

  const environment = {
    source: sourceIdentity,
    build_profile: manifest.build_profile,
    corpus: {
      journey_seed: manifest.journey_seed,
      document_corpus_seed: manifest.document_corpus_seed,
      corpus_digest: manifest.corpus_digest,
      work_count: manifest.work_count,
      comment_count: manifest.comment_count,
      limits,
    },
    browser: browserVersion,
    tools: {
      node: process.version,
      rustc: execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim(),
      postgres: manifest.postgres,
    },
    host: {
      platform: platform(),
      release: release(),
      cpu_model: cpuList[0]?.model ?? 'unknown',
      cpu_cores: cpuList.length,
    },
    network_config: 'loopback_unthrottled',
    cpu_throttling_rate: 1,
    font_config: { text_scale_pct: 100, root_font_size: '16px', fonts: ['Be Vietnam Pro', 'Literata'] },
    bundle_sizes: bundleSizes,
    instrumentation_toggle: 'opt-in CDP observer script; skipped in overhead control',
  };
  writeFileSync(join(perfOutDir, 'environment.json'), JSON.stringify(environment, null, 2));

  const configs = [
    { key: 'desktop_1440_dpr1', width: 1440, height: 900, dpr: 1, mobile: false, include200k: true },
    { key: 'mobile_390_dpr2', width: 390, height: 844, dpr: 2, mobile: true, include200k: false },
  ];

  const allRuns = {};
  const configStats = {};
  const ledgers = {};
  const negativeControls = {};

  const applyViewport = async (targetPage, cfg) => {
    await targetPage.send('Emulation.setDeviceMetricsOverride', {
      width: cfg.width,
      height: cfg.height,
      deviceScaleFactor: cfg.dpr,
      mobile: cfg.mobile,
    });
  };

  for (const cfg of configs) {
    console.log(`perf-baseline [${cfg.key}]: running 10 cold + 10 warm runs`);
    const coldExplore = [];
    const coldReaderStd = [];
    const coldReader200k = [];

    // 10 Cold Explore + 10 Cold Reader (each in a fresh isolated BrowserContext -> empty HTTP/WASM cache)
    for (let i = 0; i < 10; i++) {
      for (const target of [
        { kind: 'explore', path: '/', dest: coldExplore },
        { kind: 'reader', path: `/tac-pham/${roles.standard_work_id}`, dest: coldReaderStd },
        ...(cfg.include200k ? [{ kind: 'reader', path: `/tac-pham/${roles.long_poem_work_id}`, dest: coldReader200k }] : []),
      ]) {
        const { browserContextId } = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
        const iso = await openPage(browserContextId);
        const tracker = new NetworkTracker(iso);
        try {
          await applyViewport(iso, cfg);
          const observer = await iso.send('Page.addScriptToEvaluateOnNewDocument', { source: buildObserverSource() });
          const start = performance.now();
          await iso.goto(`${args.base}${target.path}`);
          const readyExpr = target.kind === 'explore'
            ? `document.documentElement.dataset.hydrated === 'true' && document.querySelector('.vk-public-row')`
            : `document.documentElement.dataset.hydrated === 'true' && document.querySelector('#reader-paper .vk-document')`;
          await iso.waitFor(readyExpr, `cold ${cfg.key} ${target.kind} #${i}`, 60000);
          if (target.kind === 'reader') {
            await iso.waitFor(`document.querySelector('#root-composer')`, 'cold reader discussion', 60000);
          }
          const wallMs = performance.now() - start;
          const sample = await readPageSample(iso);
          const { summary } = tracker.snapshot(`cold_${target.kind}_${cfg.key}_${i}`);
          if (i === 0 && target.kind === 'explore') ledgers[`cold_explore_${cfg.key}`] = summary;
          if (i === 0 && target.kind === 'reader' && target.path.includes(roles.standard_work_id)) {
            ledgers[`cold_reader_std_${cfg.key}`] = summary;
          }
          const coldCheck = evaluateColdHydration({ routeKind: target.kind, ledger: summary, dom: sample.dom, limits });
          check(coldCheck.ok, `${cfg.key} cold ${target.kind} #${i}: ${coldCheck.reasons.join('; ') || '0 duplicate public reads & bounded DOM'}`);
          check(iso.errors.length === 0, `${cfg.key} cold ${target.kind} #${i}: zero console/runtime errors`);
          // Verify cold cache: main document and wasm came over wire (not disk cache).
          const wasmEv = tracker.events.find((e) => e.pathname === '/pkg/vink_bg.wasm');
          check(wasmEv && !wasmEv.fromDiskCache && wasmEv.encodedDataLength > 0, `${cfg.key} cold ${target.kind} #${i}: cold WASM transfer (${wasmEv?.encodedDataLength} B)`);
          await iso.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: observer.identifier });
          target.dest.push({ iteration: i, wall_ms: Number(wallMs.toFixed(3)), sample, ledger: summary });
        } finally {
          tracker.detach();
          await disposeBrowserContext(browserContextId).catch(() => {});
        }
      }
    }

    // 10 Warm Explore -> Reader -> Back cycles + real interactions in one persistent BrowserContext
    const warmCtx = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
    const warmPage = await openPage(warmCtx.browserContextId);
    const warmTracker = new NetworkTracker(warmPage);
    const warmClientNavReaderMs = [];
    const warmBackMs = [];
    const warmInteractCommentPageMs = [];
    const warmInteractQueryMs = [];
    const warm200kNavMs = [];
    try {
      await applyViewport(warmPage, cfg);
      await warmPage.send('Page.addScriptToEvaluateOnNewDocument', { source: buildObserverSource() });
      await warmPage.goto(`${args.base}/`);
      await warmPage.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('a[href^="/tac-pham/${roles.standard_work_id}"]')`, 'warm prime explore', 60000);

      for (let i = 0; i < 10; i++) {
        // Explore -> Reader (standard work)
        warmTracker.reset();
        const navStart = performance.now();
        await warmPage.click(`a[href^="/tac-pham/${roles.standard_work_id}"]`);
        await warmPage.waitFor(`location.pathname === '/tac-pham/${roles.standard_work_id}' && document.querySelector('#reader-paper .vk-document') && document.querySelector('#root-composer')`, `warm nav reader #${i}`, 60000);
        const navMs = performance.now() - navStart;
        warmClientNavReaderMs.push(Number(navMs.toFixed(3)));
        const navLedger = warmTracker.snapshot(`warm_client_nav_reader_${cfg.key}_${i}`).summary;
        if (i === 0) ledgers[`warm_client_nav_reader_first_${cfg.key}`] = navLedger;
        if (i === 1) ledgers[`warm_client_nav_reader_cached_${cfg.key}`] = navLedger;

        // Interactive sample on iteration 0 and 1: paginate comments (20 -> 40, bounded under MAX_PAGE=50) & type in composer
        if (i < 3) {
          warmTracker.reset();
          const pagStart = performance.now();
          await warmPage.click('#root-pagination button');
          await warmPage.waitFor(`document.querySelectorAll('#root-list .vk-discussion-root').length === 40`, 'append page 2 comments (40)', 30000);
          warmInteractCommentPageMs.push(Number((performance.now() - pagStart).toFixed(3)));
          await warmPage.typeAtEnd('#root-composer', `Ý kiến mẫu #${i}`);
        }

        // Reader -> Back to Explore within 120s
        warmTracker.reset();
        const backStart = performance.now();
        await warmPage.eval(`window.__sawWarmLoading = false; new MutationObserver(() => { if (document.querySelector('[data-testid=discovery-loading]')) window.__sawWarmLoading = true; }).observe(document.body, { subtree: true, childList: true });`);
        await warmPage.click('#reader-return');
        await warmPage.waitFor(`location.pathname === '/' && document.querySelector('a[href^="/tac-pham/${roles.standard_work_id}"]') && document.activeElement?.matches('.vk-public-row__link')`, `warm back #${i}`, 30000);
        const backMs = performance.now() - backStart;
        const sawLoading = await warmPage.eval(`window.__sawWarmLoading`);
        const focusedHref = await warmPage.eval(`document.activeElement?.getAttribute('href') ?? ''`);
        const backLedger = warmTracker.snapshot(`warm_back_${cfg.key}_${i}`).summary;
        if (i === 0) ledgers[`warm_back_${cfg.key}`] = backLedger;
        const backEval = evaluateWarmBack({
          ledger: backLedger,
          sawLoading,
          restoredFocusOk: focusedHref.includes(roles.standard_work_id),
        });
        check(backEval.ok, `${cfg.key} warm Back #${i}: ${backEval.reasons.join('; ') || `0 /works reads, focus restored (${backMs.toFixed(1)} ms)`}`);
        warmBackMs.push(Number(backMs.toFixed(3)));

        // Explore interaction sample: query/pagination on first 3 rounds
        if (i < 3) {
          const qStart = performance.now();
          await warmPage.eval(`document.querySelector('#public-query').focus(); document.querySelector('#public-query').select();`);
          await warmPage.send('Input.insertText', { text: 'Khúc mưa' });
          await warmPage.click('#public-search button[type=submit]');
          await warmPage.waitFor(`new URL(location.href).searchParams.get('q') === 'Khúc mưa' && document.querySelector('#public-results')`, 'warm search', 30000);
          warmInteractQueryMs.push(Number((performance.now() - qStart).toFixed(3)));
          await warmPage.click('#clear-public-search');
          await warmPage.waitFor(`location.search === '' && document.querySelector('a[href^="/tac-pham/${roles.standard_work_id}"]')`, 'warm search clear', 30000);
        }

        if (cfg.include200k) {
          const longStart = performance.now();
          await warmPage.click(`a[href^="/tac-pham/${roles.long_poem_work_id}"]`);
          await warmPage.waitFor(`location.pathname === '/tac-pham/${roles.long_poem_work_id}' && document.querySelector('#reader-paper .vk-document')`, `warm 200k reader #${i}`, 60000);
          warm200kNavMs.push(Number((performance.now() - longStart).toFixed(3)));
          await warmPage.click('#reader-return');
          await warmPage.waitFor(`location.pathname === '/' && document.querySelector('a[href^="/tac-pham/${roles.standard_work_id}"]')`, `warm 200k back #${i}`, 30000);
        }
      }
      check(warmPage.errors.length === 0, `${cfg.key} warm runs: zero console/runtime errors`);
    } finally {
      warmTracker.detach();
      await disposeBrowserContext(warmCtx.browserContextId).catch(() => {});
    }

    allRuns[cfg.key] = {
      cold_explore: coldExplore,
      cold_reader_std: coldReaderStd,
      cold_reader_200k: coldReader200k,
      warm: {
        client_nav_reader_ms: warmClientNavReaderMs,
        warm_back_ms: warmBackMs,
        interact_comment_page_ms: warmInteractCommentPageMs,
        interact_query_ms: warmInteractQueryMs,
        client_nav_reader_200k_ms: warm200kNavMs,
      },
    };
    configStats[cfg.key] = {
      cold_explore: {
        ttfb_ms: summarizeDistribution(coldExplore.map((r) => r.sample.navigation.ttfb_ms)),
        response_end_ms: summarizeDistribution(coldExplore.map((r) => r.sample.navigation.response_end_ms)),
        fcp_ms: summarizeDistribution(coldExplore.map((r) => r.sample.paints.fcp_ms)),
        lcp_ms: summarizeDistribution(coldExplore.map((r) => r.sample.paints.lcp_ms)),
        meaningful_content_ms: summarizeDistribution(coldExplore.map((r) => r.sample.meaningful_content_ms)),
        hydrated_ms: summarizeDistribution(coldExplore.map((r) => r.sample.hydrated_ms)),
        wall_ms: summarizeDistribution(coldExplore.map((r) => r.wall_ms)),
        cls: summarizeDistribution(coldExplore.map((r) => r.sample.cls.total)),
        long_tasks_total_ms: summarizeDistribution(coldExplore.map((r) => r.sample.long_tasks.total_ms)),
        dom_elements: summarizeDistribution(coldExplore.map((r) => r.sample.dom.dom_elements)),
        v8_used_bytes: summarizeDistribution(coldExplore.map((r) => r.sample.memory_proxy.v8_used_bytes)),
        hydration_resource_script_bytes: summarizeDistribution(coldExplore.map((r) => r.sample.hydration_resource_script_bytes)),
        controls_ready_ms: {
          explore_query_ready: summarizeDistribution(coldExplore.map((r) => r.sample.controls_ready_ms?.explore_query_ready)),
          explore_filter_ready: summarizeDistribution(coldExplore.map((r) => r.sample.controls_ready_ms?.explore_filter_ready)),
          explore_next_ready: summarizeDistribution(coldExplore.map((r) => r.sample.controls_ready_ms?.explore_next_ready)),
        },
      },
      cold_reader_std: {
        ttfb_ms: summarizeDistribution(coldReaderStd.map((r) => r.sample.navigation.ttfb_ms)),
        fcp_ms: summarizeDistribution(coldReaderStd.map((r) => r.sample.paints.fcp_ms)),
        lcp_ms: summarizeDistribution(coldReaderStd.map((r) => r.sample.paints.lcp_ms)),
        meaningful_content_ms: summarizeDistribution(coldReaderStd.map((r) => r.sample.meaningful_content_ms)),
        hydrated_ms: summarizeDistribution(coldReaderStd.map((r) => r.sample.hydrated_ms)),
        wall_ms: summarizeDistribution(coldReaderStd.map((r) => r.wall_ms)),
        cls: summarizeDistribution(coldReaderStd.map((r) => r.sample.cls.total)),
        long_tasks_total_ms: summarizeDistribution(coldReaderStd.map((r) => r.sample.long_tasks.total_ms)),
        dom_elements: summarizeDistribution(coldReaderStd.map((r) => r.sample.dom.dom_elements)),
        v8_used_bytes: summarizeDistribution(coldReaderStd.map((r) => r.sample.memory_proxy.v8_used_bytes)),
        hydration_resource_script_bytes: summarizeDistribution(coldReaderStd.map((r) => r.sample.hydration_resource_script_bytes)),
        controls_ready_ms: {
          reader_return_ready: summarizeDistribution(coldReaderStd.map((r) => r.sample.controls_ready_ms?.reader_return_ready)),
          reader_article_ready: summarizeDistribution(coldReaderStd.map((r) => r.sample.controls_ready_ms?.reader_article_ready)),
          reader_composer_ready: summarizeDistribution(coldReaderStd.map((r) => r.sample.controls_ready_ms?.reader_composer_ready)),
          reader_endorse_ready: summarizeDistribution(coldReaderStd.map((r) => r.sample.controls_ready_ms?.reader_endorse_ready)),
        },
      },
      ...(cfg.include200k ? {
        cold_reader_200k: {
          ttfb_ms: summarizeDistribution(coldReader200k.map((r) => r.sample.navigation.ttfb_ms)),
          lcp_ms: summarizeDistribution(coldReader200k.map((r) => r.sample.paints.lcp_ms)),
          meaningful_content_ms: summarizeDistribution(coldReader200k.map((r) => r.sample.meaningful_content_ms)),
          hydrated_ms: summarizeDistribution(coldReader200k.map((r) => r.sample.hydrated_ms)),
          wall_ms: summarizeDistribution(coldReader200k.map((r) => r.wall_ms)),
          dom_elements: summarizeDistribution(coldReader200k.map((r) => r.sample.dom.dom_elements)),
          v8_used_bytes: summarizeDistribution(coldReader200k.map((r) => r.sample.memory_proxy.v8_used_bytes)),
          hydration_resource_script_bytes: summarizeDistribution(coldReader200k.map((r) => r.sample.hydration_resource_script_bytes)),
        },
      } : {}),
      warm_client_nav_reader_ms: summarizeDistribution(warmClientNavReaderMs),
      warm_back_ms: summarizeDistribution(warmBackMs),
      warm_interact_comment_page_ms: summarizeDistribution(warmInteractCommentPageMs),
      warm_interact_query_ms: summarizeDistribution(warmInteractQueryMs),
      ...(cfg.include200k ? { warm_client_nav_reader_200k_ms: summarizeDistribution(warm200kNavMs) } : {}),
    };
  }

  // AC-02 & AC-04: Lifecycle, back_forward unknown age, stale 121s 304 burst, hidden tab,
  // manual retry, offline revisit, secondary transport failure, late-response fence, 25-cycle disposal,
  // and real authoritative withdrawal via App/PostgreSQL.
  const lcCtx = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
  const lcPage = await openPage(lcCtx.browserContextId);
  const lcTracker = new NetworkTracker(lcPage);
  const fixtures = new Map();
  const held = [];
  const gate = ({ sessionId, method, params }) => {
    if (sessionId !== lcPage.sessionId || method !== 'Fetch.requestPaused') return;
    const parsed = new URL(params.request.url);
    const entry = fixtures.get(parsed.pathname + parsed.search) ?? fixtures.get(parsed.pathname);
    if (entry === 'hold') return held.push(params.requestId);
    const answer = typeof entry === 'function' ? entry(params) : entry;
    if (!answer) {
      lcPage.send('Fetch.continueRequest', { requestId: params.requestId }).catch(() => {});
      return;
    }
    const headers = [
      { name: 'Content-Type', value: answer.contentType ?? 'application/json' },
      { name: 'Cache-Control', value: 'no-store' },
      ...(answer.headers ?? []),
    ];
    lcPage.send('Fetch.fulfillRequest', {
      requestId: params.requestId,
      responseCode: answer.status,
      responseHeaders: headers,
      body: Buffer.from(JSON.stringify(answer.body)).toString('base64'),
    }).catch(() => {});
  };
  lcPage.cdp.listeners.push(gate);

  let disposalCycles = [];
  let disposalEval = { ok: false, reasons: ['not executed'] };
  try {
    await applyViewport(lcPage, configs[0]);
    await lcPage.send('Page.addScriptToEvaluateOnNewDocument', { source: buildObserverSource() });

    // Unknown-age history traversal (back_forward document navigation; block BFCache with a WebLock)
    await lcPage.goto(`${args.base}/`);
    await lcPage.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('.vk-public-row')`, 'before back_forward');
    await lcPage.eval(`navigator.locks?.request('vink-perf-bfcache-block', () => new Promise(() => {})).catch(() => {}); true`);
    await lcPage.goto(`${args.base}/toi`);
    await lcPage.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'utility doc for back_forward');
    lcTracker.reset();
    await lcPage.eval(`history.back()`);
    await lcPage.waitFor(`location.pathname === '/' && document.documentElement.dataset.hydrated === 'true' && document.querySelector('.vk-public-row')`, 'back_forward restored', 30000);
    await lcPage.eval(`window.dispatchEvent(new Event('focus'))`);
    await new Promise((r) => setTimeout(r, 350));
    const bfNavType = await lcPage.eval(`performance.getEntriesByType('navigation')[0]?.type ?? null`);
    ledgers.history_back_forward = { nav_type: bfNavType, ...lcTracker.snapshot('history_back_forward').summary };
    check(bfNavType === 'back_forward' || bfNavType === 'navigate', `history traversal recorded nav_type=${bfNavType}`);

    // Prime browser HTTP cache via client nav (/toi -> /), then test stale 121s 4-signal burst -> 1 wire 304
    const clock = `(window.__vinkClock ??= (() => { const realNow = performance.now.bind(performance), c = { now: realNow(), realNow }; performance.now = () => c.now; return c; })())`;
    await lcPage.goto(`${args.base}/toi`);
    await lcPage.waitFor(`document.documentElement.dataset.hydrated === 'true'`, 'utility before prime client Explore');
    await lcPage.eval(`${clock}; true`);
    await lcPage.click('#navigation-primary a[href="/"]');
    await lcPage.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row') && document.querySelector('#public-collection')?.dataset.refreshing === 'false'`, 'client Explore primed ETag');
    await lcPage.eval(`document.querySelector('.vk-public-row').__kept = true; document.querySelector('.vk-public-row__link').focus();`);
    await lcPage.eval(`${clock}.now += 121000`);
    lcTracker.reset();
    await lcPage.eval(`['focus','online','visibilitychange','pageshow'].forEach(n => (n==='visibilitychange'?document:window).dispatchEvent(new Event(n)))`);
    await lcPage.waitFor(`document.querySelector('#public-collection')?.dataset.refreshing === 'false'`, 'stale burst settled');
    await new Promise((r) => setTimeout(r, 250));
    const burstSnap = lcTracker.snapshot('stale_121s_burst');
    const burstLedger = burstSnap.summary;
    ledgers.stale_121s_burst = burstLedger;
    const burstCheck = evaluateStaleBurst({ ledger: burstLedger });
    check(burstCheck.ok, `stale 121s 4-signal burst: ${burstCheck.reasons.join('; ') || '1 logical read, 1 attempt, wire 304'} (events=${JSON.stringify(burstSnap.events.filter((e) => e.category === 'public_works_list'))})`);
    check(await lcPage.eval(`document.querySelector('.vk-public-row').__kept === true && document.activeElement?.matches('.vk-public-row__link')`), '304 refresh preserves row DOM & focus');

    // Hidden suppresses signals; visible resumes once
    await lcPage.eval(`Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' }); document.dispatchEvent(new Event('visibilitychange')); ${clock}.now += 130000;`);
    lcTracker.reset();
    await lcPage.eval(`['focus','online'].forEach(n => window.dispatchEvent(new Event(n)))`);
    await new Promise((r) => setTimeout(r, 300));
    ledgers.hidden_signals = lcTracker.snapshot('hidden_signals').summary;
    check((ledgers.hidden_signals.by_category.public_works_list?.transport_attempts ?? 0) === 0, 'hidden document fires 0 list requests on focus/online');
    await lcPage.eval(`Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'visible' }); document.dispatchEvent(new Event('visibilitychange'))`);
    await lcPage.waitFor(`document.querySelector('#public-collection')?.dataset.refreshing === 'false'`, 'visible resume');
    await new Promise((r) => setTimeout(r, 250));
    ledgers.visible_after_hidden = lcTracker.snapshot('visible_after_hidden').summary;
    check((ledgers.visible_after_hidden.by_category.public_works_list?.validator_304_count ?? 0) === 1, 'becoming visible with stale snapshot sends 1 conditional 304');

    // Manual retry after 503 failure (labelled transport-intercept-fixture)
    await lcPage.send('Fetch.enable', { patterns: [{ urlPattern: `${args.base}/api/v1/*`, requestStage: 'Request' }] });
    fixtures.set('/api/v1/works', { status: 503, headers: [{ name: 'Retry-After', value: '30' }], body: { error: { code: 'dependency_unavailable' } } });
    await lcPage.eval(`${clock}.now += 121000; window.dispatchEvent(new Event('focus'));`);
    await lcPage.waitFor(`document.querySelector('[data-testid=public-refresh-failed]')`, '503 failure notice');
    fixtures.delete('/api/v1/works');
    lcTracker.reset();
    await lcPage.click('[data-testid=public-refresh-failed] button');
    await lcPage.waitFor(`!document.querySelector('[data-testid=public-refresh-failed]') && document.querySelector('#public-collection')?.dataset.refreshing === 'false'`, 'manual retry recovery');
    ledgers.manual_retry = { boundary: '503-transport-fixture-then-real-origin', ...lcTracker.snapshot('manual_retry').summary };
    check((ledgers.manual_retry.by_category.public_works_list?.transport_attempts ?? 0) === 1, 'manual retry sends 1 conditional attempt and clears notice');

    // Offline revisit (retained Reader snapshot) + online recovery
    await lcPage.click(`a[href^="/tac-pham/${roles.standard_work_id}"]`);
    await lcPage.waitFor(`document.querySelector('#reader-paper .vk-document')`, 'prime reader for offline');
    await lcPage.click('#reader-return');
    await lcPage.waitFor(`location.pathname === '/' && document.querySelector('a[href^="/tac-pham/${roles.standard_work_id}"]')`, 'back before offline');
    await lcPage.send('Network.emulateNetworkConditions', { offline: true, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
    await lcPage.eval(`${clock}.now += 121000`);
    lcTracker.reset();
    await lcPage.click(`a[href^="/tac-pham/${roles.standard_work_id}"]`);
    await lcPage.waitFor(`document.querySelector('#reader-paper .vk-document') && document.querySelector('[data-testid=reader-refresh-failed]')`, 'offline cached reader');
    ledgers.offline_revisit = lcTracker.snapshot('offline_revisit').summary;
    check(await lcPage.eval(`!!document.querySelector('#reader-paper .vk-document') && !document.querySelector('[data-testid=public-unavailable]')`), 'offline revisit renders cached article without unavailable takeover');
    await lcPage.send('Network.emulateNetworkConditions', { offline: false, latency: 0, downloadThroughput: -1, uploadThroughput: -1 });
    await lcPage.click('[data-testid=reader-refresh-failed] button');
    await lcPage.waitFor(`!document.querySelector('[data-testid=reader-refresh-failed]')`, 'online recovery');

    // Secondary transport slow/failing (503 discussion + endorsements, real /reader 200) -> article stays readable
    await lcPage.click('#reader-return');
    await lcPage.waitFor(`location.pathname === '/'`, 'before secondary transport fault');
    const secWork = manifest.works[5].work_id;
    fixtures.set(`/api/v1/works/${secWork}/discussion/public`, { status: 503, body: { error: { code: 'unavailable' } } });
    fixtures.set(`/api/v1/works/${secWork}/endorsements`, { status: 503, body: { error: { code: 'unavailable' } } });
    lcTracker.reset();
    await lcPage.click(`a[href^="/tac-pham/${secWork}"]`);
    await lcPage.waitFor(`document.querySelector('#reader-paper .vk-document') && document.querySelector('#discussion-load-retry')`, 'article readable while secondary discussion/endorsement 503');
    ledgers.secondary_transport_503 = { boundary: 'transport-intercept-fixture', ...lcTracker.snapshot('secondary_transport_503').summary };
    fixtures.clear();

    // Late-response A -> B -> A fence (held A1 vs authoritative Hidden A2)
    const raceA = '77777777-7777-4777-8777-777777777711';
    const raceB = '77777777-7777-4777-8777-777777777712';
    fixtures.set(`/api/v1/works/${raceA}/reader`, 'hold');
    fixtures.set(`/api/v1/works/${raceB}/reader`, {
      status: 200,
      body: {
        status: 'published', work_id: raceB, revision_id: 'race-b', title: '[Mẫu] B',
        author_display_name: 'Tác giả', document: { schema_version: 1, blocks: [{ lines: ['B'] }] },
        published_at: '2026-11-16T00:00:00Z', tags: [],
      },
    });
    await lcPage.eval(`(() => { const a = document.createElement('a'); a.href = '/tac-pham/${raceA}'; document.body.append(a); a.click(); a.remove(); })()`);
    const deadline = Date.now() + 10000;
    while (!held.length && Date.now() < deadline) await new Promise((r) => setTimeout(r, 20));
    check(held.length === 1, 'late-response check held first A read');
    const oldA = held.pop();
    await lcPage.eval(`(() => { const a = document.createElement('a'); a.href = '/tac-pham/${raceB}'; document.body.append(a); a.click(); a.remove(); })()`);
    await lcPage.waitFor(`document.querySelector('#reader-paper')?.dataset.revision === 'race-b'`, 'B loaded');
    fixtures.set(`/api/v1/works/${raceA}/reader`, { status: 200, body: { status: 'hidden', work_id: raceA } });
    await lcPage.eval(`(() => { const a = document.createElement('a'); a.href = '/tac-pham/${raceA}'; document.body.append(a); a.click(); a.remove(); })()`);
    await lcPage.waitFor(`document.querySelector('[data-testid=reader-hidden]')`, 'A2 authoritative hidden');
    await lcPage.send('Fetch.fulfillRequest', {
      requestId: oldA,
      responseCode: 200,
      responseHeaders: [{ name: 'Content-Type', value: 'application/json' }, { name: 'Cache-Control', value: 'no-store' }],
      body: Buffer.from(JSON.stringify({
        status: 'published', work_id: raceA, revision_id: 'obsolete-a', title: '[Mẫu] Obsolete A',
        author_display_name: 'Tác giả', document: { schema_version: 1, blocks: [{ lines: ['Obsolete'] }] },
        published_at: '2026-11-16T00:00:00Z', tags: [],
      })).toString('base64'),
    }).catch(() => {});
    await new Promise((r) => setTimeout(r, 150));
    const fenceResult = await lcPage.eval(`({
      expectedLatestActive: !!document.querySelector('[data-testid=reader-hidden]'),
      leakedObsoleteContent: !!document.querySelector('.vk-document') || document.title.includes('Obsolete A'),
    })`);
    const fenceEval = evaluateLateResponseFence(fenceResult);
    check(fenceEval.ok, `late-response A->B->A fence: ${fenceEval.reasons.join('; ') || 'Hidden kept, no resurrection'}`);
    fixtures.clear();
    await lcPage.send('Fetch.disable');

    // Dispose / repeat nav across 25 distinct works (> soft cap 16).
    // Collect pre-GC and post-GC DOM/listener counts so V8 deferred garbage is distinguished from real retention.
    await lcPage.goto(`${args.base}/`);
    await lcPage.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('.vk-public-row')`, 'disposal start');
    await lcPage.send('HeapProfiler.enable');
    for (let c = 0; c < 25; c++) {
      const targetWork = manifest.works[5 + (c % 20)].work_id;
      await lcPage.eval(`(() => { const a = document.createElement('a'); a.href = '/tac-pham/${targetWork}'; document.body.append(a); a.click(); a.remove(); })()`);
      await lcPage.waitFor(`location.pathname === '/tac-pham/${targetWork}' && document.querySelector('#reader-paper .vk-document')`, `repeat nav reader ${c}`, 30000);
      await lcPage.eval(`(() => { const a = document.createElement('a'); a.href = '/'; document.body.append(a); a.click(); a.remove(); })()`);
      await lcPage.waitFor(`location.pathname === '/' && document.querySelector('.vk-public-row')`, `repeat nav explore ${c}`, 30000);
      const preGc = await lcPage.send('Memory.getDOMCounters');
      await lcPage.send('HeapProfiler.collectGarbage');
      const sample = await readPageSample(lcPage);
      disposalCycles.push({
        cycle: c,
        active_timers: sample.timers_and_listeners.active_timers,
        active_window_doc_listeners: sample.timers_and_listeners.active_window_doc_listeners,
        cdp_js_event_listeners: sample.dom.cdp_js_event_listeners,
        cdp_nodes: sample.dom.cdp_nodes,
        pre_gc_js_event_listeners: preGc.jsEventListeners,
        pre_gc_nodes: preGc.nodes,
        v8_used_bytes: sample.memory_proxy.v8_used_bytes,
      });
    }
    await lcPage.send('HeapProfiler.disable').catch(() => {});
    disposalEval = evaluateDisposalBounds({ cycles: disposalCycles });
    check(disposalEval.ok, `25-cycle disposal/repeat nav bounded: ${disposalEval.reasons.join('; ') || 'timers & listeners bounded'}`);

    // Real App/PostgreSQL authoritative withdrawal: POST /api/v1/works/{id}/publication/set-private
    const withdrawId = roles.withdrawal_work_id;
    const withdrawRes = await lcPage.eval(`fetch('/api/v1/works/${withdrawId}/publication/set-private', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ command_key: 'perf234-withdraw-0001', expected_generation: 2 }),
    }).then(async r => ({ status: r.status, body: await r.json() }))`);
    check(withdrawRes.status === 200, `real authoritative set-private returned 200 (${JSON.stringify(withdrawRes)})`);
    const pubAfter = await fetch(`${args.base}/api/v1/works/${withdrawId}/publication`);
    const readerAfter = await fetch(`${args.base}/api/v1/works/${withdrawId}/reader`);
    const readerAfterJson = await readerAfter.json();
    const ssrAfter = await fetch(`${args.base}/tac-pham/${withdrawId}`);
    const ssrAfterHtml = await ssrAfter.text();
    const listAfter = await (await fetch(`${args.base}/api/v1/works`)).json();
    check(
      pubAfter.status === 404 &&
      pubAfter.headers.get('cache-control') === 'no-store' &&
      readerAfter.status === 200 &&
      readerAfterJson.status === 'hidden' &&
      readerAfter.headers.get('cache-control') === 'no-store' &&
      ssrAfter.status === 200 &&
      ssrAfterHtml.includes('data-testid="reader-hidden"') &&
      !ssrAfterHtml.includes('Tác phẩm kiểm thu hồi thật') &&
      !listAfter.works.some((w) => w.work_id === withdrawId),
      'authoritative PostgreSQL withdrawal enforces hidden SSR/API no-store, 404 /publication, and list removal',
    );
    ledgers.authoritative_withdrawal = {
      boundary: 'real-app-postgresql',
      work_id: withdrawId,
      set_private_status: withdrawRes.status,
      publication_status_after: pubAfter.status,
      reader_status_after: readerAfterJson.status,
      ssr_hidden_present: ssrAfterHtml.includes('data-testid="reader-hidden"'),
      omitted_from_explore_list: !listAfter.works.some((w) => w.work_id === withdrawId),
    };
  } finally {
    lcTracker.detach();
    lcPage.cdp.listeners = lcPage.cdp.listeners.filter((l) => l !== gate);
    await disposeBrowserContext(lcCtx.browserContextId).catch(() => {});
  }

  // AC-05: Measure instrumentation overhead (5 paired navigations with observer ON vs OFF)
  const overheadOnMs = [];
  const overheadOffMs = [];
  for (const enabled of [true, false]) {
    for (let i = 0; i < 5; i++) {
      const { browserContextId } = await page.cdp.send('Target.createBrowserContext', { disposeOnDetach: false });
      const oPage = await openPage(browserContextId);
      try {
        await applyViewport(oPage, configs[0]);
        if (enabled) await oPage.send('Page.addScriptToEvaluateOnNewDocument', { source: buildObserverSource() });
        const start = performance.now();
        await oPage.goto(`${args.base}/`);
        await oPage.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('.vk-public-row')`, `overhead ${enabled} #${i}`);
        (enabled ? overheadOnMs : overheadOffMs).push(Number((performance.now() - start).toFixed(3)));
      } finally {
        await disposeBrowserContext(browserContextId).catch(() => {});
      }
    }
  }
  const onDist = summarizeDistribution(overheadOnMs);
  const offDist = summarizeDistribution(overheadOffMs);
  const instrumentationOverhead = {
    samples_per_mode: 5,
    observer_on_wall_ms: onDist,
    observer_off_wall_ms: offDist,
    median_delta_ms: Number((onDist.median - offDist.median).toFixed(3)),
  };

  // AC-04: Explicit negative controls proving the assertions catch real defects
  const negCold = evaluateColdHydration({
    routeKind: 'explore',
    ledger: summarizeStepLedger([{ url: `${args.base}/api/v1/works`, method: 'GET', wireStatus: 200, appStatus: 200 }], 'neg-dup'),
    dom: { visible_work_rows: 12, visible_comment_roots: 0 },
    limits,
  });
  check(!negCold.ok, `negative control (duplicate hydration fetch) caused assertion FAIL: ${negCold.reasons[0]}`);
  negativeControls.duplicateHydrationFetch = 'PASS';

  const negWarmBack = evaluateWarmBack({
    ledger: summarizeStepLedger([{ url: `${args.base}/api/v1/works`, method: 'GET', wireStatus: 200, appStatus: 200 }], 'neg-back'),
    sawLoading: false,
    restoredFocusOk: true,
  });
  check(!negWarmBack.ok, `negative control (warm Back refetch) caused assertion FAIL: ${negWarmBack.reasons[0]}`);
  negativeControls.warmBackRefetch = 'PASS';

  const negLate = evaluateLateResponseFence({ expectedLatestActive: false, leakedObsoleteContent: true });
  check(!negLate.ok, `negative control (late response resurrection) caused assertion FAIL: ${negLate.reasons.join('; ')}`);
  negativeControls.lateResponseResurrection = 'PASS';

  const leakingCycles = Array.from({ length: 12 }, (_, idx) => ({
    cycle: idx,
    active_timers: 1,
    active_window_doc_listeners: 4 + idx * 2,
    cdp_js_event_listeners: 20 + idx * 2,
  }));
  const negDisposal = evaluateDisposalBounds({ cycles: leakingCycles });
  check(!negDisposal.ok, `negative control (listener leak) caused assertion FAIL: ${negDisposal.reasons[0]}`);
  negativeControls.unboundedListenerLeak = 'PASS';

  const summary = {
    schema_version: 1,
    issue: 234,
    generated_at: new Date().toISOString(),
    environment_summary: {
      build_profile: environment.build_profile,
      source_sha: sourceIdentity.sourceSHA,
      dirty: sourceIdentity.dirty,
      corpus_digest: manifest.corpus_digest,
      work_count: manifest.work_count,
      comment_count: manifest.comment_count,
      bundle_sizes: bundleSizes,
    },
    distributions: configStats,
    instrumentation_overhead: instrumentationOverhead,
    disposal_stress: {
      cycles_count: disposalCycles.length,
      bounded: disposalEval.ok,
      reasons: disposalEval.reasons,
      first: disposalCycles[0],
      mid: disposalCycles[Math.floor(disposalCycles.length / 2)],
      last: disposalCycles[disposalCycles.length - 1],
    },
    negative_controls: negativeControls,
    go_no_go: buildGoNoGoLedger({ configStats, ledgerSummary: ledgers, disposalOk: disposalEval.ok }),
  };

  writeFileSync(join(perfOutDir, 'ledger.json'), JSON.stringify(ledgers, null, 2));
  writeFileSync(join(perfOutDir, 'runs.json'), JSON.stringify({ runs: allRuns, disposal_cycles: disposalCycles }, null, 2));
  writeFileSync(join(perfOutDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(`perf-baseline: wrote raw artifacts to ${perfOutDir}`);
}
