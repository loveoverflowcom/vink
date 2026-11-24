#!/usr/bin/env node
// #487 lab replay. The #473 live corpus (mirror.mjs output) is served by the production Worker module
// running unchanged in workerd (Miniflare), behind a loopback stand-in for Cloudflare zone compression,
// then loaded in Chrome with simulated network and CPU. Lab evidence only: no request leaves loopback.
//
// Zone stand-in: a 200 response without Content-Encoding is Brotli-encoded (gzip when br is not
// accepted) only when its Content-Type is compressible and Cache-Control lacks no-transform; it then
// gains "Vary: accept-encoding" and a weak ETag. This is the behaviour observed on vink.forum in
// encoding-before.json. Quality/window default to the calibration in README.md (br q4, lgwin 17).
// Encoded bodies are computed before the first sample, so server-side compression time is not measured.
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { brotliCompressSync, gzipSync, constants } from 'node:zlib';
import { setTimeout as sleep } from 'node:timers/promises';

const opt = Object.fromEntries(process.argv.slice(2).join(' ').split(/\s*--/).filter(Boolean)
  .map(pair => { const [k, ...v] = pair.split(' '); return [k, v.join(' ') || 'true']; }));
const root = resolve(opt.root || '.');
const require = createRequire(join(root, 'infra/cloudflare/package.json'));
const { Miniflare, convertV4MiniflareOptions, Response: MfResponse } = require('miniflare');
const mirror = resolve(opt.mirror);
const workerPath = resolve(opt.worker || join(root, 'infra/cloudflare/production/worker.mjs'));
const chrome = opt.chrome || process.env.VINK_CHROME || 'google-chrome';
const samples = Number(opt.samples || 5);
const device = opt.device || 'desktop';
const network = opt.network ? JSON.parse(opt.network) : { name: 'sim-10mbps-60ms', downMbps: 10, upMbps: 2, latencyMs: 60 };
const zoneQuality = Number(opt['zone-quality'] || 4);
const zoneWindow = Number(opt['zone-lgwin'] || 17);
const zoneGzip = Number(opt['zone-gzip'] || 9);

// Same MIME map as infra/cloudflare/production/artifacts.py.
const TYPES = { '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.woff2': 'font/woff2',
  '.webp': 'image/webp', '.png': 'image/png', '.svg': 'image/svg+xml', '.webmanifest': 'application/manifest+json',
  '.json': 'application/json', '.txt': 'text/plain', '.md': 'text/plain' };
// Subset of Cloudflare's default compressible content types that this corpus can produce.
const ZONE_TYPES = new Set(['text/html', 'text/css', 'text/javascript', 'application/javascript', 'application/json',
  'application/wasm', 'image/svg+xml', 'application/manifest+json', 'text/plain']);

// Stage two release namespaces with identical bytes: A is the live release, B stands for the next one.
const walk = d => readdirSync(d).flatMap(n => { const p = join(d, n); return statSync(p).isDirectory() ? walk(p) : [p]; });
const shaA = readdirSync(join(mirror, '_v'))[0];
if (!/^[0-9a-f]{40}$/.test(shaA)) throw Error('mirror must contain exactly one _v/<sha>/ namespace');
const shaB = createHash('sha256').update('next-release:' + shaA).digest('hex').slice(0, 40);
const stage = mkdtempSync(join(tmpdir(), 'vink-487-stage-'));
const inventory = {};
for (const sha of [shaA, shaB]) {
  cpSync(join(mirror, '_v', shaA), join(stage, '_v', sha), { recursive: true });
  for (const file of walk(join(stage, '_v', sha))) {
    const path = '/' + file.slice(stage.length + 1).split('\\').join('/');
    inventory[path] = { sha256: createHash('sha256').update(readFileSync(file)).digest('hex'), type: TYPES[extname(file)] };
  }
}
const htmlA = readFileSync(join(mirror, 'index.html'), 'utf8');
if (!htmlA.includes('/_v/' + shaA + '/pkg/vink.js')) throw Error('mirrored HTML does not reference its namespace');
// The hydration bootstrap also receives the bare "/_v/<sha>" prefix, so replace every occurrence.
const html = { A: htmlA, B: htmlA.split(shaA).join(shaB) };
const session = readFileSync(join(mirror, 'session.json'));
let release = 'A';

