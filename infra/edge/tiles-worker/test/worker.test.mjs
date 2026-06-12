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
          store.set(key, { body, meta: opts?.httpMetadata });
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

const ctx = { waitUntil: (p) => p };
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
  const calls = [];
  const resp = await handle(new Request(TILE), env, ctx, mockDeps(calls));
  assert.equal(resp.status, 200);
  assert.equal(resp.headers.get("x-tiles-cache"), "miss");
  assert.deepEqual(calls, ["https://origin.example/v1/basemap/12/2170/1189.mvt"]);
  assert.ok(store.has("n50-2026.06/v1/basemap/12/2170/1189.mvt"), "write-through to R2");
});

test("R2 hit serves without touching origin", async () => {
  const { env, store } = mockEnv();
  const calls = [];
  const deps = mockDeps(calls);
  await handle(new Request(TILE), env, ctx, deps); // warm
  calls.length = 0;
  const resp = await handle(new Request(TILE), env, ctx, deps);
  assert.equal(resp.headers.get("x-tiles-cache"), "r2");
  assert.equal(calls.length, 0, "origin must not be hit on R2 hit");
  assert.equal(store.size, 1);
});

test("a wiped bucket repulls from origin — R2 is disposable", async () => {
  const { env, store } = mockEnv();
  const calls = [];
  const deps = mockDeps(calls);
  await handle(new Request(TILE), env, ctx, deps);
  store.clear(); // simulate bucket wipe
  const resp = await handle(new Request(TILE), env, ctx, deps);
  assert.equal(resp.status, 200);
  assert.equal(calls.length, 2, "second request re-pulls origin");
  assert.equal(store.size, 1, "bucket re-warms");
});

test("data-version bump orphans old keys instead of overwriting", async () => {
  const { env, store } = mockEnv();
  const deps = mockDeps([]);
  await handle(new Request(TILE), env, ctx, deps);
  env.DATA_VERSION = "n50-2026.07";
  await handle(new Request(TILE), env, ctx, deps);
  assert.ok(store.has("n50-2026.06/v1/basemap/12/2170/1189.mvt"));
  assert.ok(store.has("n50-2026.07/v1/basemap/12/2170/1189.mvt"));
});

test("non-cacheable traffic proxies to origin and never touches R2", async () => {
  const { env, store } = mockEnv();
  const calls = [];
  const resp = await handle(
    new Request("https://tiles.example/v1/route/plan?x=1", { method: "POST" }),
    env,
    ctx,
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
  const resp = await handle(new Request(TILE), env, ctx, deps);
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
  const resp = await handle(new Request(TILE), env, ctx, deps);
  assert.equal(resp.headers.get("x-tiles-cache"), "edge");
  assert.equal(calls.length, 0);
  assert.equal(store.size, 0);
});
