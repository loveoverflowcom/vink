// Pages Advanced Mode. The packager inserts public, source-bound release data.
const RELEASE = /*VINK_RELEASE*/ null;

export function handler(release, originFetch = fetch) {
  const origin = new URL(release.origin);
  const assets = new Set(release.assets);
  const remote = new Set(release.origin_assets);
  const finish = (response, delivery, asset = false) => {
    const headers = new Headers(response.headers);
    headers.set("X-Robots-Tag", "noindex, nofollow");
    headers.set("X-Vink-Delivery", delivery);
    headers.set("X-Vink-Release", release.asset_digest);
    if (asset) {
      // Names are not all content-addressed. Never advertise immutable caching.
      headers.set("Cache-Control", "public, max-age=0, must-revalidate");
      headers.delete("Set-Cookie");
      headers.delete("Access-Control-Allow-Origin");
    } else if (!headers.has("Cache-Control")) {
      headers.set("Cache-Control", "no-store");
    }
    return new Response(response.body, { status: response.status, statusText: response.statusText, headers });
  };
  return async (request, env) => {
    const url = new URL(request.url);
    if (request.headers.has("X-Vink-Edge-Hop") || url.host === origin.host) {
      return finish(new Response("Origin routing loop refused", { status: 508 }), "refused");
    }
    if (url.pathname === "/pkg" || url.pathname.startsWith("/pkg/")) {
      if (!["GET", "HEAD"].includes(request.method)) {
        return finish(new Response(null, { status: 405, headers: { Allow: "GET, HEAD" } }), "refused");
      }
      if (assets.has(url.pathname)) {
        return finish(await env.ASSETS.fetch(request), "cloudflare-asset", true);
      }
      if (!remote.has(url.pathname)) {
        return finish(new Response("Asset not found", { status: 404 }), "missing-asset");
      }
    }
    const target = new URL(origin);
    target.pathname = url.pathname;
    target.search = url.search;
    // Incoming Host cannot select an upstream. Origin/CSRF, cookies and conditionals stay intact.
    const headers = new Headers(request.headers);
    headers.delete("Host");
    headers.delete("X-Forwarded-Host");
    headers.delete("X-Forwarded-For");
    headers.delete("Forwarded");
    headers.set("X-Vink-Edge-Hop", "1");
    try {
      const upstream = await originFetch(new Request(target, {
        method: request.method, headers, body: ["GET", "HEAD"].includes(request.method) ? undefined : request.body,
        redirect: "manual", duplex: "half",
      }), { cache: "no-store" });
      return finish(upstream, remote.has(url.pathname) ? "origin-asset" : "origin-dynamic");
    } catch {
      return finish(new Response("Origin unavailable", { status: 502 }), "origin-error");
    }
  };
}

export default { fetch(request, env) { return handler(RELEASE)(request, env); } };
