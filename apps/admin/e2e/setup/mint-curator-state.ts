// Mint a curator JWT and write a Playwright storageState file so all
// tests start authenticated. Run via `npm run e2e:setup`. The JWT is
// signed with the same JWT_SECRET the tileserver was launched with so
// the Axum `RequireRole<Curator>` extractor accepts it.

import { createHmac } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const SECRET = process.env.JWT_SECRET ?? "testsecret-must-be-long-enough-for-hs256-validation-yes-please";
const BASE = process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:8090";

function b64url(buf: Buffer | string): string {
  return Buffer.from(buf)
    .toString("base64")
    .replace(/=+$/, "")
    .replaceAll("+", "-")
    .replaceAll("/", "_");
}

function mint(): string {
  const header = { alg: "HS256", typ: "JWT" };
  const claims = {
    sub: "11111111-1111-1111-1111-111111111111",
    email: "sigmundsgranaas@gmail.com",
    exp: Math.floor(Date.now() / 1000) + 3600,
    "http://schemas.microsoft.com/ws/2008/06/identity/claims/role": [
      "curator",
      "admin",
    ],
  };
  const h = b64url(JSON.stringify(header));
  const c = b64url(JSON.stringify(claims));
  const sig = createHmac("sha256", SECRET).update(`${h}.${c}`).digest();
  return `${h}.${c}.${b64url(sig)}`;
}

const token = mint();
const baseUrl = new URL(BASE);
const state = {
  cookies: [
    {
      name: "access_token",
      value: token,
      domain: baseUrl.hostname,
      path: "/",
      expires: Math.floor(Date.now() / 1000) + 3600,
      httpOnly: false,
      secure: false,
      sameSite: "Lax" as const,
    },
  ],
  origins: [],
};

const outPath = resolve(import.meta.dirname ?? ".", "../.auth/curator.json");
mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, JSON.stringify(state, null, 2));
console.log(`Wrote ${outPath} with curator JWT (exp +60min)`);
