// Plain `node --test` suite for the cache decision logic — no wrangler or
// miniflare needed. The handler takes its cache + fetch as injected deps;
// R2 is mocked with a Map. The contract under test is the "R2 is only a
// cache" guarantee: miss → origin → write-through; hit → no origin; wiped
// bucket → repull; non-cacheable → straight proxy, R2 untouched.
import { test } from "node:test";
import assert from "node:assert/strict";
import { handle, isCacheable, r2Key } from "../src/worker.js";

function mockEnv() {
  const store = new Map();
  return {
    store,
    env: {
      ORIGIN: "https://origin.example",
      DATA_VERSION: "n50-2026.06",
      TILES: {
        async get(key) {
          if (!store.has(key)) return null;
          const { body, meta } = store.get(key);
          return { body, httpMetadata: meta };
        },
        async put(key, body, opts) {
          // R2 materialises what it is given. The mock must too: the
          // write-through now hands it a ReadableStream, and a stream
          // stored as-is can be read exactly once — which would let a
          // broken cache pass "hit returns the same bytes".
          const bytes =
            body instanceof ReadableStream
              ? new Uint8Array(await new Response(body).arrayBuffer())
              : body;
          store.set(key, { body: bytes, meta: opts?.httpMetadata });
        },
      },
    },
  };
}

function mockDeps(originCalls) {
  return {
    cache: { match: async () => undefined, put: async () => {} },
    fetch: async (url) => {
      originCalls.push(url);
      return new Response(new Uint8Array([1, 2, 3]), {
        status: 200,
        headers: {
          "content-type": "application/vnd.mapbox-vector-tile",
          "cache-control": "public, max-age=86400",
        },
      });
    },
  };
}

/// A `ctx` whose background work can be awaited.
///
/// The write-through is genuinely asynchronous now — it streams into R2
/// rather than handing it an already-buffered body — so a test that
/// asserts on the bucket immediately after `handle()` returns is racing
/// the put it is checking for. `settle()` is the drain the real runtime
/// provides by keeping the isolate alive until `waitUntil` resolves.
function mockCtx() {
  const pending = [];
  return {
    waitUntil: (p) => pending.push(p),
    settle: () => Promise.all(pending.splice(0)),
  };
}
const TILE = "https://tiles.example/v1/basemap/12/2170/1189.mvt";

test("cacheable allowlist matches tiles, styles, fonts — not routing/admin", () => {
  assert.ok(isCacheable("GET", "/v1/basemap/12/2170/1189.mvt"));
  assert.ok(isCacheable("GET", "/v1/basemap/style.json"));
  assert.ok(isCacheable("GET", "/v1/dem/rgb/11/1085/594.png"));
  assert.ok(isCacheable("GET", "/v1/slope/tiles/12/2170/1189.png"));
  assert.ok(isCacheable("GET", "/v1/hiking-trails/tiles/12/2170/1189.mvt"));
  assert.ok(isCacheable("GET", "/fonts/Noto Sans Regular/0-255.pbf"));
  assert.ok(!isCacheable("POST", "/v1/basemap/12/2170/1189.mvt"));
  assert.ok(!isCacheable("GET", "/v1/route/plan"));
  assert.ok(!isCacheable("GET", "/admin/api/state"));
  assert.ok(!isCacheable("GET", "/healthz"));
});

test("r2 keys are data-version prefixed (rebuild = new key space)", () => {
  assert.equal(
    r2Key("n50-2026.06", "/v1/basemap/1/2/3.mvt"),
    "n50-2026.06/v1/basemap/1/2/3.mvt",
  );
});

test("miss pulls origin and writes through to R2", async () => {
  const { env, store } = mockEnv();
  const ctx = mockCtx();
  const calls = [];
  const resp = await handle(new Request(TILE), env, ctx, mockDeps(calls));
  await ctx.settle();
  assert.equal(resp.status, 200);
  assert.equal(resp.headers.get("x-tiles-cache"), "miss");
  assert.deepEqual(calls, ["https://origin.example/v1/basemap/12/2170/1189.mvt"]);
  assert.ok(store.has("n50-2026.06/v1/basemap/12/2170/1189.mvt"), "write-through to R2");
});

