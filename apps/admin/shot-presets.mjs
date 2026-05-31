import { chromium } from "@playwright/test";
const PTS=[[10.71,59.96],[10.66,60.00]]; // Marka — direct differs a lot
const b=await chromium.launch();
const page=await b.newPage({viewport:{width:1280,height:1000}});
page.on("pageerror",e=>console.log("PAGEERR",e.message));
await page.goto("http://localhost:5173/admin/dev-login",{waitUntil:"networkidle"});
await page.goto("http://localhost:5173/admin/app/plot",{waitUntil:"networkidle"});
await page.waitForFunction(()=>window.__pf&&window.__pf.map&&window.__pf.map.loaded(),null,{timeout:30000});
await page.evaluate(()=>window.__pf.map.jumpTo({center:[10.685,59.98],zoom:12.5}));
await page.waitForTimeout(1000);
// dropdown present?
const opts=await page.$$eval("aside select option", os=>os.map(o=>o.textContent));
console.log("Trip style options:", JSON.stringify(opts));
await page.evaluate(p=>window.__pf.setPoints(p),PTS);
await page.waitForTimeout(7000);
const balLen=await page.evaluate(()=>window.__pf.__lastLen??null);
// read length from the status card text instead
const lenOf=async()=> (await page.evaluate(()=>{const el=[...document.querySelectorAll("aside *")].find(e=>/\d+(\.\d+)?\s*km/.test(e.textContent)&&e.children.length===0);return el?el.textContent.trim():null;}));
const balanced=await lenOf();
// switch to Direct
await page.selectOption("aside select", "direct");
await page.waitForTimeout(7000);
const direct=await lenOf();
console.log("balanced len:",balanced,"| direct len:",direct,"| changed:", balanced!==direct);
await b.close();
