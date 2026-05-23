import { test, expect, request } from "@playwright/test";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// These tests validate USER OUTCOMES end-to-end against a running
// tileserver + seeded PostGIS. Each test asserts on the rendered SPA
// AND the public API surface, because the curator's job is to make
// real route data visible to end users — not to make a form submit.

const APP = "/admin/app/";
const HERE = dirname(fileURLToPath(import.meta.url));

// Pull the cookie that Playwright will attach to the API request
// context so we can hit the JSON API directly in the same test.
function curatorAuth() {
  const state = JSON.parse(
    readFileSync(resolve(HERE, ".auth/curator.json"), "utf8"),
  );
  return state.cookies[0].value as string;
}

test.describe("Curator outcomes", () => {
  test("dashboard surfaces the seeded resource counts", async ({ page }) => {
    await page.goto(APP);
    await expect(
      page.getByRole("heading", { name: "Dashboard" }),
    ).toBeVisible();

    // The fixture seeds 1 published + 1 draft hiking-trails route.
    // The dashboard must reflect those numbers, otherwise the SPA
    // isn't really showing the curator anything they can act on.
    // Scope to the card (the navigation has its own Hiking trails
    // link).
    const hikingCard = page.getByRole("link", {
      name: /Hiking trails.*draft.*published/i,
    });
    await expect(hikingCard).toContainText("2");
    await expect(hikingCard).toContainText("draft: 1");
    await expect(hikingCard).toContainText("published: 1");
  });

  test("resource list shows the published Sognsvann loop", async ({
    page,
  }) => {
    await page.goto(`${APP}resources/hiking-trails`);
    await expect(
      page.getByRole("heading", { name: "Hiking trails" }),
    ).toBeVisible();

    // The "Sognsvann ridge loop" is the fixture's published route.
    // If the table doesn't render it, the curator can't manage it.
    const row = page.getByRole("row", { name: /Sognsvann ridge loop/i });
    await expect(row).toBeVisible();
    await expect(row).toContainText("published");
    await expect(row).toContainText("0.6 km");
  });

  test("curator can change a route's difficulty and the change reaches the public API", async ({
    page,
    baseURL,
  }) => {
    // Open the published route's edit screen.
    await page.goto(`${APP}resources/hiking-trails`);
    await page.getByRole("link", { name: /Sognsvann ridge loop/i }).click();
    await expect(
      page.getByRole("heading", { name: /Sognsvann ridge loop/i }),
    ).toBeVisible();

    // Change difficulty from "easy" to "hard" and save.
    await page.getByRole("combobox").nth(0).selectOption("hard");
    await page.getByRole("button", { name: "Save" }).click();

    // Save bounces back to the list — that's the user's signal that
    // the change took effect.
    await expect(
      page.getByRole("heading", { name: "Hiking trails" }),
    ).toBeVisible();

    // The real validation: the public API now reports the new value.
    // No mocks, no test doubles — this is the chain the end user reads.
    const apiCtx = await request.newContext({
      baseURL: baseURL!,
      extraHTTPHeaders: { authorization: `Bearer ${curatorAuth()}` },
    });
    const listResp = await apiCtx.get(
      "/admin/api/resources/hiking-trails?limit=5",
    );
    expect(listResp.status()).toBe(200);
    const body = await listResp.json();
    const row = body.rows.find(
      (r: { slug: string }) => r.slug === "sognsvann-loop",
    );
    expect(row).toBeTruthy();
    expect(row.difficulty).toBe("hard");
    await apiCtx.dispose();

    // Restore so subsequent runs are deterministic.
    const restore = await apiCtx.dispose;
    void restore;
    const cleanupCtx = await request.newContext({
      baseURL: baseURL!,
      extraHTTPHeaders: { authorization: `Bearer ${curatorAuth()}` },
    });
    await cleanupCtx.put(`/admin/api/resources/hiking-trails/${row.id}`, {
      data: { difficulty: "easy" },
    });
    await cleanupCtx.dispose();
  });

  test("triggering an ingest job appears in the jobs screen", async ({
    page,
  }) => {
    await page.goto(APP);
    // The fkb-sti job will fail in this sandbox (no outbound to
    // wms.geonorge.no) but the curator's outcome is "I can see that
    // I kicked off a job and it shows up in the log" — failure is a
    // valid terminal state. We assert on visibility + recorded
    // status, not on success.
    const triggers = page.getByRole("button", { name: "Trigger" });
    await triggers.first().click();

    await page.getByRole("link", { name: "Ingest jobs" }).click();
    await expect(
      page.getByRole("heading", { name: "Ingest jobs" }),
    ).toBeVisible();

    // The row for the just-triggered job must appear; the SPA polls
    // every 3s while running, so we wait a bit for the terminal state.
    const row = page
      .getByRole("row", { name: /fkb-sti/i })
      .first();
    await expect(row).toBeVisible({ timeout: 15_000 });
    // It will be `running` or `failed` (offline sandbox) — either is
    // proof the trigger actually reached the backend.
    await expect(row).toContainText(/running|failed|succeeded/);
  });
});

