// Loopback host for the standalone artifact. Only API/probes go to the local Rust service.
import {createServer, request as proxy} from 'node:http';
import {createReadStream} from 'node:fs';
import {stat} from 'node:fs/promises';
import {resolve, extname} from 'node:path';
import {spaRoute} from '../../infra/cloudflare/production/worker.mjs';

const [site = 'target/spa', portArg = '8080', apiPortArg = '3000'] = process.argv.slice(2);
function port(value) {
  if (!/^\d+$/.test(value) || +value < 1 || +value > 65535) throw Error('Expected TCP port');
  return +value;
}
const listenPort = port(portArg);
const apiPort = port(apiPortArg);
if (listenPort === apiPort) throw Error('Frontend and API require separate ports');
const host = '127.0.0.1';
const types = {'.js':'text/javascript', '.css':'text/css', '.wasm':'application/wasm',
  '.woff2':'font/woff2', '.svg':'image/svg+xml', '.webp':'image/webp', '.png':'image/png',
  '.webmanifest':'application/manifest+json', '.txt':'text/plain', '.html':'text/html; charset=utf-8'};

createServer(async (req, res) => {
  const fail = (status, message) => {
    res.writeHead(status, {'cache-control':'no-store', 'content-type':'text/plain; charset=utf-8'});
    res.end(message);
  };
  if (req.headers.host !== host + ':' + listenPort || !req.url.startsWith('/') || req.url.startsWith('//')) {
    return fail(400, 'Invalid local host or path');
  }
  const url = new URL(req.url, 'http://' + req.headers.host);
  const route = spaRoute(url.pathname, req.method, req.headers.accept);
  if (route === 'api') {
    const upstream = proxy({hostname: host, port: apiPort, path: url.pathname + url.search,
      method: req.method, headers: {...req.headers, host: host + ':' + apiPort}}, reply => {
      res.writeHead(reply.statusCode, reply.headers);
      reply.pipe(res);
      reply.on('error', () => res.destroy());
    });
    upstream.on('error', () => res.headersSent ? res.destroy() : fail(502, 'API unavailable'));
    res.on('close', () => upstream.destroy());
    req.pipe(upstream);
    return;
  }
  if (!['GET', 'HEAD'].includes(req.method)) return fail(405, 'Read-only');
  const asset = /^\/_v\/[0-9a-f]{40}\/pkg\/(?:[a-zA-Z0-9_-]+\/)*[\w.-]+$/.test(url.pathname);
  if (!asset && route !== 'document') return fail(404, 'Not found');
  const file = resolve(site, asset ? '.' + url.pathname : 'index.html');
  try {
    const info = await stat(file);
    if (!info.isFile()) return fail(404, 'Not found');
    res.writeHead(200, {'content-type': types[extname(file)] ?? 'application/octet-stream',
      'content-length': info.size, 'cache-control': asset ? 'public, max-age=31536000, immutable' : 'no-store'});
    if (req.method === 'HEAD') res.end();
    else createReadStream(file).on('error', () => res.destroy()).pipe(res);
  } catch { fail(404, 'Not found'); }
}).listen(listenPort, host, () => console.log('Vink SPA http://' + host + ':' + listenPort));
