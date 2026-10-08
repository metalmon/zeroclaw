import assert from "node:assert/strict";
import test from "node:test";

import { buttonVariants } from "./Button.tsx";

test("default variant paints the amber->raspberry brand gradient", () => {
  const c = buttonVariants({});
  assert.match(c, /bg-brand/);
  assert.match(c, /var\(--gradient-brand\)/);
});

test("outline variant is a bordered surface, not the gradient", () => {
  const c = buttonVariants({ variant: "outline" });
  assert.match(c, /bg-background/);
  assert.doesNotMatch(c, /gradient-brand/);
});

test("ghost variant tints on hover with the beige accent", () => {
  const c = buttonVariants({ variant: "ghost" });
  assert.match(c, /hover:bg-accent/);
});