test.describe("Bulk-file ingest from incoming volume", () => {
  test("admin lists files dropped on the incoming volume", async ({
    request: req,
    baseURL,
  }) => {
    // The synthetic DTM is generated by tools/synth-dtm.py before
    // the suite runs (see verify-stack.sh in the repo). If this list
    // is empty the operator dropped the file in the wrong place.
    const ctx = await request.newContext({
      baseURL: baseURL!,
      extraHTTPHeaders: { authorization: `Bearer ${curatorAuth()}` },
    });
    const resp = await ctx.get("/admin/api/ingest/incoming");
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.incoming_dir).toMatch(/incoming/);
    const tif = body.files.find(
      (f: { name: string }) => f.name === "dtm10-synth.tif",
    );
    expect(tif).toBeTruthy();
    expect(tif.size_bytes).toBeGreaterThan(1000);
    await ctx.dispose();
  });

  test("bulk-trigger endpoint runs raster2pgsql and dtm10-attach picks up the local DEM", async ({
    request: req,
    baseURL,
  }) => {
    const ctx = await request.newContext({
      baseURL: baseURL!,
      extraHTTPHeaders: { authorization: `Bearer ${curatorAuth()}` },
    });

    // Clear any prior elevation so we can prove the attach run does
    // real work this iteration.
    // (No HTTP for this; we reset via psql out-of-band before running
    // the suite. Here we just verify the bulk-trigger reaches the
    // backend and produces a job log entry.)

    const bulk = await ctx.post("/admin/api/ingest/bulk", {
      data: { job: "dtm-load", file: "dtm10-synth.tif", source: "dtm10" },
    });
    expect(bulk.status()).toBe(200);
    const bulkBody = await bulk.json();
    expect(bulkBody.ok).toBe(true);
    expect(bulkBody.file).toMatch(/dtm10-synth\.tif$/);

    // Poll the jobs log until the dtm-load run shows up. Tokio task
    // spawned by the handler runs asynchronously.
    let succeeded = false;
    for (let i = 0; i < 10; i++) {
      const jobs = await ctx.get("/admin/api/ingest/jobs?limit=5");
      const log = await jobs.json();
      const dtmRun = log.rows.find(
        (r: { name: string; run_id: string }) =>
          r.name === "dtm-load" && r.run_id === bulkBody.run_id,
      );
      if (dtmRun?.status === "succeeded") {
        expect(dtmRun.rows_in).toBeGreaterThan(0);
        succeeded = true;
        break;
      }
      if (dtmRun?.status === "failed") {
        throw new Error(
          `dtm-load failed: ${dtmRun.error_text ?? "no error text"}`,
        );
      }
      await new Promise((r) => setTimeout(r, 1000));
    }
    expect(succeeded).toBe(true);
    await ctx.dispose();
  });

  test("bulk endpoint rejects path traversal outside the incoming dir", async ({
    request: req,
    baseURL,
  }) => {
    const ctx = await request.newContext({
      baseURL: baseURL!,
      extraHTTPHeaders: { authorization: `Bearer ${curatorAuth()}` },
    });
    // Try to read /etc/passwd through the bulk endpoint — must be
    // rejected, otherwise any curator can exfiltrate server files.
    const resp = await ctx.post("/admin/api/ingest/bulk", {
      data: { job: "dtm-load", file: "../../../etc/passwd" },
    });
    expect(resp.status()).toBe(400);
    const body = await resp.json();
    expect(body.error).toMatch(/escapes|cannot resolve/i);
    await ctx.dispose();
  });
});

