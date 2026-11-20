import test from 'node:test';
import assert from 'node:assert/strict';
import { handler } from './worker.mjs';

const rev = 'a'.repeat(40), path = `/_v/${rev}/pkg/vink.js`;
const index = { [path]: { sha256: '1'.repeat(64), type: 'text/javascript' } };
const env = { ORIGIN_KEY: 'scratch-only', ASSETS: { fetch(request) {
  assert.equal(request.headers.has('cookie'), false);
  return new Response('js', { headers: { 'set-cookie': 'discard' } });
} } };
const send = (serve, pathname, options = {}, bindings = env) => serve(new Request('https://vink.forum' + pathname,
  { ...options, headers: { 'cf-connecting-ip': '192.0.2.2', ...options.headers } }), bindings);

test('inventory and retained fallback read only static bytes', async () => {
  const serve = handler(index, assert.fail, req => {
    assert.equal(new URL(req.url).host, 'r-' + 'b'.repeat(40) + '.vink-production.pages.dev');
    assert.equal(req.headers.has('cookie'), false);
    return new Response('B', { headers: { 'x-vink-asset-revision': 'b'.repeat(40) } });
  });
  const js = await send(serve, path, { headers: { cookie: 'one' } });
  assert.equal(await js.text(), 'js');
  assert.equal(js.headers.get('set-cookie'), null);
  assert.equal(js.headers.get('content-type'), 'text/javascript');
  assert.match(js.headers.get('cache-control'), /immutable/);
  assert.equal((await send(serve, path, { headers: { 'if-none-match': js.headers.get('etag') } })).status, 304);
  assert.equal((await send(serve, path, { method: 'HEAD' })).body, null);
  assert.equal(await (await send(serve, path.replace(rev, 'b'.repeat(40)))).text(), 'B');
  assert.equal((await send(serve, '/pkg/absent.wasm')).status, 404);
  assert.equal((await send(serve, path, { method: 'POST', body: 'private' })).status, 405);
  assert.equal((await send(serve, path, {}, { ASSETS: { fetch: () => new Response('<html>', {headers:{'content-type':'text/html'}}) } })).status, 502);
  assert.equal((await send(serve, path, {}, { ASSETS: { fetch: () => { throw Error(); } } })).status, 502);
});

test('canonical host and dynamic request preserve contracts and isolate private reads', async () => {
  const calls = [];
  const serve = handler({}, async (req, options) => {
    calls.push(req);
    assert.equal(options.cache, 'no-store');
    assert.equal(req.headers.get('x-vink-origin-key'), env.ORIGIN_KEY);
    assert.equal(req.headers.get('x-vink-client-ip'), '192.0.2.2');
    assert.equal(req.headers.get('x-forwarded-for'), null);
    assert.equal(req.redirect, 'manual');
    if (req.method === 'POST') {
      assert.equal(req.url, 'https://origin-prod.vink.forum/api/v1/auth/login?v=1');
      assert.equal(req.headers.get('origin'), 'https://vink.forum');
      assert.equal(await req.text(), 'body');
      return new Response(null, {status:303, headers:{location:'/toi', 'set-cookie':'one=1; Secure'}});
    }
    if (new URL(req.url).pathname === '/api/v1/works') return new Response(null, {
      status:304, headers:{etag:'"public"', 'cache-control':'private, no-cache'} });
    return new Response(req.headers.get('cookie') || 'guest', {headers:{'cache-control':'public', 'x-robots-tag':'noindex'}});
  });
  for (const cookie of ['a=one','b=two','']) {
    const r = await send(serve, '/api/v1/me/profile', { headers:{cookie, 'x-forwarded-for':'invented', 'x-vink-client-ip':'invented'} });
    assert.equal(await r.text(), cookie || 'guest');
    assert.equal(r.headers.get('cache-control'), 'no-store');
    assert.equal(r.headers.get('x-robots-tag'), 'noindex');
  }
  const write = await send(serve, '/api/v1/auth/login?v=1', {method:'POST',body:'body',headers:{origin:'https://vink.forum'}});
  assert.equal(write.status,303);
  assert.equal(write.headers.get('location'),'/toi');
  assert.match(write.headers.get('set-cookie'),/one=1/);
  const pub = await send(serve,'/api/v1/works',{headers:{'if-none-match':'"public"'}});
  assert.equal(pub.status,304);
  assert.equal(pub.headers.get('etag'),'"public"');
  assert.equal(pub.headers.get('x-robots-tag'),null);
  assert.equal((await serve(new Request('https://www.vink.forum//other.example/x?q=1'),env)).headers.get('location'),'https://vink.forum//other.example/x?q=1');
  assert.equal((await serve(new Request('https://preview.pages.dev/toi'),env)).status,404);
  assert.equal((await send(serve,'/',{headers:{'x-vink-edge-hop':'1'}})).status,508);
  assert.equal((await send(handler({},()=>{throw Error()}),'/')).status,502);
  assert.equal(calls.length,5);
});

test('retirement returns 410 for old URLs and sensitive methods without redirect', async () => {
  const {default: retired} = await import('./test-retired.mjs');
  for (const method of ['GET', 'HEAD', 'POST', 'PUT', 'DELETE']) {
    const response = retired.fetch(new Request('https://test.vink.forum/api/v1/me/profile', {method}));
    assert.equal(response.status, 410);
    assert.equal(response.headers.get('location'), null);
    assert.equal(response.headers.get('set-cookie'), null);
    if (method === 'HEAD') assert.equal(response.body, null);
  }
});
