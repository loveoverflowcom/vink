#!/usr/bin/env node
// #473 reload/reopen matrix. Read-only browser loads of one public URL through CDP.
// Persists timings, sizes, cache sources and sanitized paths only: no cookie, header value,
// query value, body or storage content leaves the browser.
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';

const opt = Object.fromEntries(process.argv.slice(2).join(' ').split(/\s*--/).filter(Boolean)
  .map(pair => { const [k, ...v] = pair.split(' '); return [k, v.join(' ') || 'true']; }));
const url = new URL(opt.url);
const chrome = opt.chrome || process.env.VINK_CHROME || 'google-chrome';
const samples = Number(opt.samples || 5);
const device = opt.device || 'desktop';
const marker = opt.marker || null; // attribute on <html> that marks hydration, e.g. data-hydrated
const trace = opt.trace === 'true';
const spaHref = opt['spa-href'] || null;
const spaSelector = opt['spa-selector'] || null;
const readySelector = opt['ready-selector'] || null;

const PROBE = String.raw`(() => {
  if (window.__perf473) return;
  const p = window.__perf473 = { marker: null, wasm: [], fcp: null, lcp: null, lcpTag: null,
    longtasks: [], cls: 0, pageshow: null, ready: null };
  const name = ${JSON.stringify(marker)};
  const ready = ${JSON.stringify(readySelector)};
  if (ready) {
    const check = () => { if (p.ready == null && document.querySelector(ready)) p.ready = performance.now(); };
    new MutationObserver(check).observe(document, { childList: true, subtree: true });
  }
  if (name) {
    const mark = () => { if (p.marker == null && document.documentElement?.hasAttribute(name)) p.marker = performance.now(); };
    new MutationObserver(mark).observe(document, { attributes: true, subtree: true, attributeFilter: [name] });
  }
  for (const fn of ['instantiateStreaming', 'instantiate', 'compileStreaming', 'compile']) {
    const original = WebAssembly[fn];
    if (typeof original !== 'function') continue;
    WebAssembly[fn] = function (...args) {
      const t0 = performance.now();
      const result = original.apply(this, args);
      Promise.resolve(result).then(() => p.wasm.push({ fn, t0, t1: performance.now() }),
        e => p.wasm.push({ fn, t0, t1: performance.now(), error: String(e && e.name) }));
      return result;
    };
  }
  const observe = (type, cb) => { try { new PerformanceObserver(l => cb(l.getEntries())).observe({ type, buffered: true }); } catch {} };
  observe('paint', es => { for (const e of es) if (e.name === 'first-contentful-paint') p.fcp = e.startTime; });
  observe('largest-contentful-paint', es => { const e = es.at(-1); p.lcp = e.startTime; p.lcpTag = e.element?.tagName ?? null; });
  observe('longtask', es => { for (const e of es) p.longtasks.push([+e.startTime.toFixed(1), +e.duration.toFixed(1)]); });
  observe('layout-shift', es => { for (const e of es) if (!e.hadRecentInput) p.cls += e.value; });
  addEventListener('pageshow', e => { p.pageshow = { persisted: e.persisted, t: performance.now() }; });
})();`;

class Cdp {
  constructor(ws) {
    this.ws = ws; this.id = 0; this.pending = new Map(); this.listeners = new Set();
    ws.onmessage = event => {
      const m = JSON.parse(event.data);
      if (m.id) {
        const p = this.pending.get(m.id); this.pending.delete(m.id);
        m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result);
      } else for (const l of this.listeners) l(m);
    };
  }
  send(method, params = {}, sessionId) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params, sessionId }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }
  on(fn) { this.listeners.add(fn); return () => this.listeners.delete(fn); }
}