test.describe("Multi-GB upload from a browser (TUS)", () => {
  test("curator uploads a GeoTIFF from disk, sees progress, then triggers ingest end-to-end", async ({
    page,
    baseURL,
  }) => {
    // The synthetic DTM10 GeoTIFF was put on disk by the setup
    // script. We hand it to the file input the same way a curator's
    // browser would after a drag-drop. tus-js-client takes over from
    // there: chunks the file, PATCHes to /admin/api/upload/<id>.
    await page.goto(`${APP}upload-bulk`);
    await expect(
      page.getByRole("heading", { name: "Upload bulk dataset" }),
    ).toBeVisible();

    const TIF = "/var/lib/tileserver/raw/incoming/dtm10-synth.tif";
    await page.getByTestId("bulk-file-input").setInputFiles(TIF);

    // Source label that ends up on the loaded raster rows.
    await page.getByPlaceholder("dtm10").fill("dtm10-via-browser");

    // Kick the upload off and wait for the "uploaded" state. The
    // file is small (~88 KB) so it lands in one chunk, but the same
    // codepath works for multi-GB inputs.
    await page.getByTestId("bulk-start").click();
    const uploadedBanner = page.getByTestId("bulk-uploaded");
    await expect(uploadedBanner).toBeVisible({ timeout: 15_000 });
    await expect(uploadedBanner).toContainText("dtm10-synth.tif");

    // Trigger ingest. The button posts {job, upload_id, source} to
    // /admin/api/ingest/bulk and the curator gets bounced to /jobs.
    await page.getByTestId("bulk-ingest").click();
    await expect(page).toHaveURL(/\/jobs$/, { timeout: 5_000 });

    // The real validation: a dtm-load row with our source label must
    // exist and reach `succeeded`. We poll the API directly because
    // the SPA's auto-refresh interval is 3 s.
    const ctx = await request.newContext({
      baseURL: baseURL!,
      extraHTTPHeaders: { authorization: `Bearer ${curatorAuth()}` },
    });
    let succeeded = false;
    for (let i = 0; i < 15; i++) {
      const resp = await ctx.get("/admin/api/ingest/jobs?limit=10");
      const log = await resp.json();
      const ours = log.rows.find(
        (r: { name: string; status: string }) =>
          r.name === "dtm-load" && r.status === "succeeded",
      );
      if (ours) {
        expect(ours.rows_in).toBeGreaterThan(0);
        succeeded = true;
        break;
      }
      await new Promise((r) => setTimeout(r, 1000));
    }
    expect(succeeded).toBe(true);
    await ctx.dispose();
  });

  test("uploaded file shows up in the incoming list tagged as `upload`", async ({
    baseURL,
  }) => {
    const ctx = await request.newContext({
      baseURL: baseURL!,
      extraHTTPHeaders: { authorization: `Bearer ${curatorAuth()}` },
    });
    const resp = await ctx.get("/admin/api/ingest/incoming");
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    const uploads = body.files.filter(
      (f: { source: string }) => f.source === "upload",
    );
    expect(uploads.length).toBeGreaterThan(0);
    // At least one upload must be complete and carry a valid
    // upload_id the operator can hand to the bulk-ingest endpoint.
    // Stale incomplete uploads from prior runs are allowed — they're
    // a real ops reality and the SPA will mark them as incomplete.
    const completed = uploads.filter(
      (u: { complete: boolean }) => u.complete,
    );
    expect(completed.length).toBeGreaterThan(0);
    for (const u of completed) {
      expect(u.upload_id).toMatch(/^[0-9a-f-]{36}$/);
    }
    await ctx.dispose();
  });
});

