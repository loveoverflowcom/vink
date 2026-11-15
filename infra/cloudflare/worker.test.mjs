import test from "node:test";
import assert from "node:assert/strict";
import { handler } from "./worker.mjs";

const release = { origin: "https://test.vink.forum", assets: ["/pkg/vink.js"], origin_assets: ["/pkg/vink_bg.wasm"], asset_digest: "a".repeat(64) };
test("known assets use Pages; missing assets cannot become SSR HTML", async () => {
  const run = handler(release, () => assert.fail("must not call origin"));
  const env = { ASSETS: { fetch: () => new Response("js", { headers: { "Set-Cookie": "unexpected=x", "Access-Control-Allow-Origin": "*" } }) } };
  const asset = await run(new Request("https://preview.pages.dev/pkg/vink.js?x=1"), env);
  assert.equal(await asset.text(), "js");
  assert.equal(asset.headers.get("Cache-Control"), "public, max-age=0, must-revalidate");
  assert.equal(asset.headers.get("Set-Cookie"), null);
  assert.equal(asset.headers.get("Access-Control-Allow-Origin"), null);
  assert.equal((await run(new Request("https://preview.pages.dev/pkg/missing.wasm"), env)).status, 404);
  assert.equal((await run(new Request("https://preview.pages.dev/pkg/vink.js", { method: "POST" }), env)).status, 405);
});
test("dynamic proxy preserves Origin, cookies, body, query and private cache semantics", async () => {
  const run = handler(release, async (request, options) => {
    assert.equal(request.url, "https://test.vink.forum/api/v1/drafts?version=2");
    assert.equal(request.headers.get("Origin"), "https://preview.pages.dev");
    assert.equal(request.headers.get("Cookie"), "test=synthetic");
    assert.equal(await request.text(), "payload");
    assert.equal(request.headers.get("X-Forwarded-Host"), null);
    assert.equal(request.redirect, "manual");
    assert.equal(options.cache, "no-store");
    assert.equal(options.cf, undefined);
    return new Response("refused", { status: 403, headers: { "Cache-Control": "private, no-cache", ETag: '"v1"' } });
  });
  const response = await run(new Request("https://preview.pages.dev/api/v1/drafts?version=2", {
    method: "POST", body: "payload", headers: { Origin: "https://preview.pages.dev", Cookie: "test=synthetic", "X-Forwarded-Host": "evil.example" },
  }), {});
  assert.equal(response.status, 403);
  assert.equal(response.headers.get("Cache-Control"), "private, no-cache");
  assert.equal(response.headers.get("ETag"), '"v1"');
  assert.equal(response.headers.get("X-Robots-Tag"), "noindex, nofollow");
});
test("WASM ranges stream from origin and SSR gains no-store", async () => {
  const run = handler(release, async request => new Response("bytes", { status: 206, headers: { "Content-Type": "application/wasm", "Content-Range": "bytes 0-4/100" } }));
  const response = await run(new Request("https://preview.pages.dev/pkg/vink_bg.wasm", { headers: { Range: "bytes=0-4" } }), {});
  assert.equal(response.status, 206);
  assert.equal(response.headers.get("Content-Range"), "bytes 0-4/100");
  assert.equal(response.headers.get("Cache-Control"), "no-store");
});
test("loops fail closed; origin failures are real 502s", async () => {
  const run = handler(release, () => { throw new Error("offline"); });
  assert.equal((await run(new Request("https://test.vink.forum/"), {})).status, 508);
  assert.equal((await run(new Request("https://preview.pages.dev/", { headers: { "X-Vink-Edge-Hop": "1" } }), {})).status, 508);
  assert.equal((await run(new Request("https://preview.pages.dev/"), {})).status, 502);
});
test("a double slash path cannot select a foreign upstream", async () => {
  const run = handler(release, request => {
    assert.equal(new URL(request.url).host, "test.vink.forum");
    return new Response("ok");
  });
  assert.equal((await run(new Request("https://preview.pages.dev//evil.example/api"), {})).status, 200);
});
