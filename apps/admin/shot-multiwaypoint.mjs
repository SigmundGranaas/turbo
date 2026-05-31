import { chromium } from "@playwright/test";

// Three stops near Langvatnet (start -> via -> end). All solve.
const PTS = [
  [15.37099, 67.39811],
  [15.38030, 67.40415],
  [15.39000, 67.40100],
];
const CENTER = [15.381, 67.401];
const ZOOM = 13.4;

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
page.on("pageerror", (e) => console.log("PAGEERR", e.message));

await page.goto("http://localhost:5173/admin/dev-login", { waitUntil: "networkidle" });
await page.goto("http://localhost:5173/admin/app/plot", { waitUntil: "networkidle" });
await page.waitForFunction(() => window.__pf && window.__pf.map && window.__pf.map.loaded(), null, { timeout: 30000 });
await page.evaluate(({ c, z }) => window.__pf.map.jumpTo({ center: c, zoom: z }), { c: CENTER, z: ZOOM });
await page.waitForTimeout(1500);

// Drop three ordered waypoints.
await page.evaluate((pts) => window.__pf.setPoints(pts), PTS);
await page.waitForTimeout(7000);

// Inspect the live DOM: marker labels + waypoint-list rows + route layers.
const report = await page.evaluate(() => {
  const markers = [...document.querySelectorAll("[data-wp]")]
    .sort((a, b) => +a.getAttribute("data-wp") - +b.getAttribute("data-wp"))
    .map((el) => el.textContent);
  const rows = document.querySelectorAll("aside ol li").length;
  const routeLayers = (window.__pf.map.getStyle().layers || [])
    .filter((l) => l.id.startsWith("path-")).length;
  return { markers, rows, routeLayers };
});
console.log("MARKERS:", JSON.stringify(report.markers));
console.log("LIST ROWS:", report.rows);
console.log("ROUTE LAYERS:", report.routeLayers);

await page.screenshot({ path: "/tmp/multiwaypoint.png" });
console.log("shot saved /tmp/multiwaypoint.png");

// Move the middle stop and confirm it recomputes (route layers still present).
await page.evaluate((pts) => window.__pf.setPoints(pts), [PTS[0], [15.384, 67.398], PTS[2]]);
await page.waitForTimeout(6000);
const after = await page.evaluate(() => ({
  rows: document.querySelectorAll("aside ol li").length,
  routeLayers: (window.__pf.map.getStyle().layers || []).filter((l) => l.id.startsWith("path-")).length,
}));
console.log("AFTER MOVE rows:", after.rows, "routeLayers:", after.routeLayers);

await browser.close();
