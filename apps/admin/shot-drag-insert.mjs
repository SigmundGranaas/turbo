import { chromium } from "@playwright/test";
// 2 stops; drag the route midpoint to insert a 3rd.
const P = [[15.37099, 67.39811], [15.39000, 67.40100]];
const b = await chromium.launch();
const page = await b.newPage({ viewport: { width: 1280, height: 1000 } });
page.on("pageerror", (e) => console.log("PAGEERR", e.message));
await page.goto("http://localhost:5173/admin/dev-login", { waitUntil: "networkidle" });
await page.goto("http://localhost:5173/admin/app/plot", { waitUntil: "networkidle" });
await page.waitForFunction(() => window.__pf && window.__pf.map && window.__pf.map.loaded(), null, { timeout: 30000 });
await page.evaluate(() => window.__pf.map.jumpTo({ center: [15.381, 67.401], zoom: 13.4 }));
await page.waitForTimeout(1200);
await page.evaluate((p) => window.__pf.setPoints(p), P);
await page.waitForTimeout(7000);
const before = await page.evaluate(() => document.querySelectorAll("aside ol li").length);
// Viewport pixel of the route midpoint = canvas rect offset + map.project.
const px = await page.evaluate(() => {
  const m = window.__pf.map;
  const c = m.getSource("path-hit")._data.geometry.coordinates;
  const p = m.project(c[Math.floor(c.length / 2)]);
  const r = m.getCanvas().getBoundingClientRect();
  return { x: r.left + p.x, y: r.top + p.y };
});
await page.mouse.move(px.x, px.y);
await page.mouse.down();
await page.mouse.move(px.x + 60, px.y - 40, { steps: 8 });
await page.mouse.up();
await page.waitForTimeout(6000);
const after = await page.evaluate(() => document.querySelectorAll("aside ol li").length);
const labels = await page.evaluate(() => [...document.querySelectorAll("[data-wp]")].sort((a, b) => +a.dataset.wp - +b.dataset.wp).map((e) => e.textContent));
console.log("rows before:", before, "after:", after, "markers:", JSON.stringify(labels));
await page.screenshot({ path: "/tmp/drag-insert.png" });
await b.close();
