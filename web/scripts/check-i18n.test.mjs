import { test } from "node:test";
import assert from "node:assert/strict";

import {
  extractObjectLiteral,
  checkCoverage,
  checkPlaceholderParity,
  checkSplitKeyOrphans,
  checkEnglishLeak,
  looksLikeEnglishLeak,
  runChecks,
} from "./check-i18n.mjs";

// ---------------------------------------------------------------------------
// extractObjectLiteral
// ---------------------------------------------------------------------------

test("extractObjectLiteral pulls a locale block without executing the module", () => {
  const source = [
    "import { doStuff } from './somewhere-that-does-not-exist';",
    "",
    "const translations = {",
    "  en: {",
    "    'a.b': \"hello {name}\",",
    "    'c.d': 'plain, no tokens',",
    "  },",
    "  ru: {",
    "    'a.b': \"привет {name}\",",
    "  },",
    "};",
  ].join("\n");

  const en = extractObjectLiteral(source, "en");
  assert.deepEqual(en, { "a.b": "hello {name}", "c.d": "plain, no tokens" });

  const ru = extractObjectLiteral(source, "ru");
  assert.deepEqual(ru, { "a.b": "привет {name}" });
});

test("extractObjectLiteral throws on an unknown key", () => {
  const source = "const translations = {\n  en: {\n    'a': 'b',\n  },\n};";
  assert.throws(() => extractObjectLiteral(source, "zz"));
});

// ---------------------------------------------------------------------------
// a) key coverage
// ---------------------------------------------------------------------------

test("checkCoverage fails when an allowlisted key is missing from ru", () => {
  const en = { "roles.title": "Roles", "other.x": "X" };
  const ru = {};
  const { failures, infos } = checkCoverage(en, ru, ["roles."]);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /roles\.title/);
  // non-allowlisted misses are informational only
  assert.equal(infos.length, 1);
  assert.match(infos[0], /other\.x/);
});

test("checkCoverage passes when allowlisted keys are all present", () => {
  const en = { "roles.title": "Roles", "other.x": "X" };
  const ru = { "roles.title": "Роли" };
  const { failures } = checkCoverage(en, ru, ["roles."]);
  assert.deepEqual(failures, []);
});

// ---------------------------------------------------------------------------
// b) placeholder-token parity
// ---------------------------------------------------------------------------

test("checkPlaceholderParity fails on a token mismatch", () => {
  const en = { k: "Hello {name}, you have {count} messages" };
  const ru = { k: "Привет {name}" }; // dropped {count}
  const { failures } = checkPlaceholderParity(en, ru);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /k/);
});

test("checkPlaceholderParity passes when token sets match (order-independent)", () => {
  const en = { k: "{count} of {total}" };
  const ru = { k: "{total} из {count}" };
  const { failures } = checkPlaceholderParity(en, ru);
  assert.deepEqual(failures, []);
});

test("checkPlaceholderParity ignores keys not present in ru", () => {
  const en = { k: "Hello {name}" };
  const ru = {};
  const { failures } = checkPlaceholderParity(en, ru);
  assert.deepEqual(failures, []);
});

// ---------------------------------------------------------------------------
// c) split-key orphans
// ---------------------------------------------------------------------------

test("checkSplitKeyOrphans fails when only one half of a real pair is translated", () => {
  const en = { "confirm_prefix": "Are you sure you want to ", "confirm_suffix": "?" };
  const ru = { "confirm_prefix": "Вы уверены, что хотите " }; // suffix missing
  const { failures } = checkSplitKeyOrphans(en, ru);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /confirm_suffix/);
});

test("checkSplitKeyOrphans passes when both halves are translated together", () => {
  const en = { "confirm_prefix": "Are you sure ", "confirm_suffix": "?" };
  const ru = { "confirm_prefix": "Вы уверены ", "confirm_suffix": "?" };
  const { failures } = checkSplitKeyOrphans(en, ru);
  assert.deepEqual(failures, []);
});

