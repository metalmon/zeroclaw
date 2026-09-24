import { filterByAllow } from "./allowlist.ts";
import assert from "node:assert";
import { test } from "node:test";

test("filterByAllow keeps only allowed keys, order preserved", () => {
  const items = [{k:"telegram"},{k:"discord"},{k:"email"},{k:"notion"}];
  const out = filterByAllow(items, (i)=>i.k, ["telegram","email"]);
  assert.deepStrictEqual(out.map(i=>i.k), ["telegram","email"]);
});
test("null allow = passthrough (no filtering)", () => {
  const items = [{k:"telegram"},{k:"discord"}];
  assert.deepStrictEqual(filterByAllow(items,(i)=>i.k,null), items);
});
test("empty allow = passthrough (disabled)", () => {
  const items = [{k:"telegram"},{k:"discord"}];
  assert.deepStrictEqual(filterByAllow(items,(i)=>i.k,[]), items);
});