async function launch(profile) {
  const proc = spawn(chrome, ['--headless=new', '--no-sandbox', '--remote-debugging-port=0',
    '--user-data-dir=' + profile, '--no-first-run', '--no-default-browser-check',
    '--disable-background-networking', '--disable-component-update', '--disable-sync',
    '--window-size=1280,900', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
  const wsUrl = await new Promise((resolve, reject) => {
    let buffer = '';
    proc.stderr.on('data', chunk => {
      buffer += chunk;
      const found = /DevTools listening on (ws:\/\/\S+)/.exec(buffer);
      if (found) resolve(found[1]);
    });
    proc.on('exit', code => reject(new Error('chrome exited ' + code)));
  });
  const ws = new WebSocket(wsUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  const cdp = new Cdp(ws);
  const version = await cdp.send('Browser.getVersion');
  return { proc, cdp, version: version.product };
}

async function close(browser) {
  const exited = new Promise(resolve => browser.proc.on('exit', resolve));
  await browser.cdp.send('Browser.close').catch(() => {});
  await Promise.race([exited, sleep(10000)]);
  browser.proc.kill('SIGKILL');
}

function clean(raw) {
  try {
    const u = new URL(raw);
    if (!/^https?:$/.test(u.protocol)) return u.protocol;
    const params = [...new Set([...u.searchParams.keys()])].sort();
    const path = u.pathname.replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g, ':uuid');
    return (u.origin === url.origin ? '' : u.origin) + path + (params.length ? '?' + params.join('&') : '');
  } catch { return 'unparsed'; }
}

async function openTab(browser) {
  const { cdp } = browser;
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const tab = { targetId, sessionId, cdp, requests: new Map(), inflight: new Set(), lastActivity: Date.now(), bfcache: null };
  const send = (m, p) => cdp.send(m, p, sessionId);
  tab.send = send;
  tab.off = cdp.on(m => {
    if (m.sessionId !== sessionId) return;
    const p = m.params || {};
    const r = p.requestId && tab.requests.get(p.requestId);
    switch (m.method) {
      case 'Network.requestWillBeSent':
        if (p.redirectResponse && r) r.redirects = (r.redirects || 0) + 1;
        tab.requests.set(p.requestId, { url: clean(p.request.url), type: p.type, method: p.request.method, t: p.timestamp, ...(r || {}) });
        tab.inflight.add(p.requestId); tab.lastActivity = Date.now(); break;
      case 'Network.requestServedFromCache': if (r) r.servedFromCache = true; break;
      case 'Network.responseReceived': if (r) {
        const h = Object.fromEntries(Object.entries(p.response.headers).map(([k, v]) => [k.toLowerCase(), v]));
        Object.assign(r, { status: p.response.status, protocol: p.response.protocol, mime: p.response.mimeType,
          fromDiskCache: !!p.response.fromDiskCache, fromServiceWorker: !!p.response.fromServiceWorker,
          fromPrefetchCache: !!p.response.fromPrefetchCache,
          encoding: h['content-encoding'] || null, cacheControl: h['cache-control'] || null,
          assetRoute: h['x-vink-asset-route'] || null, assetRevision: h['x-vink-asset-revision'] || null,
          cfCache: h['cf-cache-status'] || null, contentLength: h['content-length'] ? Number(h['content-length']) : null,
          timing: p.response.timing ? { dns: p.response.timing.dnsEnd - p.response.timing.dnsStart,
            connect: p.response.timing.connectEnd - p.response.timing.connectStart,
            ssl: p.response.timing.sslEnd - p.response.timing.sslStart,
            send: p.response.timing.sendStart, headers: p.response.timing.receiveHeadersEnd } : null });
      } break;
      case 'Network.responseReceivedExtraInfo': if (r) r.wireStatus = p.statusCode; else tab.requests.set(p.requestId, { wireStatus: p.statusCode }); break;
      case 'Network.loadingFinished': if (r) { r.encodedDataLength = p.encodedDataLength; r.end = p.timestamp; }
        tab.inflight.delete(p.requestId); tab.lastActivity = Date.now(); break;
      case 'Network.loadingFailed': if (r) { r.failed = p.errorText; r.canceled = p.canceled; }
        tab.inflight.delete(p.requestId); tab.lastActivity = Date.now(); break;
      case 'Page.backForwardCacheNotUsed': tab.bfcache = { used: false, reasons: p.notRestoredExplanations.map(e => e.reason) }; break;
    }
  });
  await send('Page.enable');
  await send('Network.enable');
  await send('Runtime.enable');
  await send('Page.addScriptToEvaluateOnNewDocument', { source: PROBE });
  if (device === 'mobile') {
    await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 3, mobile: true });
    await send('Emulation.setCPUThrottlingRate', { rate: 4 });
  } else {
    await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  }
  return tab;
}

