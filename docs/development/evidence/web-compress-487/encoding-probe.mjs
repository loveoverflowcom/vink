#!/usr/bin/env node
// #487: read-only guest probe of one release's assets through the production edge. For each file it
// compares the versioned path (/_v/<sha>/pkg/) with the bootstrap alias (/pkg/) under three
// Accept-Encoding values plus If-None-Match (strong and W/). Both paths use the same Worker branch
// (X-Vink-Asset-Route: inventory/r26, env.ASSETS.fetch without client headers); only Cache-Control
// differs, which isolates no-transform. Records status, sizes, selected headers and whether the
// decoded body hashes to the ETag. No cookie, body or other header value is written.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { brotliDecompressSync, gunzipSync, zstdDecompressSync } from 'node:zlib';

const opt = Object.fromEntries(process.argv.slice(2).join(' ').split(/\s*--/).filter(Boolean)
  .map(pair => { const [k, ...v] = pair.split(' '); return [k, v.join(' ') || 'true']; }));
const origin = opt.origin || 'https://vink.forum';
const files = (opt.files || 'vink_bg.wasm,vink.css,vink.js,fonts/literata-variable.woff2').split(',');
const dir = mkdtempSync(join(tmpdir(), 'vink-487-probe-'));
const KEEP = ['content-type', 'content-encoding', 'cache-control', 'etag', 'vary', 'x-vink-asset-route',
  'x-vink-asset-revision', 'cf-cache-status'];

function curl(url, headers) {
  const out = join(dir, 'body');
  const raw = execFileSync('curl', ['-sS', '--http2', '-o', out, '-D', '-', '-w', '\n%{size_download} %{http_code}',
    ...headers.flatMap(h => ['-H', h]), url]).toString();
  const head = {};
  for (const line of raw.split(/\r?\n/)) { const m = /^([A-Za-z0-9-]+):\s*(.*)$/.exec(line); if (m && KEEP.includes(m[1].toLowerCase())) head[m[1].toLowerCase()] = m[2]; }
  const [size, status] = raw.trim().split('\n').at(-1).split(' ').map(Number);
  return { status, size, head, body: readFileSync(out) };
}
const decode = (body, encoding) => encoding === 'br' ? brotliDecompressSync(body) : encoding === 'gzip' ? gunzipSync(body)
  : encoding === 'zstd' ? zstdDecompressSync(body) : body;

try {
  const html = execFileSync('curl', ['-sS', '--compressed', origin + '/']).toString();
  const release = /\/_v\/([0-9a-f]{40})\/pkg\/vink\.js/.exec(html)?.[1];
  if (!release) throw Error('release namespace not found in HTML');
  const rows = [];
  for (const file of files) for (const [kind, path] of [['versioned', '/_v/' + release + '/pkg/' + file], ['alias', '/pkg/' + file]]) {
    for (const accept of ['br, gzip, zstd', 'gzip', 'identity']) {
      const r = curl(origin + path, ['Accept-Encoding: ' + accept]);
      const encoding = r.head['content-encoding'] || 'identity';
      const decoded = r.status === 200 ? decode(r.body, encoding) : Buffer.alloc(0);
      const digest = createHash('sha256').update(decoded).digest('hex');
      const tag = r.head.etag || '';
      const row = { file, kind, accept, status: r.status, wireBytes: r.size, decodedBytes: decoded.length, encoding,
        cacheControl: r.head['cache-control'] ?? null, vary: r.head.vary ?? null, weakEtag: tag.startsWith('W/'),
        etagIsDecodedSha256: tag.replace(/^W\//, '') === '"' + digest + '"', route: r.head['x-vink-asset-route'] ?? null,
        revisionMatches: r.head['x-vink-asset-revision'] === undefined ? null : r.head['x-vink-asset-revision'].length === 40,
        cfCacheStatus: r.head['cf-cache-status'] ?? null, contentType: r.head['content-type'] ?? null };
      for (const condition of [tag.replace(/^W\//, ''), 'W/' + tag.replace(/^W\//, '')]) {
        row[condition.startsWith('W/') ? 'ifNoneMatchWeak' : 'ifNoneMatchStrong'] =
          curl(origin + path, ['Accept-Encoding: ' + accept, 'If-None-Match: ' + condition]).status;
      }
      rows.push(row);
      process.stderr.write([file, kind, accept, r.status, r.size, encoding].join(' ') + '\n');
    }
  }
  writeFileSync(opt.out, JSON.stringify({ origin, release, measuredAt: new Date().toISOString(),
    curl: execFileSync('curl', ['--version']).toString().split('\n')[0], rows }, null, 1) + '\n');
} finally { rmSync(dir, { recursive: true, force: true }); }
