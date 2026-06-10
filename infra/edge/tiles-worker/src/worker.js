// Cloudflare Worker: the public tile host. R2 is STRICTLY a pull-through
// distributed cache between Cloudflare's edge and our self-hosted origin —
// never the system of record (design rule 1 of the N50 basemap plan).
//
// Request lifecycle:
//   1. L1 — the edge Cache API (per-PoP).
//   2. L2 — R2 (`env.TILES`), shared across PoPs.
//   3. Origin — the tileserver (`env.ORIGIN`). The response is written
//      through to R2 + edge cache in the background.
//
// Correctness invariants:
//   - Every R2 object is byte-for-byte regenerable from origin: keys are
//     prefixed with `env.DATA_VERSION`, so a data rebuild bumps the version,
//     orphaning old keys (a 30-day lifecycle rule deletes them). Wiping the
//     bucket loses nothing but warmth.
//   - Only idempotent GETs on the cacheable-path allowlist touch R2. Routing
//     POSTs, search, admin, health — everything else — proxy straight
//     through to origin uncached.
//   - The Worker never serves from R2 without an origin able to regenerate
//     the same key (no build step uploads tiles; R2 fills lazily).

/** Paths that are immutable-ish tile/style assets, safe to cache hard. */
const CACHEABLE = [
  /^\/v1\/basemap\/\d+\/\d+\/\d+\.mvt$/,
  /^\/v1\/basemap\/style\.json$/,
  /^\/v1\/basemap$/,
  /^\/v1\/dem\/rgb\/\d+\/\d+\/\d+\.png$/,
  /^\/v1\/slope\/tiles\/\d+\/\d+\/\d+\.png$/,
  /^\/v1\/raster\/[a-z0-9-]+\/\d+\/\d+\/\d+\.(png|webp)$/,
  /^\/v1\/[a-z-]+\/tiles\/\d+\/\d+\/\d+\.mvt$/, // curated resources
  /^\/fonts\/[^/]+\/[0-9-]+\.pbf$/,
  /^\/sprite[^/]*\.(json|png)$/,
];

export function isCacheable(method, pathname) {
  return method === "GET" && CACHEABLE.some((re) => re.test(pathname));
}

/** R2 key for a request path: versioned so rebuilds orphan, not corrupt. */
export function r2Key(dataVersion, pathname) {
  return `${dataVersion}${pathname}`;
}

/**
 * Core handler, dependency-injected for tests:
 *   deps.cache   — Cache-API-like  { match(req), put(req, resp) }
 *   deps.fetch   — fetch-like, used for the origin request
 * env: { TILES: R2Bucket, ORIGIN: string, DATA_VERSION: string }
 */
export async function handle(request, env, ctx, deps) {
  const url = new URL(request.url);
  const origin = env.ORIGIN.replace(/\/$/, "");

  // Non-cacheable traffic: transparent proxy, R2 never involved.
  if (!isCacheable(request.method, url.pathname)) {
    return deps.fetch(origin + url.pathname + url.search, request);
  }

  // L1: edge cache.
  const cached = await deps.cache.match(request);
  if (cached) {
    return withHeader(cached, "x-tiles-cache", "edge");
  }

  // L2: R2.
  const key = r2Key(env.DATA_VERSION, url.pathname);
  const obj = await env.TILES.get(key);
  if (obj !== null && obj !== undefined) {
    const resp = r2Response(obj);
    ctx.waitUntil(deps.cache.put(request, resp.clone()));
    return withHeader(resp, "x-tiles-cache", "r2");
  }

  // Origin. Only successful, non-empty responses are cached — errors and
  // blank tiles must not poison either tier.
  const upstream = await deps.fetch(origin + url.pathname, {
    method: "GET",
    headers: { "user-agent": "turbo-tiles-worker" },
  });
  if (!upstream.ok) {
    return upstream;
  }
  const body = await upstream.arrayBuffer();
  const resp = new Response(body, {
    status: 200,
    headers: pickHeaders(upstream.headers),
  });
  if (body.byteLength > 0) {
    ctx.waitUntil(
      env.TILES.put(key, body, {
        httpMetadata: {
          contentType: upstream.headers.get("content-type") ?? "application/octet-stream",
          cacheControl: upstream.headers.get("cache-control") ?? undefined,
        },
      }),
    );
    ctx.waitUntil(deps.cache.put(request, resp.clone()));
  }
  return withHeader(resp, "x-tiles-cache", "miss");
}

function pickHeaders(headers) {
  const out = new Headers();
  for (const name of ["content-type", "cache-control", "content-encoding"]) {
    const v = headers.get(name);
    if (v) out.set(name, v);
  }
  return out;
}

function r2Response(obj) {
  const headers = new Headers();
  headers.set("content-type", obj.httpMetadata?.contentType ?? "application/octet-stream");
  if (obj.httpMetadata?.cacheControl) {
    headers.set("cache-control", obj.httpMetadata.cacheControl);
  }
  return new Response(obj.body, { status: 200, headers });
}

function withHeader(resp, name, value) {
  const r = new Response(resp.body, resp);
  r.headers.set(name, value);
  return r;
}

export default {
  async fetch(request, env, ctx) {
    return handle(request, env, ctx, {
      cache: caches.default,
      fetch: (url, init) => fetch(url, init),
    });
  },
};
