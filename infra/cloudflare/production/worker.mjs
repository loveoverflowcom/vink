// Filled from the exact image-exported inventory. Pages owns the files alongside this module.
const releases = /*VINK_ASSETS*/ {};
const publicHost = "vink.forum";
const origin = "https://origin-prod.vink.forum";
// The zone may compress these for transfer: decoded bytes still match the inventory SHA-256.
// Fonts and images are already compressed and keep no-transform, so their bytes stay exact.
const encodable = new Set(["text/javascript", "text/css", "application/wasm", "image/svg+xml",
  "application/manifest+json", "application/json", "text/plain"]);
const textError = (status, message) => new Response(message, {
  status, headers: { "cache-control": "no-store", "content-type": "text/plain; charset=utf-8",
    "x-robots-tag": "noindex", "x-content-type-options": "nosniff" },
});

// Reserve API, probes and asset-like paths even when opened in the address bar.
export function spaRoute(path, method, accept) {
  if (/^\/api\/v1(?:\/|$)/.test(path) || /^\/health\/(live|ready)$/.test(path)) return "api";
  if (/^\/(api|health|pkg|_v)(?:\/|$)/.test(path) || /\.[^/]+\/?$/.test(path)) return "missing";
  if (method !== "GET" && method !== "HEAD") return "method";
  const acceptsHtml = accept?.split(",").some(range => {
    const [type, ...params] = range.toLowerCase().split(";").map(part => part.trim());
    const quality = params.find(param => /^q\s*=/.test(param));
    return type === "text/html" && (!quality || Number(quality.split("=")[1]) > 0);
  });
  return acceptsHtml ? "document" : "missing";
}

