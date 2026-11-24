import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
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
    assert.equal(new URL(req.url).host, 'r-' + 'b'.repeat(26) + '.vink-production.pages.dev');
    assert.equal(new URL(req.url).pathname, `/_v/${'b'.repeat(40)}/pkg/vink-editor.js`);
    assert.equal(req.headers.has('cookie'), false);
    assert.equal(req.headers.has('authorization'), false);
    return new Response('B', { headers: { 'x-vink-asset-revision': 'b'.repeat(40) } });
  });
  const js = await send(serve, path, { headers: { cookie: 'one' } });
  assert.equal(await js.text(), 'js');
  assert.equal(js.headers.get('set-cookie'), null);
  assert.equal(js.headers.get('content-type'), 'text/javascript');
  assert.match(js.headers.get('cache-control'), /immutable/);
  assert.equal((await send(serve, path, { headers: { 'if-none-match': js.headers.get('etag') } })).status, 304);
  assert.equal((await send(serve, path, { method: 'HEAD' })).body, null);
  assert.equal(js.headers.get('x-vink-asset-route'), 'inventory/r26');
  const lazy = await send(serve, `/_v/${'b'.repeat(40)}/pkg/vink-editor.js`, {
    headers: { cookie: 'one', authorization: 'Bearer scratch' },
  });
  assert.equal(await lazy.text(), 'B');
  assert.equal(lazy.headers.get('x-vink-asset-route'), 'fallback/r26');
  assert.equal((await send(serve, '/pkg/absent.wasm')).status, 404);
  assert.equal((await send(serve, path, { method: 'POST', body: 'private' })).status, 405);
  assert.equal((await send(serve, path, {}, { ASSETS: { fetch: () => new Response('<html>', {headers:{'content-type':'text/html'}}) } })).status, 502);
  assert.equal((await send(serve, path, {}, { ASSETS: { fetch: () => { throw Error(); } } })).status, 502);
});

test('versioned assets let the zone compress text and WebAssembly but keep binary bytes exact', async () => {
  const types = {
    'vink_bg.wasm': 'application/wasm', 'vink.js': 'text/javascript', 'vink.css': 'text/css',
    'brand/vpen-r1-favicon.svg': 'image/svg+xml', 'brand/vpen-r1-site.webmanifest': 'application/manifest+json',
    'expressions/build.json': 'application/json', 'fonts/OFL-literata.txt': 'text/plain',
    'fonts/literata-variable.woff2': 'font/woff2', 'author-landscape.webp': 'image/webp',
    'brand/vpen-r1-icon-192.png': 'image/png',
  };
  const exact = new Set(['font/woff2', 'image/webp', 'image/png']);
  const inventory = {};
  for (const [name, type] of Object.entries(types)) {
    const entry = { sha256: createHash('sha256').update(name).digest('hex'), type };
    inventory[`/_v/${rev}/pkg/${name}`] = entry;
    inventory[`/pkg/${name}`] = { ...entry, path: `/_v/${rev}/pkg/${name}` };
  }
  const bindings = { ASSETS: { fetch(request) {
    // Pages never sees client headers, so it cannot pick an encoding the Worker would mislabel.
    assert.equal(request.headers.has('accept-encoding'), false);
    assert.equal(request.headers.has('if-none-match'), false);
    return new Response(new URL(request.url).pathname.split('/pkg/')[1]);
  } } };
  const serve = handler(inventory, assert.fail, assert.fail);
  for (const [name, type] of Object.entries(types)) {
    const path = `/_v/${rev}/pkg/${name}`, tag = '"' + inventory[path].sha256 + '"';
    const cache = 'public, max-age=31536000, immutable' + (exact.has(type) ? ', no-transform' : '');
    const vary = exact.has(type) ? null : 'Accept-Encoding';
    for (const encoding of ['br, gzip, zstd', 'gzip', 'identity', null]) {
      const headers = encoding ? { 'accept-encoding': encoding } : {};
      const response = await send(serve, path, { headers }, bindings);
      assert.equal(response.status, 200);
      // The Worker never encodes: the edge compresses, so one inventory body serves every variant.
      assert.equal(await response.text(), name);
      assert.equal(response.headers.get('content-encoding'), null);
      assert.equal(response.headers.get('content-type'), type);
      assert.equal(response.headers.get('cache-control'), cache);
      assert.equal(response.headers.get('vary'), vary);
      assert.equal(response.headers.get('etag'), tag);
      assert.equal(response.headers.get('x-vink-asset-revision'), rev);
    }
    for (const condition of [tag, 'W/' + tag, '"other", W/' + tag, '*']) {
      const unchanged = await send(serve, path, { headers: { 'if-none-match': condition, 'accept-encoding': 'br' } }, bindings);
      assert.equal(unchanged.status, 304);
      assert.equal(unchanged.body, null);
      assert.equal(unchanged.headers.get('etag'), tag);
      assert.equal(unchanged.headers.get('cache-control'), cache);
      assert.equal(unchanged.headers.get('vary'), vary);
    }
    assert.equal((await send(serve, path, { headers: { 'if-none-match': 'W/"' + '0'.repeat(64) + '"' } }, bindings)).status, 200);
    const head = await send(serve, path, { method: 'HEAD', headers: { 'accept-encoding': 'br' } }, bindings);
    assert.equal(head.body, null);
    assert.equal(head.headers.get('cache-control'), cache);
    const alias = await send(serve, `/pkg/${name}`, { headers: { 'accept-encoding': 'br' } }, bindings);
    assert.equal(alias.headers.get('cache-control'), 'no-cache');
    assert.equal(alias.headers.get('vary'), vary);
    assert.equal(alias.headers.get('x-vink-asset-revision'), rev);
  }
});

