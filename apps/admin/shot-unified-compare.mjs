import { chromium } from "@playwright/test";

// Detour case from the user's image 16 (off-trail endpoints near a sti).
const FROM = [15.38375, 67.40034];
const TO = [15.38844, 67.39493];
const CENTER = [15.3861, 67.3976];
const ZOOM = 15.2;
const CLIP = { x: 360, y: 230, width: 560, height: 560 };

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
page.on("pageerror", (e) => console.log("PAGEERR", e.message));

await page.goto("http://localhost:5173/admin/dev-login", { waitUntil: "networkidle" });
await page.goto("http://localhost:5173/admin/app/plot", { waitUntil: "networkidle" });
await page.waitForFunction(() => window.__pf && window.__pf.map && window.__pf.map.loaded(), null, { timeout: 30000 });
await page.evaluate(({ c, z }) => window.__pf.map.jumpTo({ center: c, zoom: z }), { c: CENTER, z: ZOOM });
await page.waitForTimeout(2500);

// Hybrid (current default)
await page.evaluate(() => { window.__pf.setUnified(false); window.__pf.setForceOffTrail(false); });
await page.evaluate(({ from, to }) => { window.__pf.setFrom(from[0], from[1]); window.__pf.setTo(to[0], to[1]); }, { from: FROM, to: TO });
await page.waitForTimeout(6000);
await page.screenshot({ path: "/tmp/uni-hybrid.png", clip: CLIP });
console.log("hybrid shot saved");

// Unified
await page.evaluate(() => window.__pf.setUnified(true));
await page.waitForTimeout(6000);
await page.screenshot({ path: "/tmp/uni-unified.png", clip: CLIP });
console.log("unified shot saved");

await browser.close();