const template = readFileSync(workerPath, 'utf8');
if (template.split('/*VINK_ASSETS*/ {}').length !== 2) throw Error('worker template drift');
const script = template.replace('/*VINK_ASSETS*/ {}', JSON.stringify(inventory));
const mf = new Miniflare(convertV4MiniflareOptions({
  modules: true, script, compatibilityDate: '2026-10-01', host: '127.0.0.1', cf: false,
  bindings: { ORIGIN_KEY: 'edge-replay-scratch' },
  serviceBindings: { ASSETS: { disk: { path: stage } } },
  outboundService: async req => {
    const url = new URL(req.url);
    if (url.origin !== 'https://origin-prod.vink.forum' || req.headers.get('x-vink-origin-key') !== 'edge-replay-scratch') {
      return new MfResponse(null, { status: 502 });
    }
    if (url.pathname === '/') return new MfResponse(html[release], { headers: { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' } });
    if (url.pathname === '/api/v1/auth/session') return new MfResponse(session, { headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } });
    return new MfResponse('{"error":"not_found"}', { status: 404, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } });
  },
}));
await mf.ready;

const encoded = new Map();
function encode(body, encoding) {
  const key = createHash('sha256').update(body).digest('hex') + ':' + encoding;
  if (!encoded.has(key)) {
    encoded.set(key, encoding === 'br'
      ? brotliCompressSync(body, { params: { [constants.BROTLI_PARAM_QUALITY]: zoneQuality,
        [constants.BROTLI_PARAM_LGWIN]: zoneWindow, [constants.BROTLI_PARAM_SIZE_HINT]: body.length } })
      : gzipSync(body, { level: zoneGzip }));
  }
  return encoded.get(key);
}
function zone(method, acceptEncoding, status, headers, body) {
  const type = (headers.get('content-type') || '').split(';')[0].trim().toLowerCase();
  if (method !== 'GET' || status !== 200 || !body.length || headers.has('content-encoding') ||
      /(^|,)\s*no-transform\s*(,|$)/i.test(headers.get('cache-control') || '') || !ZONE_TYPES.has(type)) return body;
  const offered = new Set((acceptEncoding || '').split(',').map(x => x.trim().split(';')).filter(([, q]) => !q || !/^q=0(\.0*)?$/.test(q.trim())).map(([t]) => t.toLowerCase()));
  const encoding = offered.has('br') ? 'br' : offered.has('gzip') ? 'gzip' : null;
  if (!encoding) return body;
  headers.set('content-encoding', encoding);
  headers.set('vary', [headers.get('vary'), 'accept-encoding'].filter(Boolean).join(', '));
  const etag = headers.get('etag');
  if (etag && !etag.startsWith('W/')) headers.set('etag', 'W/' + etag);
  return encode(body, encoding);
}
const server = createServer(async (req, res) => {
  try {
    const path = new URL(req.url, 'http://x');
    const target = new URL('https://vink.forum'); target.pathname = path.pathname; target.search = path.search;
    const forwarded = Object.fromEntries(Object.entries(req.headers).filter(([k]) => !/^(host|connection)$/i.test(k)));
    const r = await mf.dispatchFetch(target.href, { method: req.method, headers: { ...forwarded, host: 'vink.forum', 'cf-connecting-ip': '192.0.2.2' }, redirect: 'manual' });
    const headers = new Headers(r.headers);
    let body = Buffer.from(await r.arrayBuffer());
    body = zone(req.method, req.headers['accept-encoding'], r.status, headers, body);
    headers.delete('content-length'); headers.delete('transfer-encoding');
    for (const [k, v] of headers) res.setHeader(k, v);
    if (req.method !== 'HEAD' && r.status !== 304) res.setHeader('content-length', body.length);
    res.writeHead(r.status); res.end(req.method === 'HEAD' ? undefined : body);
  } catch (error) { console.error(error); res.writeHead(502).end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const base = 'http://127.0.0.1:' + server.address().port;

// Warm the encoded-body cache for both namespaces (and record the wire bytes per asset) before measuring.
const wire = {};
for (const sha of [shaA, shaB]) for (const path of Object.keys(inventory).filter(p => p.startsWith('/_v/' + sha + '/'))) {
  const r = await fetch(base + path, { headers: { 'accept-encoding': 'br, gzip, zstd' } });
  const bytes = Number(r.headers.get('content-length'));
  await r.arrayBuffer();
  if (sha === shaA) wire[path.replace(shaA, '<sha>')] = { encoding: r.headers.get('content-encoding') || 'identity', bytes,
    cacheControl: r.headers.get('cache-control'), etag: r.headers.get('etag'), vary: r.headers.get('vary') };
}

const PROBE = String.raw`(() => {
  if (window.__perf473) return;
  const p = window.__perf473 = { marker: null, wasm: [], fcp: null, lcp: null, longtasks: [] };
  new MutationObserver(() => { if (p.marker == null && document.documentElement?.hasAttribute('data-hydrated')) p.marker = performance.now(); })
    .observe(document, { attributes: true, subtree: true, attributeFilter: ['data-hydrated'] });
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
  observe('largest-contentful-paint', es => { p.lcp = es.at(-1).startTime; });
  observe('longtask', es => { for (const e of es) p.longtasks.push([+e.startTime.toFixed(1), +e.duration.toFixed(1)]); });
})();`;

class Cdp {
  constructor(ws) {
    this.ws = ws; this.id = 0; this.pending = new Map(); this.listeners = new Set();
    ws.onmessage = event => {
      const m = JSON.parse(event.data);
      if (m.id) { const p = this.pending.get(m.id); this.pending.delete(m.id); m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result); }
      else for (const l of this.listeners) l(m);
    };
  }
  send(method, params = {}, sessionId) {
    const id = ++this.id; this.ws.send(JSON.stringify({ id, method, params, sessionId }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }
  on(fn) { this.listeners.add(fn); return () => this.listeners.delete(fn); }
}
async function launch(profile) {
  const proc = spawn(chrome, ['--headless=new', '--no-sandbox', '--remote-debugging-port=0', '--user-data-dir=' + profile,
    '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-component-update',
    '--disable-sync', '--window-size=1280,900', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
  const wsUrl = await new Promise((resolve, reject) => {
    let buffer = '';
    proc.stderr.on('data', chunk => { buffer += chunk; const found = /DevTools listening on (ws:\/\/\S+)/.exec(buffer); if (found) resolve(found[1]); });
    proc.on('exit', code => reject(new Error('chrome exited ' + code)));
  });
  const ws = new WebSocket(wsUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  const cdp = new Cdp(ws);
  return { proc, cdp, version: (await cdp.send('Browser.getVersion')).product };
}
async function close(browser) {
  const exited = new Promise(resolve => browser.proc.on('exit', resolve));
  await browser.cdp.send('Browser.close').catch(() => {});
  await Promise.race([exited, sleep(10000)]);
  browser.proc.kill('SIGKILL');
}
const clean = raw => {
  try { const u = new URL(raw); if (!/^https?:$/.test(u.protocol)) return u.protocol;
    return (u.origin === base ? '' : u.origin) + u.pathname.replace(shaA, '<A>').replace(shaB, '<B>'); } catch { return 'unparsed'; }
};
const conditions = offline => ({ offline, latency: offline ? 0 : network.latencyMs,
  downloadThroughput: offline ? -1 : network.downMbps * 125000, uploadThroughput: offline ? -1 : network.upMbps * 125000 });

async function openTab(browser) {
  const { cdp } = browser;
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const tab = { targetId, sessionId, cdp, requests: new Map(), inflight: new Set(), lastActivity: Date.now(), errors: [] };
  tab.send = (m, p) => cdp.send(m, p, sessionId);
  tab.off = cdp.on(m => {
    if (m.sessionId !== sessionId) return;
    const p = m.params || {};
    const r = p.requestId && tab.requests.get(p.requestId);
    switch (m.method) {
      case 'Network.requestWillBeSent':
        tab.requests.set(p.requestId, { url: clean(p.request.url), type: p.type, method: p.request.method, ...(r || {}) });
        tab.inflight.add(p.requestId); tab.lastActivity = Date.now(); break;
      case 'Network.requestServedFromCache': if (r) r.servedFromCache = true; break;
      case 'Network.responseReceived': if (r) {
        const h = Object.fromEntries(Object.entries(p.response.headers).map(([k, v]) => [k.toLowerCase(), v]));
        Object.assign(r, { status: p.response.status, fromDiskCache: !!p.response.fromDiskCache,
          encoding: h['content-encoding'] || null, cacheControl: h['cache-control'] || null, etag: h['etag'] || null,
          timing: p.response.timing ? { send: p.response.timing.sendStart, headers: p.response.timing.receiveHeadersEnd } : null });
      } break;
      case 'Network.loadingFinished': if (r) r.encodedDataLength = p.encodedDataLength;
        tab.inflight.delete(p.requestId); tab.lastActivity = Date.now(); break;
      case 'Network.loadingFailed': if (r) { r.failed = p.errorText; r.canceled = p.canceled; }
        tab.inflight.delete(p.requestId); tab.lastActivity = Date.now(); break;
      case 'Runtime.exceptionThrown': tab.errors.push('exception: ' + (p.exceptionDetails?.exception?.description || p.exceptionDetails?.text || '').split('\n')[0]); break;
      case 'Runtime.consoleAPICalled': if (p.type === 'error') tab.errors.push('console.error: ' + String(p.args?.[0]?.value ?? p.args?.[0]?.description ?? '').split('\n')[0]); break;
    }
  });
  await tab.send('Page.enable'); await tab.send('Network.enable'); await tab.send('Runtime.enable');
  await tab.send('Network.emulateNetworkConditions', conditions(false));
  await tab.send('Page.addScriptToEvaluateOnNewDocument', { source: PROBE });
  if (device === 'mobile') {
    await tab.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 3, mobile: true });
    await tab.send('Emulation.setCPUThrottlingRate', { rate: 4 });
  } else await tab.send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  return tab;
}
async function evaluate(tab, expression) {
  const { result, exceptionDetails } = await tab.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (exceptionDetails) throw new Error(exceptionDetails.text);
  return result.value;
}
async function settle(tab, needMarker = true, timeoutMs = 120000) {
  const start = Date.now();
  for (;;) {
    await sleep(100);
    let ok = false;
    try { ok = await evaluate(tab, 'document.readyState === "complete" && !!window.__perf473' + (needMarker ? ' && window.__perf473.marker != null' : '')); } catch {}
    if (ok && tab.inflight.size === 0 && Date.now() - tab.lastActivity >= 1500) return true;
    if (Date.now() - start > timeoutMs) return false;
  }
}
const COLLECT = '(' + (() => {
  const n = performance.getEntriesByType('navigation')[0];
  const r = x => x == null ? null : Math.round(x * 10) / 10;
  return { href: location.href, nav: n && { type: n.type, ttfb: r(n.responseStart), dcl: r(n.domContentLoadedEventEnd), load: r(n.loadEventEnd) },
    probe: window.__perf473 && JSON.parse(JSON.stringify(window.__perf473)),
    hydrated: document.documentElement.dataset.hydrated === 'true', mainChars: (document.querySelector('main') || document.body)?.innerText.length ?? 0 };
}).toString() + ')()';
const pageEvent = (tab, name) => new Promise(resolve => { const off = tab.cdp.on(m => { if (m.sessionId === tab.sessionId && m.method === name) { off(); resolve(); } }); });
async function load(tab, label, action) {
  tab.requests.clear(); tab.inflight.clear(); tab.errors = []; tab.lastActivity = Date.now();
  const t0 = Date.now();
  await action();
  const settled = await settle(tab);
  const page = await evaluate(tab, COLLECT);
  page.href = clean(page.href);
  return { label, release, settled, wallMs: Date.now() - t0, page, requests: [...tab.requests.values()].filter(x => x.url), errors: tab.errors };
}
async function navigate(tab, url) { const loaded = pageEvent(tab, 'Page.loadEventFired'); await tab.send('Page.navigate', { url }); await Promise.race([loaded, sleep(120000)]); }
async function reload(tab, ignoreCache) { const loaded = pageEvent(tab, 'Page.loadEventFired'); await tab.send('Page.reload', { ignoreCache }); await Promise.race([loaded, sleep(120000)]); }

const runs = [];
let version = null;
try {
  for (let i = 0; i < samples; i++) {
    release = 'A';
    const profile = mkdtempSync(join(tmpdir(), 'vink-487-'));
    try {
      let browser = await launch(profile); version = browser.version;
      let tab = await openTab(browser);
      const sample = { sample: i, device, loads: [] };
      sample.loads.push(await load(tab, 'cold', () => navigate(tab, base + '/')));
      sample.loads.push(await load(tab, 'reload', () => reload(tab, false)));
      await tab.cdp.send('Target.closeTarget', { targetId: tab.targetId }); tab.off();
      await sleep(300);
      tab = await openTab(browser);
      sample.loads.push(await load(tab, 'reopen_tab', () => navigate(tab, base + '/')));
      await close(browser);
      browser = await launch(profile);
      tab = await openTab(browser);
      sample.loads.push(await load(tab, 'reopen_browser', () => navigate(tab, base + '/')));
      // No Service Worker exists and HTML is no-store: an offline reload is expected to fail in both arms.
      await tab.send('Network.emulateNetworkConditions', conditions(true));
      await tab.send('Page.reload', { ignoreCache: false }).catch(() => {});
      await sleep(3000);
      sample.loads.push({ label: 'offline_reload', release, offline: await evaluate(tab, '({ url: document.URL.startsWith("chrome-error:") ? "chrome-error" : "document", hydrated: document.documentElement?.dataset.hydrated === "true" })').catch(e => ({ error: e.message })) });
      await tab.send('Network.emulateNetworkConditions', conditions(false));
      release = 'B';
      sample.loads.push(await load(tab, 'after_release', () => navigate(tab, base + '/')));
      sample.loads.push(await load(tab, 'hard_reload', () => reload(tab, true)));
      await close(browser);
      runs.push(sample);
      process.stderr.write('sample ' + i + ' ' + sample.loads.map(l => l.label + '=' + (l.page?.probe?.marker?.toFixed(0) ?? l.offline?.url)).join(' ') + '\n');
    } finally { rmSync(profile, { recursive: true, force: true }); }
  }
} finally {
  server.close(); await mf.dispose(); rmSync(stage, { recursive: true, force: true });
}
const corpus = Object.fromEntries(Object.entries(inventory).filter(([p]) => p.startsWith('/_v/' + shaA + '/')).map(([p, e]) => [p.replace(shaA, '<sha>'), e.sha256]));
writeFileSync(opt.out, JSON.stringify({
  release: shaA, corpus, wire, worker: { path: workerPath.slice(root.length + 1), sha256: createHash('sha256').update(template).digest('hex') },
  zone: { brotliQuality: zoneQuality, brotliWindow: zoneWindow, gzipLevel: zoneGzip, respectsNoTransform: true },
  chrome: version, device, cpuThrottle: device === 'mobile' ? 4 : 1,
  viewport: device === 'mobile' ? '390x844@3 mobile (emulated)' : '1280x900@1', network, samples, runs,
}));
process.stderr.write('wrote ' + opt.out + '\n');
