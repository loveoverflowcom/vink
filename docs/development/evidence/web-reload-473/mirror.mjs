#!/usr/bin/env node
// #473: copies the public bytes a guest cold load fetched (HTML, anonymous session answer and
// every /_v/<sha>/ asset in the first cold ledger) into a directory for compression-control.mjs.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { gunzipSync } from 'node:zlib';

const opt = Object.fromEntries(process.argv.slice(2).join(' ').split(/\s*--/).filter(Boolean)
  .map(pair => { const [k, ...v] = pair.split(' '); return [k, v.join(' ') || 'true']; }));
const origin = opt.origin || 'https://vink.forum';
const raw = readFileSync(opt.ledger);
const matrix = JSON.parse((opt.ledger.endsWith('.gz') ? gunzipSync(raw) : raw).toString());
const cold = matrix.runs[0].loads.find(l => l.label === 'cold');
const paths = [...new Set(cold.requests.map(r => r.url).filter(u => u?.startsWith('/_v/')).map(u => u.replace(/\?.*$/, '')))];
const get = async path => { const r = await fetch(origin + path, { headers: { 'accept-encoding': 'br, gzip' } });
  if (!r.ok) throw new Error(path + ' ' + r.status); return Buffer.from(await r.arrayBuffer()); };
mkdirSync(opt.out, { recursive: true });
writeFileSync(join(opt.out, 'index.html'), await get('/'));
writeFileSync(join(opt.out, 'session.json'), await get('/api/v1/auth/session'));
for (const path of paths) { mkdirSync(dirname(join(opt.out, path)), { recursive: true }); writeFileSync(join(opt.out, path), await get(path)); }
console.log(JSON.stringify({ origin, files: paths.length + 2 }));