test("R2 hit serves without touching origin", async () => {
  const { env, store } = mockEnv();
  const ctx = mockCtx();
  const calls = [];
  const deps = mockDeps(calls);
  await handle(new Request(TILE), env, ctx, deps); // warm
  await ctx.settle();
  calls.length = 0;
  const resp = await handle(new Request(TILE), env, ctx, deps);
  assert.equal(resp.headers.get("x-tiles-cache"), "r2");
  assert.equal(calls.length, 0, "origin must not be hit on R2 hit");
  assert.equal(store.size, 1);
});

test("a wiped bucket repulls from origin — R2 is disposable", async () => {
  const { env, store } = mockEnv();
  const ctx = mockCtx();
  const calls = [];
  const deps = mockDeps(calls);
  await handle(new Request(TILE), env, ctx, deps);
  await ctx.settle();
  store.clear(); // simulate bucket wipe
  const resp = await handle(new Request(TILE), env, ctx, deps);
  await ctx.settle();
  assert.equal(resp.status, 200);
  assert.equal(calls.length, 2, "second request re-pulls origin");
  assert.equal(store.size, 1, "bucket re-warms");
});

test("data-version bump orphans old keys instead of overwriting", async () => {
  const { env, store } = mockEnv();
  const ctx = mockCtx();
  const deps = mockDeps([]);
  await handle(new Request(TILE), env, ctx, deps);
  await ctx.settle();
  env.DATA_VERSION = "n50-2026.07";
  await handle(new Request(TILE), env, ctx, deps);
  await ctx.settle();
  assert.ok(store.has("n50-2026.06/v1/basemap/12/2170/1189.mvt"));
  assert.ok(store.has("n50-2026.07/v1/basemap/12/2170/1189.mvt"));
});

test("non-cacheable traffic proxies to origin and never touches R2", async () => {
  const { env, store } = mockEnv();
  const calls = [];
  const resp = await handle(
    new Request("https://tiles.example/v1/route/plan?x=1", { method: "POST" }),
    env,
    mockCtx(),
    mockDeps(calls),
  );
  assert.equal(resp.status, 200);
  assert.deepEqual(calls, ["https://origin.example/v1/route/plan?x=1"]);
  assert.equal(store.size, 0, "R2 untouched for non-cacheable traffic");
});

test("origin errors are passed through and never cached", async () => {
  const { env, store } = mockEnv();
  const deps = {
    cache: { match: async () => undefined, put: async () => {} },
    fetch: async () => new Response("boom", { status: 503 }),
  };
  const resp = await handle(new Request(TILE), env, mockCtx(), deps);
  assert.equal(resp.status, 503);
  assert.equal(store.size, 0, "errors must not poison R2");
});

test("edge cache hit short-circuits both R2 and origin", async () => {
  const { env, store } = mockEnv();
  const calls = [];
  const deps = {
    cache: {
      match: async () => new Response("tile", { status: 200 }),
      put: async () => {},
    },
    fetch: async (url) => {
      calls.push(url);
      return new Response("x");
    },
  };
  const resp = await handle(new Request(TILE), env, mockCtx(), deps);
  assert.equal(resp.headers.get("x-tiles-cache"), "edge");
  assert.equal(calls.length, 0);
  assert.equal(store.size, 0);
});

// ---- region packs ---------------------------------------------------

test("pack files are cacheable; malformed pack paths are not", () => {
  assert.ok(isCacheable("GET", "/v1/packs/z12_2218_1007_2219_1008/pack.toml"));
  assert.ok(isCacheable("GET", "/v1/packs/z12_2218_1007_2219_1008/norway.dem"));
  assert.ok(isCacheable("GET", "/v1/packs/z12_1_2_3_4/norway.graph_geom"));
  // A path that is not a well-formed key must not be cached: it cannot
  // be regenerated from origin the same way twice, which is the one
  // invariant this tier rests on.
  assert.ok(!isCacheable("GET", "/v1/packs/../../etc/passwd"));
  assert.ok(!isCacheable("GET", "/v1/packs/z12_2218_1007_2219_1008/a/b"));
  assert.ok(!isCacheable("GET", "/v1/packs?bbox=14.9,67.0,15.2,67.1"));
  assert.ok(!isCacheable("POST", "/v1/packs/z12_1_2_3_4/pack.toml"));
});

