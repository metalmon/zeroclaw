// Guards against new hardcoded-English regressions bypassing the i18n
// catalog (src/lib/i18n.ts + src/locales/*.ts). The catalog coverage check
// (check-i18n.mjs) only sees strings that already go through `t()` — a
// literal like `placeholder="6-digit code"` never reaches it. This script
// closes that gap for the highest-value, lowest-noise attribute set.
//
// Scope (see report for the reasoning):
//   HARD-FAIL — string-literal values on `placeholder=`, `title=`, and
//   `aria-label=` JSX attributes that read as English prose. These are
//   small in number, easy to eyeball, and every prior regression this repo
//   has hit (App.tsx, SkillsBundleEditor.tsx, Config.tsx) was one of these
//   three. A literal wrapped in `{t(...)}` / `{tLocale(...)}` / `{plural(...)}`
//   / a template literal / a bare variable reference is not a plain string
//   literal and is out of scope for this check (it either already routes
//   through the catalog, or is a dynamic value the checker can't judge).
//
//   WARN-ONLY — bare JSX text nodes (e.g. `<label>Name</label>`). The repo
//   has a large pre-existing backlog of these (see BARE_TEXT_BACKLOG_HINT
//   below, and the task-5 report for the counted snapshot); hard-failing on
//   them today would block CI on unrelated pages. This check reports a
//   count so the backlog is visible without gating the build on it.
//
// Run standalone: `node scripts/check-no-literal-english.mjs`.

import { readFileSync, readdirSync } from "node:fs";
import { extname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const repoRoot = join(__dirname, "..");
const SRC_DIR = join(repoRoot, "src");

const TARGET_ATTRS = ["placeholder", "title", "aria-label"];
const SCAN_EXTENSIONS = new Set([".tsx"]);
const EXCLUDE_SUFFIXES = [".test.tsx", ".test.ts"];

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

export function listSourceFiles(dir = SRC_DIR) {
  const out = [];
  const entries = readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...listSourceFiles(full));
      continue;
    }
    if (!SCAN_EXTENSIONS.has(extname(entry.name))) continue;
    if (EXCLUDE_SUFFIXES.some((suffix) => entry.name.endsWith(suffix))) continue;
    out.push(full);
  }
  return out;
}

// ---------------------------------------------------------------------------
// a) hard-fail: literal-English in placeholder / title / aria-label
// ---------------------------------------------------------------------------

// Matches `attr="..."`, `attr='...'`, `attr={"..."}`, `attr={'...'}` — i.e.
// a plain string literal, optionally wrapped in a single pair of braces.
// Anything else after `=` (a call, a template literal, a bare identifier,
// a member expression) is left alone: it's not a literal this check can
// safely judge, and — for `t(...)`/`tLocale(...)`/`plural(...)`/`fmt*(...)`
// — is exactly the escape hatch the fix is supposed to route through.
const ATTR_RE =
  /\b(placeholder|title|aria-label)\s*=\s*(?:\{\s*)?(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')(?:\s*\})?/g;

const URL_RE = /https?:\/\/\S+/gi;
const FORMAT_TOKEN_RE = /%[sd]|\{[^{}]*\}/g;
const CSS_UNIT_WORD_RE = /^(px|rem|em|vh|vw|vmin|vmax|ms|deg|fr)$/i;
const ALPHA_WORD_RE = /[A-Za-z]{3,}/g;

/**
 * True when `value` contains at least one ASCII alphabetic word (>= 3
 * letters) that isn't purely a technical token — i.e. it reads as English
 * prose that belongs in the i18n catalog instead of hardcoded in JSX.
 *
 * Allowlisted as "technical, not text": URLs, `%s`/`%d`-style format
 * tokens, stray `{...}` placeholders, bare numbers/symbols, and bare
 * CSS-unit words (px/rem/...).
 */
export function isLiteralEnglishText(value) {
  if (typeof value !== "string") return false;
  const trimmed = value.trim();
  if (trimmed.length === 0) return false;

  const stripped = trimmed.replace(URL_RE, " ").replace(FORMAT_TOKEN_RE, " ");
  const words = stripped.match(ALPHA_WORD_RE) ?? [];
  const textWords = words.filter((w) => !CSS_UNIT_WORD_RE.test(w));
  return textWords.length > 0;
}

export function findAttrViolations(source, filePath) {
  const violations = [];
  const lines = source.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    ATTR_RE.lastIndex = 0;
    let match;
    while ((match = ATTR_RE.exec(line)) !== null) {
      const attr = match[1];
      const value = match[2] !== undefined ? match[2] : match[3];
      if (!isLiteralEnglishText(value)) continue;
      violations.push({
        file: filePath,
        line: i + 1,
        attr,
        value,
      });
    }
  }
  return violations;
}

// ---------------------------------------------------------------------------
// b) warn-only: bare JSX text nodes
// ---------------------------------------------------------------------------

// Approximate bare-text-node detector: a `>...<` span with no nested tags
// or `{}` interpolation, containing an English-looking word. This is a
// regex heuristic, not a JSX parse — it will both under- and over-count
// (e.g. it can't see across multi-line text, and can pick up incidental
// `>text<` shapes inside non-JSX strings), which is why it stays WARN-only:
// it's a backlog-size signal, not a correctness gate.
const BARE_TEXT_RE = />([^<>{}\n]+)</g;

export function countBareTextWarnings(source) {
  let count = 0;
  let match;
  BARE_TEXT_RE.lastIndex = 0;
  while ((match = BARE_TEXT_RE.exec(source)) !== null) {
    if (isLiteralEnglishText(match[1])) count++;
  }
  return count;
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

function main() {
  const files = listSourceFiles();
  const allViolations = [];
  let bareTextWarnings = 0;

  for (const file of files) {
    const source = readFileSync(file, "utf8");
    const rel = relative(repoRoot, file).split("\\").join("/");
    allViolations.push(...findAttrViolations(source, rel));
    bareTextWarnings += countBareTextWarnings(source);
  }

  console.log(
    `no-literal-english check: scanned ${files.length} .tsx file(s) under src/`,
  );

  console.log(
    `\nWARN — bare JSX text nodes that look like hardcoded English: ${bareTextWarnings} ` +
      `(heuristic count, not gated; pre-existing backlog, see task-5 report)`,
  );

  if (allViolations.length > 0) {
    console.error(
      `\nFAIL (${allViolations.length}) — literal English in placeholder/title/aria-label:`,
    );
    for (const v of allViolations) {
      console.error(`  - ${v.file}:${v.line} ${v.attr}="${v.value}"`);
    }
    process.exit(1);
  }

  console.log("\nno-literal-english check passed.");
}

const isMain = process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url;
if (isMain) {
  main();
}
