#!/usr/bin/env node
// #473: median (min–max) per access kind for reload-matrix.mjs output and per network/encoding
// for compression-control.mjs output. Lab numbers from one workstation, not RUM or an SLA.
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';

const read = f => JSON.parse((f.endsWith('.gz') ? gunzipSync(readFileSync(f)) : readFileSync(f)).toString());
export const dist = xs => {
  const s = xs.filter(Number.isFinite).sort((a, b) => a - b); if (!s.length) return null;
  const n = s.length, m = n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2;
  return { n, median: +m.toFixed(1), min: +s[0].toFixed(1), max: +s[n - 1].toFixed(1) };
};
const fmt = d => d ? d.median + ' (' + d.min + '–' + d.max + ')' : '—';
const STATIC = ['Script', 'Stylesheet', 'Font', 'Image', 'Other', 'Manifest'];

export function summarizeMatrix(data) {
  const rows = {};
  for (const run of data.runs) for (const l of run.loads) {
    const r = rows[l.label] ??= { kind: new Set(), ttfb: [], fcp: [], ready: [], wasmStart: [], wasmMs: [], hydrated: [], afterWasm: [],
      longtask: [], bytes: [], cached: [], session: [], spaUrl: [], spaQuiet: [], spaRequests: [], bfcache: new Set(), errors: [] };
    if (l.spa) {
      if (l.spa.error) { r.errors.push(l.spa.error); continue; }
      r.kind.add('same-document'); r.spaUrl.push(l.spa.urlMs); r.spaQuiet.push(l.spa.domQuietMs);
      r.spaRequests.push(l.requests.filter(q => q.url && !q.url.startsWith('data:')).length); continue;
    }
    const p = l.page, probe = p.probe || {}, nav = p.nav || {}, wasm = (probe.wasm || [])[0];
    r.kind.add(nav.type); r.ttfb.push(nav.ttfb); r.fcp.push(probe.fcp); r.ready.push(probe.ready); r.hydrated.push(probe.marker);
    if (wasm) { r.wasmStart.push(wasm.t0); r.wasmMs.push(wasm.t1 - wasm.t0); if (probe.marker != null) r.afterWasm.push(probe.marker - wasm.t1); }
    r.longtask.push((probe.longtasks || []).reduce((a, [, d]) => a + d, 0));
    r.bytes.push(l.requests.reduce((a, q) => a + (q.encodedDataLength || 0), 0));
    const sub = l.requests.filter(q => q.method === 'GET' && q.url && !q.url.startsWith('data:') && STATIC.includes(q.type));
    r.cached.push(sub.length ? sub.filter(q => q.fromDiskCache || q.servedFromCache).length / sub.length : null);
    const session = l.requests.find(q => q.url === '/api/v1/auth/session');
    if (session?.timing) r.session.push(session.timing.headers - session.timing.send);
    if (l.bfcache) r.bfcache.add(l.bfcache.reasons.join(' + '));
    if (!l.settled) r.errors.push('not settled');
  }
  return Object.fromEntries(Object.entries(rows).map(([k, r]) => [k, Object.fromEntries(Object.entries(r).map(([m, v]) =>
    [m, v instanceof Set ? [...v] : m === 'errors' ? v : dist(v)]))]));
}

export function summarizeControl(data) {
  const rows = {};
  for (const r of data.runs) {
    const key = r.network + ' · CPU ' + r.cpu + '× · ' + (r.quality ? 'br q' + r.quality : 'identity');
    const row = rows[key] ??= { fcp: [], wasmEnd: [], hydrated: [], bytes: [] };
    row.fcp.push(r.fcp); row.wasmEnd.push(r.wasm?.t1); row.hydrated.push(r.hydrated); row.bytes.push(r.bytes);
  }
  return Object.fromEntries(Object.entries(rows).map(([k, r]) => [k, Object.fromEntries(Object.entries(r).map(([m, v]) => [m, dist(v)]))]));
}

if (import.meta.url === 'file://' + process.argv[1]) {
  for (const f of process.argv.slice(2)) {
    const data = read(f);
    if (data.conditions) {
      console.log('\n# ' + f.split('/').pop() + ' — loopback, cold cache, n=' + data.samples + '/row');
      console.log('| row | FCP ms | WASM ready ms | hydrated ms | bytes |\n|---|---|---|---|---|');
      for (const [k, r] of Object.entries(summarizeControl(data))) console.log('| ' + [k, fmt(r.fcp), fmt(r.wasmEnd), fmt(r.hydrated), fmt(r.bytes)].join(' | ') + ' |');
      continue;
    }
    console.log('\n# ' + f.split('/').pop() + ' — ' + data.chrome + ', ' + data.viewport + ', CPU ' + data.cpuThrottle + '×, n=' + data.samples);
    console.log('| kind | nav | TTFB | FCP | ready selector | WASM start | WASM instantiate | hydrated | hydrate after WASM | long task Σ | bytes | static from cache | session wait | notes |');
    console.log('|---|---|---|---|---|---|---|---|---|---|---|---|---|---|');
    for (const [k, r] of Object.entries(summarizeMatrix(data))) {
      if (r.spaUrl) { console.log('| ' + [k, r.kind.join(','), 'URL ' + fmt(r.spaUrl), 'DOM quiet ' + fmt(r.spaQuiet), '', '', '', '', '', '', 'requests ' + fmt(r.spaRequests), '', '', r.errors.join('; ')].join(' | ') + ' |'); continue; }
      console.log('| ' + [k, r.kind.join(','), fmt(r.ttfb), fmt(r.fcp), fmt(r.ready), fmt(r.wasmStart), fmt(r.wasmMs), fmt(r.hydrated), fmt(r.afterWasm), fmt(r.longtask), fmt(r.bytes), fmt(r.cached), fmt(r.session), [...r.bfcache, ...r.errors].join('; ')].join(' | ') + ' |');
    }
  }
}
