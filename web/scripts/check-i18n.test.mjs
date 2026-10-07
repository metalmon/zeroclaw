import { test } from "node:test";
import assert from "node:assert/strict";

import {
  extractObjectLiteral,
  checkCoverage,
  checkPlaceholderParity,
  checkSplitKeyOrphans,
  checkEnglishLeak,
  checkPluralCoverage,
  findPluralBases,
  looksLikeEnglishLeak,
  runChecks,
  loadEnCatalog,
  loadRuCatalog,
  loadSchemaPaths,
  normalizeConfigFieldPath,
  fieldCatalogPaths,
  checkFieldCatalogOrphans,
  checkHighTrafficFieldCoverage,
  HIGH_TRAFFIC_FIELD_PREFIXES,
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

test("looksLikeEnglishLeak ignores ASCII letters inside {placeholder} tokens", () => {
  // format-only value: the only letters are placeholder names ("count",
  // "total"), which are not translatable content and must not trip the
  // ASCII-ratio test on their own.
  assert.equal(looksLikeEnglishLeak("{count} / {total}"), false);
});

test("looksLikeEnglishLeak treats a URL-only value as not a leak", () => {
  assert.equal(looksLikeEnglishLeak("https://example.com/docs"), false);
});

test("looksLikeEnglishLeak ignores ASCII letters inside a bare URL alongside symbols", () => {
  // no real translatable words here, just a symbol and a URL — the URL's
  // ASCII letters must not be counted against the ratio.
  assert.equal(looksLikeEnglishLeak("→ https://example.com/docs"), false);
});

test("looksLikeEnglishLeak still flags a real English sentence", () => {
  assert.equal(looksLikeEnglishLeak("Save the configuration"), true);
});

test("looksLikeEnglishLeak still allows a proper Russian value", () => {
  assert.equal(looksLikeEnglishLeak("Сохранить конфигурацию"), false);
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

// ---------------------------------------------------------------------------
// e) plural-family coverage (ru needs one/few/many where en only has one/other)
// ---------------------------------------------------------------------------

test("findPluralBases detects a family from its _other sibling", () => {
  const en = { "runs.count_one": "{n} run", "runs.count_other": "{n} runs" };
  assert.deepEqual([...findPluralBases(en)], ["runs.count"]);
});

test("findPluralBases does not misdetect a lone _one key with no _other sibling", () => {
  // this key ends in "_one" but is not a plural family — it must not be
  // flagged as one, or checkPluralCoverage would demand ru forms that were
  // never meant to exist
  const en = { "fieldform.no_entries_add_one": "No entries. Click \"+ Add\" to add one." };
  assert.deepEqual([...findPluralBases(en)], []);
});

test("checkPluralCoverage fails when ru is missing a required plural category", () => {
  const en = { "runs.count_one": "{n} run", "runs.count_other": "{n} runs" };
  const ru = {
    "runs.count_one": "{n} запуск",
    "runs.count_many": "{n} запусков",
    // "runs.count_few" missing
  };
  const { failures } = checkPluralCoverage(en, ru);
  assert.equal(failures.length, 1);
  assert.match(failures[0], /runs\.count_few/);
});

test("checkPluralCoverage passes when ru ships one/few/many", () => {
  const en = { "runs.count_one": "{n} run", "runs.count_other": "{n} runs" };
  const ru = {
    "runs.count_one": "{n} запуск",
    "runs.count_few": "{n} запуска",
    "runs.count_many": "{n} запусков",
  };
  const { failures } = checkPluralCoverage(en, ru);
  assert.deepEqual(failures, []);
});

test("checkPluralCoverage is independent of the coverage allowlist — it always applies", () => {
  const en = { "unlisted.count_one": "{n} thing", "unlisted.count_other": "{n} things" };
  const ru = {}; // nothing translated, and "unlisted." is not allowlisted anywhere
  const { failures } = checkPluralCoverage(en, ru);
  assert.equal(failures.length, 3);
  assert.ok(failures.some((f) => f.includes("unlisted.count_one")));
  assert.ok(failures.some((f) => f.includes("unlisted.count_few")));
  assert.ok(failures.some((f) => f.includes("unlisted.count_many")));
});

test("runChecks fails when a plural family is missing a ru category", () => {
  const en = {
    "roles.title": "Roles",
    "runs.count_one": "{n} run",
    "runs.count_other": "{n} runs",
  };
  const ru = {
    "roles.title": "Роли",
    "runs.count_one": "{n} запуск",
    "runs.count_few": "{n} запуска",
    // "runs.count_many" missing
  };
  const { failures } = runChecks(en, ru, ["roles."]);
  assert.ok(failures.some((f) => f.startsWith("plural:") && f.includes("runs.count_many")));
});

// ---------------------------------------------------------------------------
// f) Russian plural-category mapping + plural() lookup/interpolation
// ---------------------------------------------------------------------------

test("Intl.PluralRules('ru') selects the expected CLDR category for each count", () => {
  const rules = new Intl.PluralRules("ru");
  assert.equal(rules.select(1), "one");
  assert.equal(rules.select(2), "few");
  assert.equal(rules.select(4), "few");
  assert.equal(rules.select(5), "many");
  assert.equal(rules.select(11), "many");
  assert.equal(rules.select(21), "one");
  assert.equal(rules.select(22), "few");
});

/**
 * Mirrors the fallback/interpolation contract of `plural()` in
 * src/lib/i18n.ts: select the CLDR category for the current locale, try
 * `<base>_<category>` then `<base>_other` then the bare `<base>` (checking
 * the current locale before falling back to en at each step), then replace
 * `{n}` in whatever string was found.
 *
 * i18n.ts can't be imported directly under plain Node for this test — it
 * transitively touches `window` at module scope (via lib/basePath.ts) — so
 * this re-implements the same small algorithm and runs it against catalogs
 * pulled from the real source files with loadEnCatalog/loadRuCatalog, which
 * verifies the actual committed translations render correctly end to end.
 */
function pluralLookup(n, base, locale, catalogs) {
  const category = new Intl.PluralRules(locale).select(n);
  const candidates = [`${base}_${category}`, `${base}_other`, base];
  let raw;
  for (const candidate of candidates) {
    raw = catalogs[locale]?.[candidate] ?? catalogs.en[candidate];
    if (raw !== undefined) break;
  }
  return (raw ?? base).replace(/\{n\}/g, String(n));
}

test("plural(n, 'runs.count') renders the correct ru form with {n} substituted", async () => {
  const en = loadEnCatalog();
  const ru = await loadRuCatalog();
  const catalogs = { en, ru };

  assert.equal(pluralLookup(1, "runs.count", "ru", catalogs), "1 запуск");
  assert.equal(pluralLookup(2, "runs.count", "ru", catalogs), "2 запуска");
  assert.equal(pluralLookup(4, "runs.count", "ru", catalogs), "4 запуска");
  assert.equal(pluralLookup(5, "runs.count", "ru", catalogs), "5 запусков");
  assert.equal(pluralLookup(11, "runs.count", "ru", catalogs), "11 запусков");
  assert.equal(pluralLookup(21, "runs.count", "ru", catalogs), "21 запуск");
  assert.equal(pluralLookup(22, "runs.count", "ru", catalogs), "22 запуска");
});

test("plural() falls back to the en form for a locale with no ru-specific catalog", () => {
  const en = { "runs.count_one": "{n} run", "runs.count_other": "{n} runs" };
  const catalogs = { en, fr: {} };
  assert.equal(pluralLookup(1, "runs.count", "fr", catalogs), "1 run");
  assert.equal(pluralLookup(3, "runs.count", "fr", catalogs), "3 runs");
});

// ---------------------------------------------------------------------------
// g) config.field.* schema-audit (normalizeConfigFieldPath + orphan/coverage)
// ---------------------------------------------------------------------------

test("normalizeConfigFieldPath collapses a known map's instance key to *", () => {
  assert.equal(
    normalizeConfigFieldPath("agents.crm-bot.model_provider"),
    "agents.*.model_provider",
  );
  assert.equal(
    normalizeConfigFieldPath("authz.principals.alice.allowed_agents"),
    "authz.principals.*.allowed_agents",
  );
  assert.equal(
    normalizeConfigFieldPath("authz.profiles.crm-view.admin"),
    "authz.profiles.*.admin",
  );
});

test("normalizeConfigFieldPath leaves a fixed (non-map) path untouched", () => {
  assert.equal(normalizeConfigFieldPath("gateway.tls.client_auth.enabled"), "gateway.tls.client_auth.enabled");
  assert.equal(normalizeConfigFieldPath("locale"), "locale");
});

test("normalizeConfigFieldPath leaves an unlisted dynamic section untouched", () => {
  // mcp.servers.<alias>.* is a map too, but it isn't in DYNAMIC_KEY_SECTIONS
  // (no catalog entries reference it yet) — normalization must not guess.
  assert.equal(
    normalizeConfigFieldPath("mcp.servers.my-server.command"),
    "mcp.servers.my-server.command",
  );
});

test("fieldCatalogPaths extracts normalized paths from label/desc key pairs, deduped", () => {
  const en = { "config.field.gateway.port.label": "Port" };
  const ru = {
    "config.field.gateway.port.label": "Порт",
    "config.field.gateway.port.desc": "TCP-порт",
    "not.a.field.key": "irrelevant",
  };
  assert.deepEqual([...fieldCatalogPaths(en, ru)].sort(), ["gateway.port"]);
});

test("checkFieldCatalogOrphans warns on a catalog path with no matching schema path", () => {
  const catalogPaths = new Set(["gateway.port", "gateway.typo_field"]);
  const schemaPaths = ["gateway.port", "gateway.host"];
  const warnings = checkFieldCatalogOrphans(catalogPaths, schemaPaths);
  assert.equal(warnings.length, 1);
  assert.match(warnings[0], /gateway\.typo_field/);
});

test("checkFieldCatalogOrphans passes when every catalog path resolves against a normalized schema path", () => {
  const catalogPaths = new Set(["agents.*.model_provider"]);
  const schemaPaths = ["agents.crm-bot.model_provider", "agents.payroll-bot.model_provider"];
  assert.deepEqual(checkFieldCatalogOrphans(catalogPaths, schemaPaths), []);
});

test("checkHighTrafficFieldCoverage reports a per-prefix backlog count, never fails", () => {
  const catalogPaths = new Set(["agents.*.enabled"]);
  const schemaPaths = [
    "agents.crm-bot.enabled",
    "agents.crm-bot.model_provider",
    "agents.payroll-bot.enabled",
    "agents.payroll-bot.model_provider",
  ];
  const warnings = checkHighTrafficFieldCoverage(catalogPaths, schemaPaths, ["agents.*."]);
  assert.equal(warnings.length, 1);
  assert.match(warnings[0], /1\/2 field\(s\) under "agents\.\*\."/);
});

test("checkHighTrafficFieldCoverage reports nothing for a fully-covered prefix", () => {
  const catalogPaths = new Set(["gateway.port", "gateway.host"]);
  const schemaPaths = ["gateway.port", "gateway.host"];
  assert.deepEqual(checkHighTrafficFieldCoverage(catalogPaths, schemaPaths, ["gateway."]), []);
});

test("the committed config.field.* catalog has no orphans against the live schema fixture", async () => {
  const en = loadEnCatalog();
  const ru = await loadRuCatalog();
  const schemaPaths = loadSchemaPaths();
  // Fixture may be absent in some checkouts (it's a committed dashboard dev
  // fixture, not generated at test time) — skip rather than false-fail.
  if (schemaPaths.length === 0) return;
  const catalogPaths = fieldCatalogPaths(en, ru);
  const warnings = checkFieldCatalogOrphans(catalogPaths, schemaPaths);
  assert.deepEqual(warnings, []);
});

test("HIGH_TRAFFIC_FIELD_PREFIXES is a non-empty, deliberate list", () => {
  assert.ok(HIGH_TRAFFIC_FIELD_PREFIXES.length > 0);
});