async function evaluate(tab, expression) {
  const { result, exceptionDetails } = await tab.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (exceptionDetails) throw new Error(exceptionDetails.text);
  return result.value;
}

async function settle(tab, { needMarker = !!marker, quietMs = 1500, timeoutMs = 60000 } = {}) {
  const start = Date.now();
  for (;;) {
    await sleep(100);
    let ok = false;
    try {
      ok = await evaluate(tab, 'document.readyState === "complete" && !!window.__perf473' +
        (needMarker ? ' && window.__perf473.marker != null' : ''));
    } catch { ok = false; }
    if (ok && tab.inflight.size === 0 && Date.now() - tab.lastActivity >= quietMs) return true;
    if (Date.now() - start > timeoutMs) return false;
  }
}

const COLLECT = '(' + (() => {
  const n = performance.getEntriesByType('navigation')[0];
  const r = x => x == null ? null : Math.round(x * 10) / 10;
  return {
    href: location.href,
    nav: n && { type: n.type, dns: r(n.domainLookupEnd - n.domainLookupStart), connect: r(n.connectEnd - n.connectStart),
      tls: n.secureConnectionStart ? r(n.connectEnd - n.secureConnectionStart) : 0, requestStart: r(n.requestStart),
      ttfb: r(n.responseStart), responseEnd: r(n.responseEnd), domInteractive: r(n.domInteractive),
      dcl: r(n.domContentLoadedEventEnd), load: r(n.loadEventEnd), transferSize: n.transferSize,
      encodedBodySize: n.encodedBodySize, decodedBodySize: n.decodedBodySize, protocol: n.nextHopProtocol,
      notRestoredReasons: n.notRestoredReasons ? JSON.parse(JSON.stringify(n.notRestoredReasons)) : null },
    resources: performance.getEntriesByType('resource').map(e => ({ name: e.name, initiator: e.initiatorType,
      start: r(e.startTime), responseStart: r(e.responseStart), end: r(e.responseEnd), transferSize: e.transferSize,
      encodedBodySize: e.encodedBodySize, decodedBodySize: e.decodedBodySize, deliveryType: e.deliveryType ?? null,
      renderBlocking: e.renderBlockingStatus ?? null, status: e.responseStatus ?? null })),
    probe: window.__perf473 && JSON.parse(JSON.stringify(window.__perf473)),
    mainChars: (document.querySelector('main') || document.body)?.innerText.length ?? 0,
  };
}).toString() + ')()';

function summarizeRequests(tab) {
  return [...tab.requests.values()].filter(x => x.url).map(x => ({ ...x }));
}

async function capture(tab, label, extra = {}) {
  const page = await evaluate(tab, COLLECT);
  for (const res of page.resources) res.name = clean(res.name);
  page.href = clean(page.href);
  return { label, at: new Date().toISOString(), ...extra, page, requests: summarizeRequests(tab), bfcache: tab.bfcache };
}

async function load(tab, label, action) {
  tab.requests.clear(); tab.inflight.clear(); tab.bfcache = null; tab.lastActivity = Date.now();
  const t0 = Date.now();
  await action();
  const settled = await settle(tab);
  return capture(tab, label, { settled, wallMs: Date.now() - t0 });
}

async function navigate(tab, target) {
  const loaded = new Promise(resolve => { const off = tab.cdp.on(m => { if (m.sessionId === tab.sessionId && m.method === 'Page.loadEventFired') { off(); resolve(); } }); });
  await tab.send('Page.navigate', { url: target });
  await Promise.race([loaded, sleep(60000)]);
}

