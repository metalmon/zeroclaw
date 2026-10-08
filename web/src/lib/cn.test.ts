import assert from "node:assert/strict";
import test from "node:test";

import { cn } from "./cn.ts";

test("cn merges and dedups conflicting tailwind classes", () => {
  assert.equal(cn("px-2", "px-4"), "px-4");
});

test("cn drops falsy values and joins the rest", () => {
  assert.equal(cn("a", false && "b", undefined, "c"), "a c");
});