test("a large body is written through without being buffered", async () => {
  // The regression this guards: the write-through used to
  // `await upstream.arrayBuffer()`, which is fine for a kilobyte tile
  // and an OOM for a pack DEM. `tee()` means neither branch is ever
  // whole in memory — and the client must still receive every byte.
  const { store, env } = mockEnv();
  const PACK = "https://tiles.example/v1/packs/z12_1_2_3_4/norway.dem";
  const payload = new Uint8Array(1 << 20).fill(7); // 1 MiB

  let pulled = 0;
  const deps = {
    cache: { match: async () => undefined, put: async () => {} },
    fetch: async () => {
      pulled += 1;
      return new Response(payload, {
        status: 200,
        headers: {
          "content-type": "application/octet-stream",
          "content-length": String(payload.byteLength),
          "cache-control": "public, max-age=31536000, immutable",
        },
      });
    },
  };

  const ctx = mockCtx();
  const resp = await handle(new Request(PACK), env, ctx, deps);
  const got = new Uint8Array(await resp.arrayBuffer());
  await ctx.settle();

  assert.equal(resp.headers.get("x-tiles-cache"), "miss");
  assert.equal(got.byteLength, payload.byteLength, "the client must get every byte");
  assert.equal(got[0], 7);
  assert.equal(got[got.length - 1], 7);

  // And R2 holds the same bytes, so the next request is a hit rather
  // than a second trip to a single-node origin.
  const cached = store.get(r2Key(env.DATA_VERSION, "/v1/packs/z12_1_2_3_4/norway.dem"));
  assert.ok(cached, "the pack must have been written through to R2");
  assert.equal(cached.body.byteLength, payload.byteLength);
  assert.equal(cached.meta.cacheControl, "public, max-age=31536000, immutable");
  assert.equal(pulled, 1);
});

test("a pack served from R2 does not touch origin", async () => {
  const { env } = mockEnv();
  const PATH = "/v1/packs/z12_1_2_3_4/pack.toml";
  await env.TILES.put(r2Key(env.DATA_VERSION, PATH), new Uint8Array([9, 9]), {
    httpMetadata: { contentType: "text/plain" },
  });

  const calls = [];
  const deps = {
    cache: { match: async () => undefined, put: async () => {} },
    fetch: async (u) => {
      calls.push(u);
      return new Response("nope", { status: 500 });
    },
  };
  const resp = await handle(new Request("https://tiles.example" + PATH), env, mockCtx(), deps);
  assert.equal(resp.headers.get("x-tiles-cache"), "r2");
  assert.deepEqual(calls, [], "an R2 hit must not reach the origin");
});

test("the write-through does not wait for the whole body", async () => {
  // The property the `tee()` change exists for, and the one the size
  // test above CANNOT check: buffering also delivers every byte, just
  // after holding them all in memory first.
  //
  // So this asserts timing instead of content. The origin hands back a
  // body that emits one chunk and then stalls. A streaming handler
  // returns a Response immediately — headers are known, the body is
  // still arriving. A buffering handler cannot return until the last
  // byte lands, which here is never.
  const { env } = mockEnv();
  let release;
  const stalled = new Promise((r) => (release = r));

  const deps = {
    cache: { match: async () => undefined, put: async () => {} },
    fetch: async () =>
      new Response(
        new ReadableStream({
          async start(c) {
            c.enqueue(new Uint8Array([1]));
            await stalled;
            c.enqueue(new Uint8Array([2]));
            c.close();
          },
        }),
        { status: 200, headers: { "content-type": "application/octet-stream" } },
      ),
  };

  const raced = await Promise.race([
    handle(new Request("https://tiles.example/v1/packs/z12_1_2_3_4/norway.dem"), env, mockCtx(), deps)
      .then(() => "returned"),
    new Promise((r) => setTimeout(() => r("blocked"), 250)),
  ]);
  release();

  assert.equal(
    raced,
    "returned",
    "handle() waited for the body — it is buffering, and a pack-sized " +
      "response will OOM the Worker",
  );
});