async function reload(tab, ignoreCache) {
  const loaded = new Promise(resolve => { const off = tab.cdp.on(m => { if (m.sessionId === tab.sessionId && m.method === 'Page.loadEventFired') { off(); resolve(); } }); });
  await tab.send('Page.reload', { ignoreCache });
  await Promise.race([loaded, sleep(60000)]);
}

async function spa(tab, label, href, selector) {
  tab.requests.clear(); tab.inflight.clear(); tab.lastActivity = Date.now();
  const result = await evaluate(tab, '(' + (async (href, selector) => {
    const target = selector ? document.querySelector(selector) : href ? [...document.querySelectorAll('a[href]')].find(a => a.getAttribute('href') === href)
      : [...document.querySelectorAll('a[href]')].find(a => a.origin === location.origin && a.pathname !== location.pathname && !a.hash && !a.download);
    if (!target) return { error: 'no link' };
    const main = document.querySelector('main') || document.body;
    const from = location.href;
    let last = 0, urlAt = null;
    const t0 = performance.now();
    const mo = new MutationObserver(() => { last = performance.now(); });
    mo.observe(document.body, { childList: true, subtree: true, characterData: true });
    target.click();
    const deadline = t0 + 15000;
    while (performance.now() < deadline) {
      await new Promise(r => setTimeout(r, 20));
      if (urlAt == null && location.href !== from) urlAt = performance.now();
      if (urlAt != null && last && performance.now() - last > 750) break;
    }
    mo.disconnect();
    return { href: target.getAttribute('href').replace(/\?.*$/, '').replace(/[0-9a-f]{8}-[0-9a-f-]{27}/g, ':uuid'), urlMs: urlAt && urlAt - t0, domQuietMs: last && last - t0, mainChars: main.innerText.length };
  }).toString() + ')(' + JSON.stringify(href) + ',' + JSON.stringify(selector) + ')').catch(e => ({ error: e.message }));
  await settle(tab, { needMarker: false, quietMs: 1000, timeoutMs: 20000 });
  return { label, at: new Date().toISOString(), spa: result, requests: summarizeRequests(tab) };
}

async function spaBack(tab) {
  tab.requests.clear(); tab.inflight.clear(); tab.lastActivity = Date.now();
  const result = await evaluate(tab, '(' + (async () => {
    const from = location.href; let last = 0, urlAt = null;
    const t0 = performance.now();
    const mo = new MutationObserver(() => { last = performance.now(); });
    mo.observe(document.body, { childList: true, subtree: true, characterData: true });
    history.back();
    const deadline = t0 + 15000;
    while (performance.now() < deadline) {
      await new Promise(r => setTimeout(r, 20));
      if (urlAt == null && location.href !== from) urlAt = performance.now();
      if (urlAt != null && last && performance.now() - last > 750) break;
    }
    mo.disconnect();
    return { urlMs: urlAt && urlAt - t0, domQuietMs: last && last - t0, sameDocument: !!window.__perf473 };
  }).toString() + ')()').catch(e => ({ error: e.message }));
  await settle(tab, { needMarker: false, quietMs: 1000, timeoutMs: 20000 });
  return { label: 'spa_back', at: new Date().toISOString(), spa: result, requests: summarizeRequests(tab) };
}

