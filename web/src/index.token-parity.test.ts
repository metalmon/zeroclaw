import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const themes = JSON.parse(readFileSync(join(here, "contexts/themes.json"), "utf8"));
const css = readFileSync(join(here, "index.css"), "utf8");

test("themes.json uses Thunderbolt token names, no legacy pc-", () => {
  const keys = themes.flatMap((t: { vars: Record<string, string> }) => Object.keys(t.vars));
  assert.equal(keys.some((k: string) => k.startsWith("--pc-")), false);
  assert.ok(keys.includes("--color-background"), "has --color-background");
  assert.ok(keys.includes("--color-brand"), "has --color-brand");
});

test("volt-light ground is the Thunderbolt cream", () => {
  const light = themes.find((t: { id: string }) => t.id === "volt-light") as { vars: Record<string, string> };
  assert.equal(light.vars["--color-background"], "#f5f2ee");
  assert.equal(light.vars["--color-foreground"], "#262120");
});

test("primary is the neutral ink; accent is a distinct beige mix (not primary)", () => {
  // primary (was --pc-accent) is a concrete neutral; accent is a derived beige tint.
  assert.match(css, /--color-primary:\s*#393530/);
  assert.match(css, /--color-accent:\s*color-mix\(/);
  // themes.json must NOT carry an --color-accent override (it is derived).
  const light = themes.find((t: { id: string }) => t.id === "volt-light") as { vars: Record<string, string> };
  assert.equal("--color-accent" in light.vars, false);
});

test("no legacy pc- token remains in index.css", () => {
  assert.equal(/--pc-/.test(css), false);
});

function collectSrcFiles(dir: string, acc: string[]): void {
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    if (statSync(p).isDirectory()) {
      if (entry !== "node_modules" && entry !== "dist") collectSrcFiles(p, acc);
    } else if (
      /\.(ts|tsx|css)$/.test(p) &&
      !/\.test\./.test(p) &&
      !p.includes("token-codemod")
    ) {
      acc.push(p);
    }
  }
}

test("no residual pc- design token or class anywhere in src", () => {
  const files: string[] = [];
  collectSrcFiles(here, files);
  const offenders = files.filter((f) => {
    const txt = readFileSync(f, "utf8");
    // side-scoped classes (border-t-pc-*), var refs, and raw --pc- tokens
    return /[a-z]-pc-[a-z]/.test(txt) || /var\(--pc-/.test(txt) || /--pc-[a-z]/.test(txt);
  });
  assert.deepEqual(
    offenders.map((f) => f.slice(here.length + 1)),
    [],
    "these files still reference legacy pc- tokens/classes",
  );
});