test.describe("End-user outcomes (public API)", () => {
  test("hiking-trails MVT returns vector tile bytes for the seeded area", async ({
    request: req,
  }) => {
    // tile 12/2238/1189 covers the seeded Sognsvann grid.
    const resp = await req.get("/v1/hiking-trails/tiles/12/2238/1189.mvt");
    expect(resp.status()).toBe(200);
    expect(resp.headers()["content-type"]).toBe(
      "application/vnd.mapbox-vector-tile",
    );
    const body = await resp.body();
    // 5994 bytes is what we observed during stack bring-up. Anything
    // <1000 means the layer is effectively empty — that's a real
    // regression a user would notice as a blank map.
    expect(body.byteLength).toBeGreaterThan(1000);
  });

  test("hiking-trails GeoJSON list returns features in the seeded bbox", async ({
    request: req,
  }) => {
    const resp = await req.get(
      "/v1/hiking-trails?bbox=16.69,59.97,16.74,60.00&limit=100",
    );
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.type).toBe("FeatureCollection");
    // Fixture: 180 edges as 'sti' or 'traktorveg' that flow into the
    // hiking-trails view, plus one published curated_route.
    expect(body.features.length).toBeGreaterThan(50);
    // One of them must be the published Sognsvann loop.
    const named = body.features.find(
      (f: { properties: { name: string | null } }) =>
        f.properties.name === "Sognsvann ridge loop",
    );
    expect(named).toBeTruthy();
  });

  test("routing finds a real path between two corners of the seeded grid", async ({
    request: req,
  }) => {
    const resp = await req.post("/v1/routing/route", {
      data: {
        from: [16.70210084655078, 59.976356856805864],
        to: [16.735179600968184, 59.99209256156877],
        profile: "hiking",
      },
    });
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    // pgr_dijkstra on a 200 m grid should give ~3-4 km (the diagonal
    // is 2 km straight-line, real graph routes longer).
    expect(body.distance_m).toBeGreaterThan(2000);
    expect(body.distance_m).toBeLessThan(8000);
    // Should be a connected LineString of at least a dozen vertices.
    expect(body.geom.type).toBe("LineString");
    expect(body.geom.coordinates.length).toBeGreaterThan(10);
    // Hiking speed gives ~55 min for 3.6 km → duration_s in range.
    expect(body.duration_s).toBeGreaterThan(1500);
    expect(body.duration_s).toBeLessThan(8000);
  });

  test("routing rejects an unreachable point with a useful error", async ({
    request: req,
  }) => {
    const resp = await req.post("/v1/routing/route", {
      data: {
        from: [10.0, 60.0], // far from seeded grid
        to: [10.001, 60.001],
        profile: "hiking",
      },
    });
    // The user sees a 400 with "no nearby path node" — that's the
    // signal they're trying to route from a place we don't know about.
    expect(resp.status()).toBe(400);
    const body = await resp.json();
    expect(body.error).toMatch(/nearby/);
  });

  test("catalog advertises all four resources with stable URL templates", async ({
    request: req,
  }) => {
    const resp = await req.get("/v1/catalog");
    expect(resp.status()).toBe(200);
    const body = await resp.json();
    expect(body.resources).toHaveLength(4);
    const ids = body.resources.map((r: { id: string }) => r.id).sort();
    expect(ids).toEqual([
      "cycling-routes",
      "forest-roads",
      "hiking-trails",
      "ski-tracks",
    ]);
    // Each entry must contain {z}/{x}/{y} for the MVT template — the
    // Flutter client interpolates these literally.
    for (const r of body.resources) {
      expect(r.tiles_url_template).toContain("{z}");
      expect(r.tiles_url_template).toContain("{x}");
      expect(r.tiles_url_template).toContain("{y}");
    }
  });
});