async function withTrace(browser, fn) {
  if (!trace) return { value: await fn(), trace: null };
  const events = [];
  const off = browser.cdp.on(m => { if (!m.sessionId && m.method === 'Tracing.dataCollected') events.push(...m.params.value); });
  const done = new Promise(resolve => { const o = browser.cdp.on(m => { if (!m.sessionId && m.method === 'Tracing.tracingComplete') { o(); resolve(); } }); });
  await browser.cdp.send('Tracing.start', { transferMode: 'ReportEvents',
    traceConfig: { includedCategories: ['v8.wasm', 'disabled-by-default-v8.wasm.detailed', 'devtools.timeline', 'v8', 'v8.execute', 'blink', 'loading'] } });
  const value = await fn();
  await browser.cdp.send('Tracing.end');
  await done; off();
  const threads = new Map();
  for (const e of events) if (e.ph === 'M' && e.name === 'thread_name') threads.set(e.pid + ':' + e.tid, e.args.name);
  const sums = new Map();
  for (const e of events) {
    if (e.ph !== 'X' || !e.dur) continue;
    const thread = (threads.get(e.pid + ':' + e.tid) || 'other').replace(/\d+$/, '');
    if (!/CrRendererMain|DedicatedWorker|ThreadPoolForegroundWorker|ThreadPoolBackgroundWorker|V8/.test(thread) && !/wasm/i.test(e.cat)) continue;
    const key = thread + ' | ' + e.cat + ' | ' + e.name;
    const s = sums.get(key) || { count: 0, ms: 0 };
    s.count++; s.ms += e.dur / 1000; sums.set(key, s);
  }
  const top = [...sums.entries()].sort((a, b) => b[1].ms - a[1].ms).slice(0, 60)
    .map(([key, s]) => ({ key, count: s.count, ms: Math.round(s.ms * 10) / 10 }));
  const wasmNames = [...new Set(events.filter(e => /wasm/i.test(e.cat) || /wasm/i.test(e.name)).map(e => e.name))].sort();
  return { value, trace: { top, wasmNames } };
}

const runs = [];
let version = null;
for (let i = 0; i < samples; i++) {
  const profile = mkdtempSync(join(tmpdir(), 'vink-473-'));
  try {
    let browser = await launch(profile);
    version = browser.version;
    let tab = await openTab(browser);
    const sample = { sample: i, device, loads: [] };
    sample.loads.push(await load(tab, 'cold', () => navigate(tab, url.href)));
    const traced = await withTrace(browser, () => load(tab, 'reload', () => reload(tab, false)));
    traced.value.trace = traced.trace; sample.loads.push(traced.value);
    const spaResult = await spa(tab, 'spa', spaHref, spaSelector);
    sample.loads.push(spaResult);
    if (spaResult.spa && spaResult.spa.urlMs != null) sample.loads.push(await spaBack(tab));
    await navigate(tab, 'about:blank');
    await sleep(300);
    sample.loads.push(await load(tab, 'history_back', async () => {
      const loaded = new Promise(resolve => { const off = tab.cdp.on(m => { if (m.sessionId === tab.sessionId && (m.method === 'Page.loadEventFired' || m.method === 'Page.frameNavigated')) { off(); resolve(); } }); });
      const h = await tab.send('Page.getNavigationHistory');
      await tab.send('Page.navigateToHistoryEntry', { entryId: h.entries[h.currentIndex - 1].id });
      await Promise.race([loaded, sleep(60000)]);
    }));
    await tab.cdp.send('Target.closeTarget', { targetId: tab.targetId }); tab.off();
    await sleep(300);
    tab = await openTab(browser);
    sample.loads.push(await load(tab, 'reopen_tab', () => navigate(tab, url.href)));
    await close(browser);
    browser = await launch(profile);
    tab = await openTab(browser);
    const reopened = await withTrace(browser, () => load(tab, 'reopen_browser', () => navigate(tab, url.href)));
    reopened.value.trace = reopened.trace; sample.loads.push(reopened.value);
    sample.loads.push(await load(tab, 'hard_reload', () => reload(tab, true)));
    await close(browser);
    runs.push(sample);
    process.stderr.write('sample ' + i + ' done\n');
  } finally { rmSync(profile, { recursive: true, force: true }); }
}

const output = { url: clean(url.href), origin: url.origin, chrome: version, device,
  cpuThrottle: device === 'mobile' ? 4 : 1, viewport: device === 'mobile' ? '390x844@3 mobile (emulated)' : '1280x900@1',
  network: 'real network from the measuring workstation, no throttling', marker, readySelector, spa: spaHref || spaSelector, samples, runs };
writeFileSync(opt.out, JSON.stringify(output));
process.stderr.write('wrote ' + opt.out + '\n');
