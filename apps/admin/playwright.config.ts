import { defineConfig, devices } from "@playwright/test";

// E2E tests run against the *built* SPA served by a running tileserver
// on PLAYWRIGHT_BASE_URL (default http://localhost:8090/admin/app/).
// No mocks. The DB must be seeded with the fixture in
// apps/tileserver/tools/seed-oslo-fixture.sql first.
export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:8090",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    // Inject the curator JWT issued by tools/mint-test-jwt.mjs (matches
    // the JWT_SECRET the tileserver was started with). The Rust auth
    // extractor reads both the Authorization header and the
    // access_token cookie; we set the cookie globally so SPA fetches
    // (which use credentials: 'include') pick it up.
    storageState: "./e2e/.auth/curator.json",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        // Sandbox ships chromium-1194 at /opt/pw-browsers; point
        // launch at the binary directly so Playwright doesn't try to
        // download a newer build it doesn't have outbound access for.
        launchOptions: {
          executablePath:
            process.env.CHROME_BIN ??
            "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
        },
      },
    },
  ],
});
