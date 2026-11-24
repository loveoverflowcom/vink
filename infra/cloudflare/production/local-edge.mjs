// Qualification only. A loopback browser front door and scratch origin stand in for DNS/TLS/Caddy.
// No subrequest may leave loopback. The deployed Worker module runs unchanged in workerd.
import { Miniflare, convertV4MiniflareOptions, Response as MfResponse } from 'miniflare';
import { createServer } from 'node:http';
import { Readable } from 'node:stream';
import { resolve } from 'node:path';
import { readFileSync } from 'node:fs';

const [staged, originArg, portArg] = process.argv.slice(2);
const local = new URL(originArg);
if (local.protocol !== 'http:' || local.hostname !== '127.0.0.1' || local.pathname !== '/') {
  throw Error('Use a dedicated loopback scratch origin');
}
const mf = new Miniflare(convertV4MiniflareOptions({
  modules: true, script: readFileSync(resolve(staged, '_worker.js'), 'utf8'),
  compatibilityDate: '2026-10-01', host: '127.0.0.1', cf: false,
  bindings: { ORIGIN_KEY: 'local-edge-scratch' },
  assets: { directory: resolve(staged), binding: 'ASSETS', run_worker_first: true,
    routerConfig: { has_user_worker: true },
    assetConfig: { html_handling: 'auto-trailing-slash', not_found_handling: 'none' } },
  outboundService: async req => {
    const url = new URL(req.url);
    if (url.origin !== 'https://origin-prod.vink.forum' || req.headers.get('x-vink-origin-key') !== 'local-edge-scratch') {
      return new MfResponse(null, { status: 502 });
    }
    const headers = new Headers(req.headers);
    headers.delete('host');
    headers.delete('x-vink-origin-key');
    headers.set('x-forwarded-for', headers.get('x-vink-client-ip'));
    const upstream = new URL(local);
    upstream.pathname = url.pathname;
    upstream.search = url.search;
    const r = await fetch(upstream, {method: req.method, body: req.body, duplex:'half', headers, redirect:'manual'});
    return new MfResponse(r.body, {status:r.status, headers:r.headers});
  },
}));
await mf.ready;
const server = createServer(async (req, res) => {
  try {
    const url = new URL('https://vink.forum');
    const path = new URL(req.url, 'http://localhost');
    url.pathname = path.pathname;
    url.search = path.search;
    const r = await mf.dispatchFetch(url.href, {
      method:req.method, headers:{...req.headers, host:'vink.forum', 'cf-connecting-ip':'192.0.2.2'},
      body:['GET','HEAD'].includes(req.method) ? undefined : Readable.toWeb(req), duplex:'half', redirect:'manual',
    });
    for (const [name,value] of r.headers) if (name !== 'set-cookie') res.setHeader(name,value);
    const cookies = r.headers.getSetCookie();
    if (cookies.length) res.setHeader('set-cookie', cookies);
    res.statusCode=r.status;
    if (r.body) Readable.fromWeb(r.body).pipe(res); else res.end();
  } catch (error) { console.error(error.message); res.writeHead(502).end(); }
});
server.listen(Number(portArg), '127.0.0.1', () => console.log('READY local workerd'));
async function stop() { server.close(); await mf.dispose(); }
process.once('SIGTERM', stop);
process.once('SIGINT', stop);