export function handler(inventory, originFetch = fetch, assetFetch = fetch) {
  const known = new Set(Object.keys(inventory).filter(p => p.startsWith("/_v/")).map(p => p.split("/")[2]));
  return async (request, env) => {
    const url = new URL(request.url);
    if (request.headers.has("X-Vink-Edge-Hop") || url.origin === origin) {
      return textError(508, "Origin routing loop");
    }
    if (url.hostname === "www.vink.forum" || (url.hostname === publicHost && url.protocol === "http:")) {
      url.protocol = "https:";
      url.host = publicHost;
      return Response.redirect(url.href, request.url.startsWith("http:") ? 308 : 301);
    }
    const preview = url.hostname !== publicHost;
    if (url.pathname === "/pkg" || url.pathname.startsWith("/pkg/") ||
        url.pathname === "/_v" || url.pathname.startsWith("/_v/")) {
      if (!["GET", "HEAD"].includes(request.method)) return textError(405, "Read-only assets");
      const entry = Object.hasOwn(inventory, url.pathname) && inventory[url.pathname];
      if (!entry) {
        const match = /^\/_v\/([a-f0-9]{40})\/pkg\/([a-zA-Z0-9._/-]+)$/.exec(url.pathname);
        // During Pages propagation an older edge may see HTML from the newer origin.
        // The pre-staged release alias serves inventory bytes only, never production SSR.
        if (preview || !match || known.has(match[1])) return textError(404, "Asset not found");
        try {
          const pinned = new URL(url.pathname, `https://r-${match[1].slice(0, 26)}.vink-production.pages.dev`);
          const headers = new Headers();
          if (request.headers.has("If-None-Match")) headers.set("If-None-Match", request.headers.get("If-None-Match"));
          const found = await assetFetch(new Request(pinned, { method: request.method, headers, redirect: "manual" }));
          if (found.status === 404) return textError(404, "Asset not found");
          if (![200, 304].includes(found.status) || found.headers.get("X-Vink-Asset-Revision") !== match[1]) {
            return textError(502, "Release storage unavailable");
          }
          const result = new Response(found.body, found);
          result.headers.delete("X-Robots-Tag");
          result.headers.delete("Set-Cookie");
          result.headers.set("X-Vink-Asset-Route", "fallback/r26");
          return result;
        } catch { return textError(502, "Release storage unavailable"); }
      }
      try {
        // Pages receives neither credentials nor actor-specific headers.
        const target = new URL(request.url);
        target.search = "";
        if (entry.path) target.pathname = entry.path;
        const response = await env.ASSETS.fetch(new Request(target, { method: request.method }));
        if (response.status !== 200 || response.headers.get("content-type")?.includes("text/html")) {
          return textError(502, "Published asset unavailable");
        }
        const encoded = encodable.has(entry.type);
        const headers = new Headers({ "Content-Type": entry.type, ETag: '"' + entry.sha256 + '"',
          "Cache-Control": entry.path ? "no-cache" : "public, max-age=31536000, immutable" + (encoded ? "" : ", no-transform"),
          "X-Vink-Asset-Revision": (entry.path || url.pathname).split("/")[2],
          "X-Vink-Asset-Route": "inventory/r26",
          "X-Content-Type-Options": "nosniff" });
        if (encoded) headers.set("Vary", "Accept-Encoding");
        if (preview) headers.set("X-Robots-Tag", "noindex, nofollow");
        // A compressed edge response carries W/"sha256"; If-None-Match uses weak comparison.
        const conditions = (request.headers.get("if-none-match") || "").split(",").map(x => x.trim().replace(/^W\//, ""));
        const unchanged = conditions.includes("*") || conditions.includes(headers.get("ETag"));
        if (unchanged || request.method === "HEAD") await response.body?.cancel();
        return new Response(unchanged || request.method === "HEAD" ? null : response.body, {
          status: unchanged ? 304 : 200, headers,
        });
      } catch { return textError(502, "Asset storage unavailable"); }
    }
    // Provider and preview URLs may qualify public bytes, never read production sessions.
    if (preview) return textError(404, "Use the canonical site");
    // Only the standalone packager emits a root document in the inventory.
    if (Object.hasOwn(inventory, "/")) {
      const route = spaRoute(url.pathname, request.method, request.headers.get("accept"));
      if (route === "method") return textError(405, "Read-only document");
      if (route === "missing") return textError(404, "Not found");
      if (route === "document") {
        try {
          const page = await env.ASSETS.fetch(new Request(new URL("/", url), { method: request.method }));
          if (page.status !== 200 || !page.headers.get("content-type")?.startsWith("text/html")) {
            return textError(502, "Published document unavailable");
          }
          return new Response(request.method === "HEAD" ? null : page.body, { headers: {
            "content-type": "text/html; charset=utf-8", "cache-control": "no-store",
            "x-content-type-options": "nosniff", "referrer-policy": "strict-origin-when-cross-origin",
          } });
        } catch { return textError(502, "Document storage unavailable"); }
      }
    }
    if (!env.ORIGIN_KEY || !request.headers.get("CF-Connecting-IP")) return textError(503, "Origin ingress unavailable");
    const headers = new Headers(request.headers);
    for (const name of [...headers.keys()]) {
      if (/^(host$|forwarded$|x-forwarded-|x-real-ip$|x-vink-|cf-)/i.test(name)) headers.delete(name);
    }
    headers.set("X-Vink-Edge-Hop", "1");
    headers.set("X-Vink-Origin-Key", env.ORIGIN_KEY);
    headers.set("X-Vink-Client-IP", request.headers.get("CF-Connecting-IP"));
    const target = new URL(origin);
    target.pathname = url.pathname;
    target.search = url.search;
    try {
      const upstream = await originFetch(new Request(target, { method: request.method,
        body: request.body, duplex: "half", headers, redirect: "manual" }), { cache: "no-store" });
      const result = new Response(upstream.body, upstream);
      // READ-01 endpoints alone may retain an App-supplied private revalidation contract.
      const publicRead = /^\/api\/v1\/works(?:\/[0-9a-f-]+\/(?:reader|publication))?$/.test(url.pathname);
      if (!publicRead || !["GET", "HEAD"].includes(request.method) || headers.has("cookie") ||
          headers.has("authorization") || result.headers.has("set-cookie") ||
          result.headers.get("cache-control") !== "private, no-cache") {
        result.headers.set("cache-control", "no-store");
      } else {
        // Prevent Pages from re-encoding the response and weakening its body-hash ETag.
        result.headers.set("cache-control", "private, no-cache, no-transform");
      }
      return result;
    } catch { return textError(502, "Origin unavailable"); }
  };
}
const serve = handler(releases);
export default { fetch: serve };
