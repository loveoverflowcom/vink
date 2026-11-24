#!/usr/bin/env node
// #473: request→first-byte by hop on reused HTTP/2 connections, from the measuring machine.
// Edge only, Worker+Pages asset, Worker+origin without and with App/DB/SSR work.
// Records status, timing and wire size only; /cdn-cgi/trace contributes colo/loc/http/tls.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const opt = Object.fromEntries(process.argv.slice(2).join(' ').split(/\s*--/).filter(Boolean)
  .map(pair => { const [k, ...v] = pair.split(' '); return [k, v.join(' ') || 'true']; }));
const origin = opt.origin || 'https://vink.forum';
const rounds = Number(opt.rounds || 15);
const endpoints = {
  edge: '/cdn-cgi/trace',
  asset: '/_v/' + opt.sha + '/pkg/brand/vpen-r1-mark-light.svg',
  live: '/health/live',
  session: '/api/v1/auth/session',
  tags: '/api/v1/tags',
  works: '/api/v1/works',
  home_ssr: '/',
  desk_ssr: '/ban-viet',
  me_ssr: '/toi',
};
const dir = mkdtempSync(join(tmpdir(), 'vink-473-edge-'));
try {
  const lines = [];
  for (let i = 0; i < rounds; i++) for (const path of Object.values(endpoints)) lines.push('url = "' + origin + path + '"', 'output = "/dev/null"');
  writeFileSync(join(dir, 'curl.cfg'), lines.join('\n') + '\n');
  const out = execFileSync('curl', ['-sS', '--http2', '-H', 'Accept-Encoding: br, gzip', '-K', join(dir, 'curl.cfg'), '-w',
    '%{url_effective}\t%{http_code}\t%{time_pretransfer}\t%{time_starttransfer}\t%{size_download}\t%{num_connects}\n'], { encoding: 'utf8', maxBuffer: 1 << 26 });
  const trace = Object.fromEntries(execFileSync('curl', ['-sS', origin + '/cdn-cgi/trace'], { encoding: 'utf8' })
    .split('\n').map(l => l.split('=')).filter(([k]) => ['colo', 'loc', 'http', 'tls'].includes(k)));
  const rows = {};
  for (const line of out.trim().split('\n')) {
    const [url, code, pre, start, size, connects] = line.split('\t');
    const name = Object.entries(endpoints).find(([, p]) => url === origin + p)?.[0] ?? 'unknown';
    (rows[name] ??= []).push({ code, waitMs: +((start - pre) * 1000).toFixed(1), bytes: +size, newConnection: +connects > 0 });
  }
  const median = xs => { const s = [...xs].sort((a, b) => a - b), n = s.length; return n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2; };
  const summary = Object.fromEntries(Object.entries(rows).map(([name, r]) => {
    const w = r.filter(x => !x.newConnection).map(x => x.waitMs);
    return [name, { path: endpoints[name], codes: [...new Set(r.map(x => x.code))], n: w.length, median: median(w), min: Math.min(...w), max: Math.max(...w), wireBytes: r[0].bytes, waits: w }];
  }));
  const result = { at: new Date().toISOString(), origin, rounds, trace, endpoints: summary };
  if (opt.out) writeFileSync(opt.out, JSON.stringify(result, null, 1)); else console.log(JSON.stringify(result, null, 1));
} finally { rmSync(dir, { recursive: true, force: true }); }
