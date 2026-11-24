#!/usr/bin/env node
// #487: median (min–max) per access kind for edge-replay.mjs output, and the production probe table
// for encoding-probe.mjs output. Lab numbers from one workstation, not RUM or an SLA.
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';

const read = f => JSON.parse((f.endsWith('.gz') ? gunzipSync(readFileSync(f)) : readFileSync(f)).toString());
const dist = xs => {
  const s = xs.filter(Number.isFinite).sort((a, b) => a - b); if (!s.length) return null;
  const n = s.length, m = n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2;
  return { n, median: m, min: s[0], max: s[n - 1] };
};
const ms = d => d ? Math.round(d.median) + ' (' + Math.round(d.min) + '–' + Math.round(d.max) + ')' : '—';
const mb = d => d ? (d.median >= 1e5 ? (d.median / 1e6).toFixed(2) + ' MB' : (d.median / 1e3).toFixed(1) + ' KB') : '—';
const pct = d => d ? Math.round(d.median * 100) + '%' : '—';
const STATIC = ['Script', 'Stylesheet', 'Font', 'Image', 'Other', 'Manifest', 'Fetch'];

export function summarize(data) {
  const rows = {};
  for (const run of data.runs) for (const l of run.loads) {
    const r = rows[l.label] ??= { ttfb: [], fcp: [], hydrated: [], wasmMs: [], longtask: [], bytes: [], cached: [], wasmBytes: [],
      encodings: new Set(), ok: 0, n: 0, errors: [], offline: new Set() };
    r.n++;
    if (l.offline) { r.offline.add(l.offline.url + (l.offline.hydrated ? ' hydrated' : '')); continue; }
    const probe = l.page.probe || {}, wasm = (probe.wasm || [])[0];
    r.ttfb.push(l.page.nav?.ttfb); r.fcp.push(probe.fcp); r.hydrated.push(probe.marker);
    if (wasm) r.wasmMs.push(wasm.t1 - wasm.t0);
    r.longtask.push((probe.longtasks || []).reduce((a, [, d]) => a + d, 0));
    r.bytes.push(l.requests.reduce((a, q) => a + (q.encodedDataLength || 0), 0));
    const sub = l.requests.filter(q => q.method === 'GET' && q.url?.startsWith('/_v/') && STATIC.includes(q.type));
    r.cached.push(sub.length ? sub.filter(q => q.fromDiskCache || q.servedFromCache).length / sub.length : null);
    const w = l.requests.filter(q => q.url?.endsWith('/vink_bg.wasm'));
    r.wasmBytes.push(w.reduce((a, q) => a + (q.encodedDataLength || 0), 0));
    for (const q of w) r.encodings.add((q.fromDiskCache || q.servedFromCache) ? 'cache' : (q.encoding || 'identity'));
    if (l.settled && l.page.hydrated && wasm && !wasm.error) r.ok++;
    r.errors.push(...l.errors);
  }
  return rows;
}

if (import.meta.url === 'file://' + process.argv[1]) {
  for (const f of process.argv.slice(2)) {
    const data = read(f);
    if (data.rows) {
      console.log('\n# ' + f.split('/').pop() + ' — ' + data.origin + ', release ' + data.release.slice(0, 8) + ', ' + data.measuredAt);
      console.log('| file | path | Accept-Encoding | status | Content-Encoding | wire bytes | decoded bytes | Cache-Control | Vary | weak ETag | ETag = SHA-256(decoded) | 304 strong / W/ | route |');
      console.log('|---|---|---|---|---|---:|---:|---|---|---|---|---|---|');
      for (const r of data.rows) console.log('| ' + [r.file, r.kind, r.accept, r.status, r.encoding, r.wireBytes, r.decodedBytes, r.cacheControl, r.vary ?? '—',
        r.weakEtag ? 'yes' : 'no', r.etagIsDecodedSha256 ? 'yes' : 'NO', r.ifNoneMatchStrong + ' / ' + r.ifNoneMatchWeak, r.route].join(' | ') + ' |');
      continue;
    }
    console.log('\n# ' + f.split('/').pop() + ' — ' + data.chrome + ', ' + data.viewport + ', CPU ' + data.cpuThrottle + '×, ' +
      data.network.downMbps + ' Mbps / ' + data.network.latencyMs + ' ms, zone br q' + data.zone.brotliQuality + ' w' + data.zone.brotliWindow + ', worker ' + data.worker.sha256.slice(0, 12) + ', n=' + data.samples);
    console.log('| kind | TTFB | FCP | hydrated | WASM instantiate | long task Σ | bytes | WASM wire | WASM via | static from cache | hydrated OK | errors |');
    console.log('|---|---|---|---|---|---|---|---|---|---|---|---|');
    for (const [k, r] of Object.entries(summarize(data))) {
      if (r.offline.size) { console.log('| ' + k + ' | ' + [...r.offline].join(', ') + ' (' + r.n + '/' + r.n + ') |||||||||||'); continue; }
      console.log('| ' + [k, ms(dist(r.ttfb)), ms(dist(r.fcp)), ms(dist(r.hydrated)), ms(dist(r.wasmMs)), ms(dist(r.longtask)), mb(dist(r.bytes)),
        mb(dist(r.wasmBytes)), [...r.encodings].join('/'), pct(dist(r.cached)), r.ok + '/' + r.n, r.errors.length].join(' | ') + ' |');
    }
  }
}