test('an open tab retains its lazy asset on both Workers during the bridge', async () => {
  const next = 'b'.repeat(40);
  const lazy = `/_v/${rev}/pkg/vink-editor.js`;
  const body = 'old lazy asset';
  const fallback = () => assert.fail('retained old lazy asset must come from inventory');
  const previous = { ...index, [lazy]: { sha256: createHash('sha256').update(body).digest('hex'), type: 'text/javascript' } };
  const after = { ...previous, [path.replace(rev, next)]: index[path] };
  const bindings = { ASSETS: { fetch: request => {
    assert.equal(new URL(request.url).pathname, lazy);
    return new Response(body);
  } } };
  for (const inventory of [previous, after]) {
    const response = await send(handler(inventory, assert.fail, fallback), lazy, {}, bindings);
    assert.equal(await response.text(), body);
    assert.equal(response.headers.get('x-vink-asset-revision'), rev);
  }
});

test('canonical host and dynamic request preserve contracts and isolate private reads', async () => {
  const calls = [];
  const publicBody = '{"works":[]}';
  const publicTag = '"' + createHash('sha256').update(publicBody).digest('hex').slice(0, 32) + '"';
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
    const url = new URL(req.url);
    if (url.pathname === '/api/v1/works') {
      const unchanged = req.headers.get('if-none-match') === publicTag;
      const headers = {etag:publicTag, 'cache-control':'private, no-cache'};
      if (url.searchParams.has('issue-cookie')) headers['set-cookie'] = 'new=1; Secure';
      return new Response(unchanged || req.method === 'HEAD' ? null : publicBody, {
        status: unchanged ? 304 : 200, headers,
      });
    }
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
  for (const encoding of ['identity', 'gzip', 'br']) {
    const pub = await send(serve, '/api/v1/works', {headers:{'accept-encoding':encoding}});
    assert.equal(pub.status, 200);
    assert.equal(await pub.text(), publicBody);
    assert.equal(pub.headers.get('etag'), publicTag);
    assert.equal(pub.headers.get('cache-control'), 'private, no-cache, no-transform');
    assert.equal(pub.headers.get('content-encoding'), null);
    assert.equal(pub.headers.get('x-robots-tag'), null);
    const unchanged = await send(serve, '/api/v1/works', {
      headers:{'accept-encoding':encoding, 'if-none-match':pub.headers.get('etag')},
    });
    assert.equal(unchanged.status, 304);
    assert.equal(await unchanged.text(), '');
    assert.equal(unchanged.headers.get('etag'), pub.headers.get('etag'));
    assert.equal(unchanged.headers.get('cache-control'), pub.headers.get('cache-control'));
  }
  for (const [pathname, headers] of [
    ['/api/v1/works', {cookie:'session=one'}],
    ['/api/v1/works', {authorization:'Bearer scratch'}],
    ['/api/v1/works?issue-cookie', {}],
  ]) {
    const privateRead = await send(serve, pathname, {headers});
    assert.equal(privateRead.headers.get('cache-control'), 'no-store');
  }
  assert.equal((await serve(new Request('https://www.vink.forum//other.example/x?q=1'),env)).headers.get('location'),'https://vink.forum//other.example/x?q=1');
  assert.equal((await serve(new Request('https://preview.pages.dev/toi'),env)).status,404);
  assert.equal((await send(serve,'/',{headers:{'x-vink-edge-hop':'1'}})).status,508);
  assert.equal((await send(handler({},()=>{throw Error()}),'/')).status,502);
  const offline = handler({}, () => new Response('Origin in maintenance', {status:503}));
  const waiting = await send(offline, '/api/v1/works', {headers:{'if-none-match':publicTag}});
  assert.equal(waiting.status, 503);
  assert.equal(waiting.headers.get('cache-control'), 'no-store');
  assert.equal(calls.length,13);
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

test('standalone documents work with the API offline and only API/probes reach origin', async () => {
  const apiCalls = [];
  const serve = handler({ ...index, '/': {type:'text/html'} }, req => {
    apiCalls.push(new URL(req.url).pathname);
    throw Error('offline');
  }, assert.fail);
  const bindings = { ...env, ASSETS: { fetch(req) {
    assert.equal(new URL(req.url).pathname, '/');
    assert.equal([...req.headers].length, 0);
    return new Response(req.method === 'HEAD' ? null : 'SPA', {headers: {'content-type': 'text/html'}});
  } } };
  for (const path of ['/', '/ban-viet/123', '/@tacgia', '/tac-pham/123', '/duong-dan-la']) {
    for (const method of ['GET', 'HEAD']) {
      const res = await send(serve, path, {method, headers: {accept: 'text/html', cookie: 'private'}}, bindings);
      assert.equal(res.status, 200);
      assert.equal(res.headers.get('cache-control'), 'no-store');
      assert.equal(await res.text(), method === 'GET' ? 'SPA' : '');
    }
  }
  for (const path of ['/api', '/api/v2/absent', '/health/unknown', '/pkg/not.js', '/missing.wasm', '/_v/missing']) {
    assert.equal((await send(serve, path, {headers: {accept: 'text/html'}}, bindings)).status, 404);
  }
  assert.equal((await send(serve, '/toi', {method:'POST'}, bindings)).status, 405);
  assert.equal((await send(serve, '/toi', {headers:{accept:'application/json'}}, bindings)).status, 404);
  assert.equal((await send(serve, '/api/v1/not-found', {}, bindings)).status, 502);
  assert.equal((await send(serve, '/health/ready', {}, bindings)).status, 502);
  assert.deepEqual(apiCalls, ['/api/v1/not-found', '/health/ready']);
});
