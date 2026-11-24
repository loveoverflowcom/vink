#!/usr/bin/env node
// #473 local control: the same live release bytes served from loopback either identity-encoded
// or Brotli-encoded, cold HTTP cache, simulated network through CDP. Lab evidence only.
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname } from 'node:path';
import { brotliCompressSync, constants } from 'node:zlib';
import { setTimeout as sleep } from 'node:timers/promises';

const opt = Object.fromEntries(process.argv.slice(2).join(' ').split(/\s*--/).filter(Boolean)
  .map(pair => { const [k, ...v] = pair.split(' '); return [k, v.join(' ') || 'true']; }));
const root = opt.mirror;
const chrome = opt.chrome || 'google-chrome';
const samples = Number(opt.samples || 5);
const types = { '.wasm': 'application/wasm', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml',
  '.woff2': 'font/woff2', '.webp': 'image/webp', '.webmanifest': 'application/manifest+json', '.json': 'application/json' };
const compressible = new Set(['.wasm', '.js', '.css', '.svg', '.webmanifest']);
const cache = new Map();
function body(path, quality) {
  const key = path + ':' + quality;
  if (!cache.has(key)) {
    const raw = readFileSync(path);
    cache.set(key, quality ? brotliCompressSync(raw, { params: { [constants.BROTLI_PARAM_QUALITY]: quality,
      [constants.BROTLI_PARAM_SIZE_HINT]: raw.length } }) : raw);
  }
  return cache.get(key);
}
let quality = 0;
const server = createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  if (url.pathname === '/') { res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' }); return res.end(readFileSync(join(root, 'index.html'))); }
  if (url.pathname === '/api/v1/auth/session') { res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' }); return res.end(readFileSync(join(root, 'session.json'))); }
  const file = join(root, url.pathname);
  if (!url.pathname.startsWith('/_v/') || !existsSync(file)) { res.writeHead(404, { 'cache-control': 'no-store' }); return res.end(); }
  const ext = extname(file);
  const encode = quality && compressible.has(ext) && /\bbr\b/.test(req.headers['accept-encoding'] || '');
  const data = body(file, encode ? quality : 0);
  const headers = { 'content-type': types[ext] || 'application/octet-stream', 'cache-control': 'public, max-age=31536000, immutable', 'content-length': data.length };
  if (encode) headers['content-encoding'] = 'br';
  res.writeHead(200, headers); res.end(data);
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const origin = 'http://127.0.0.1:' + server.address().port;

class Cdp {
  constructor(ws) { this.ws = ws; this.id = 0; this.pending = new Map(); this.listeners = new Set();
    ws.onmessage = e => { const m = JSON.parse(e.data); if (m.id) { const p = this.pending.get(m.id); this.pending.delete(m.id); m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result); } else for (const l of this.listeners) l(m); }; }
  send(method, params = {}, sessionId) { const id = ++this.id; this.ws.send(JSON.stringify({ id, method, params, sessionId })); return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject })); }
  on(fn) { this.listeners.add(fn); return () => this.listeners.delete(fn); }
}
async function launch(profile) {
  const proc = spawn(chrome, ['--headless=new', '--no-sandbox', '--remote-debugging-port=0', '--user-data-dir=' + profile,
    '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-component-update',
    '--window-size=1280,900', 'about:blank'], { stdio: ['ignore', 'ignore', 'pipe'] });
  const wsUrl = await new Promise((resolve, reject) => { let b = ''; proc.stderr.on('data', c => { b += c; const m = /DevTools listening on (ws:\/\/\S+)/.exec(b); if (m) resolve(m[1]); }); proc.on('exit', c => reject(new Error('exit ' + c))); });
  const ws = new WebSocket(wsUrl); await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  return { proc, cdp: new Cdp(ws) };
}
const PROBE = '(() => { const p = window.__perf473 = { marker: null, wasm: [], fcp: null }; new MutationObserver(() => { if (p.marker == null && document.documentElement?.hasAttribute("data-hydrated")) p.marker = performance.now(); }).observe(document, { attributes: true, subtree: true, attributeFilter: ["data-hydrated"] }); const o = WebAssembly.instantiateStreaming; WebAssembly.instantiateStreaming = function (...a) { const t0 = performance.now(); const r = o.apply(this, a); r.then(() => p.wasm.push({ t0, t1: performance.now() })); return r; }; new PerformanceObserver(l => { for (const e of l.getEntries()) if (e.name === "first-contentful-paint") p.fcp = e.startTime; }).observe({ type: "paint", buffered: true }); })();';

const conditions = JSON.parse(opt.conditions);
// Encode every compressible file before the first browser load, so server-side Brotli time is
// never part of a measured sample.
{
  const { readdirSync, statSync } = await import('node:fs');
  const walk = d => readdirSync(d).flatMap(n => { const p = join(d, n); return statSync(p).isDirectory() ? walk(p) : [p]; });
  for (const q of (opt.modes || '0,5,11').split(',').map(Number)) for (const f of walk(join(root, '_v'))) if (compressible.has(extname(f))) body(f, q);
}
const modes = (opt.modes || '0,5,11').split(',').map(Number);
const cpu = Number(opt.cpu || 1);
const runs = [];
for (const net of conditions) for (const q of modes) for (let i = 0; i < samples; i++) {
  quality = q;
  const profile = mkdtempSync(join(tmpdir(), 'vink-473c-'));
  const b = await launch(profile);
  try {
    const { targetId } = await b.cdp.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await b.cdp.send('Target.attachToTarget', { targetId, flatten: true });
    const send = (m, p) => b.cdp.send(m, p, sessionId);
    let bytes = 0; const done = new Set();
    b.cdp.on(m => { if (m.sessionId === sessionId && m.method === 'Network.loadingFinished') bytes += m.params.encodedDataLength; });
    await send('Page.enable'); await send('Network.enable'); await send('Runtime.enable');
    await send('Network.clearBrowserCache');
    await send('Network.emulateNetworkConditions', { offline: false, latency: net.latencyMs,
      downloadThroughput: net.downMbps * 125000, uploadThroughput: net.upMbps * 125000 });
    if (cpu > 1) { await send('Emulation.setCPUThrottlingRate', { rate: cpu }); await send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 3, mobile: true }); }
    await send('Page.addScriptToEvaluateOnNewDocument', { source: PROBE });
    await send('Page.navigate', { url: origin + '/' });
    const start = Date.now(); let res = null;
    while (Date.now() - start < 120000) {
      await sleep(100);
      const { result } = await send('Runtime.evaluate', { expression: 'JSON.stringify(window.__perf473 && window.__perf473.marker != null ? { p: window.__perf473, n: performance.getEntriesByType("navigation")[0].toJSON() } : null)', returnByValue: true });
      if (result.value && result.value !== 'null') { res = JSON.parse(result.value); break; }
    }
    await sleep(500);
    runs.push({ network: net.name, quality: q, cpu, sample: i, bytes, fcp: res?.p.fcp ?? null, hydrated: res?.p.marker ?? null,
      wasm: res?.p.wasm?.[0] ?? null, ttfb: res?.n.responseStart ?? null });
    process.stderr.write(net.name + ' q' + q + ' #' + i + ' hydrated=' + (res?.p.marker?.toFixed(0)) + ' bytes=' + bytes + '\n');
  } finally {
    await b.cdp.send('Browser.close').catch(() => {}); await sleep(500); b.proc.kill('SIGKILL');
    rmSync(profile, { recursive: true, force: true });
  }
}
server.close();
writeFileSync(opt.out, JSON.stringify({ origin: 'loopback', cpu, conditions, modes, samples, precompressed_before_samples: true, runs }));