test("checkSplitKeyOrphans passes when neither half is translated yet", () => {
  const en = { "confirm_prefix": "Are you sure ", "confirm_suffix": "?" };
  const ru = {};
  const { failures } = checkSplitKeyOrphans(en, ru);
  assert.deepEqual(failures, []);
});

test("checkSplitKeyOrphans does not assume every _prefix has a _suffix sibling", () => {
  // lone _prefix key with no matching _suffix in en — must not be treated as a pair
  const en = { "lone_prefix": "Some standalone value" };
  const ru = {}; // untranslated, but there is no real pair so no orphan possible
  const { failures } = checkSplitKeyOrphans(en, ru);
  assert.deepEqual(failures, []);
});

// ---------------------------------------------------------------------------
// d) english-leak heuristic
// ---------------------------------------------------------------------------

test("looksLikeEnglishLeak flags a long untranslated phrase", () => {
  assert.equal(looksLikeEnglishLeak("Manage user roles and permissions"), true);
});

test("looksLikeEnglishLeak does not flag Russian text", () => {
  assert.equal(looksLikeEnglishLeak("Управление ролями и правами пользователей"), false);
});

test("looksLikeEnglishLeak allows single-token values (URLs, %s, short codes)", () => {
  assert.equal(looksLikeEnglishLeak("https://example.com/docs"), false);
  assert.equal(looksLikeEnglishLeak("%s"), false);
  assert.equal(looksLikeEnglishLeak("OLED"), false);
});

test("looksLikeEnglishLeak allows short/symbolic values", () => {
  assert.equal(looksLikeEnglishLeak("OK"), false);
  assert.equal(looksLikeEnglishLeak("v2.1"), false);
  assert.equal(looksLikeEnglishLeak("—"), false);
});

test("checkEnglishLeak only flags allowlisted keys", () => {
  const ru = {
    "roles.title": "Manage user roles and permissions",
    "other.title": "Manage other things in english",
  };
  const { failures } = checkEnglishLeak(ru, ["roles."]);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /roles\.title/);
});

test("checkEnglishLeak passes on translated allowlisted values", () => {
  const ru = { "roles.title": "Управление ролями и правами пользователей" };
  const { failures } = checkEnglishLeak(ru, ["roles."]);
  assert.deepEqual(failures, []);
});

// ---------------------------------------------------------------------------
// runChecks — combined
// ---------------------------------------------------------------------------

test("runChecks passes on a fully clean fixture", () => {
  const en = {
    "roles.title": "Roles",
    "roles.desc": "Manage roles for {user}",
    "confirm_prefix": "Are you sure ",
    "confirm_suffix": "?",
    "other.untranslated": "Not in scope yet",
  };
  const ru = {
    "roles.title": "Роли",
    "roles.desc": "Управление ролями для {user}",
    "confirm_prefix": "Вы уверены ",
    "confirm_suffix": "?",
  };
  const { failures, infos } = runChecks(en, ru, ["roles."]);
  assert.deepEqual(failures, []);
  // the untranslated non-allowlisted key is still surfaced as info
  assert.equal(infos.length, 1);
  assert.match(infos[0], /other\.untranslated/);
});

test("runChecks fails when any violation class is present", () => {
  const en = {
    "roles.title": "Roles",
    "roles.missing": "Missing entirely",
    "roles.tokens": "Hello {name}",
    "confirm_prefix": "Are you sure ",
    "confirm_suffix": "?",
  };
  const ru = {
    "roles.title": "This is still English text", // english-leak
    "roles.tokens": "Привет", // placeholder mismatch (dropped {name})
    "confirm_prefix": "Вы уверены ", // split-key orphan (suffix missing)
    // roles.missing absent entirely -> coverage failure
  };
  const { failures } = runChecks(en, ru, ["roles."]);
  assert.ok(failures.some((f) => f.startsWith("coverage:")));
  assert.ok(failures.some((f) => f.startsWith("placeholder:")));
  assert.ok(failures.some((f) => f.startsWith("split-key:")));
  assert.ok(failures.some((f) => f.startsWith("english-leak:")));
});
