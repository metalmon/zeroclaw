import { test } from "node:test";
import assert from "node:assert/strict";

import {
  isLiteralEnglishText,
  findAttrViolations,
  findTemplateLiteralWarnings,
  hasStaticProseWords,
  countBareTextWarnings,
} from "./check-no-literal-english.mjs";

// ---------------------------------------------------------------------------
// isLiteralEnglishText
// ---------------------------------------------------------------------------

test("isLiteralEnglishText flags plain English prose", () => {
  assert.equal(isLiteralEnglishText("6-digit code"), true);
  assert.equal(isLiteralEnglishText("Global channel settings"), true);
});

test("isLiteralEnglishText allows a bare URL", () => {
  assert.equal(isLiteralEnglishText("https://example.com/docs"), false);
});

test("isLiteralEnglishText allows numbers, symbols, and css units", () => {
  assert.equal(isLiteralEnglishText("6"), false);
  assert.equal(isLiteralEnglishText("×"), false);
  assert.equal(isLiteralEnglishText("1rem"), false);
  assert.equal(isLiteralEnglishText("%s"), false);
});

test("isLiteralEnglishText allows empty/whitespace-only values", () => {
  assert.equal(isLiteralEnglishText(""), false);
  assert.equal(isLiteralEnglishText("   "), false);
});

// ---------------------------------------------------------------------------
// findAttrViolations — the hard-fail gate
// ---------------------------------------------------------------------------

test("findAttrViolations FAILS a hardcoded English placeholder", () => {
  const source = ['<input placeholder="6-digit code" />'].join("\n");
  const violations = findAttrViolations(source, "fixture.tsx");
  assert.equal(violations.length, 1);
  assert.equal(violations[0].attr, "placeholder");
  assert.equal(violations[0].value, "6-digit code");
  assert.equal(violations[0].line, 1);
});

test("findAttrViolations PASSES a placeholder routed through t()", () => {
  const source = ["<input placeholder={t('pairing.code_input_placeholder')} />"].join("\n");
  const violations = findAttrViolations(source, "fixture.tsx");
  assert.deepEqual(violations, []);
});

test("findAttrViolations PASSES a URL value", () => {
  const source = ['<a title="https://example.com/docs">docs</a>'].join("\n");
  const violations = findAttrViolations(source, "fixture.tsx");
  assert.deepEqual(violations, []);
});

test("findAttrViolations catches title and aria-label too", () => {
  const source = [
    '<span title="Loader-managed skill" />',
    '<button aria-label="Remove option" />',
  ].join("\n");
  const violations = findAttrViolations(source, "fixture.tsx");
  assert.equal(violations.length, 2);
  assert.equal(violations[0].attr, "title");
  assert.equal(violations[1].attr, "aria-label");
});

test("findAttrViolations PASSES a string-literal wrapped in braces around t()", () => {
  // tLocale/plural calls are expressions, not plain string literals — only
  // a literal directly (optionally brace-wrapped) after `=` is in scope.
  const source = ["<input placeholder={tLocale('x', locale)} />"].join("\n");
  const violations = findAttrViolations(source, "fixture.tsx");
  assert.deepEqual(violations, []);
});

test("findAttrViolations ignores unrelated attributes", () => {
  const source = ['<div className="flex items-center gap-2" data-testid="thing" />'].join("\n");
  const violations = findAttrViolations(source, "fixture.tsx");
  assert.deepEqual(violations, []);
});

// ---------------------------------------------------------------------------
// findTemplateLiteralWarnings / hasStaticProseWords — the second warn-only
// category (template-literal placeholder/title/aria-label values)
// ---------------------------------------------------------------------------

test("findTemplateLiteralWarnings WARNS on a template literal with static English text", () => {
  const source = ["<button aria-label={`Remove tag ${x}`} />"].join("\n");
  const warnings = findTemplateLiteralWarnings(source, "fixture.tsx");
  assert.equal(warnings.length, 1);
  assert.equal(warnings[0].attr, "aria-label");
  assert.equal(warnings[0].line, 1);
});

test("findTemplateLiteralWarnings does not flag a template literal with no static words", () => {
  const source = ["<button aria-label={`${count}`} />"].join("\n");
  const warnings = findTemplateLiteralWarnings(source, "fixture.tsx");
  assert.deepEqual(warnings, []);
});

test("findTemplateLiteralWarnings ignores config-path-shaped static text", () => {
  // "open this config path" tooltips (`agents.${alias}`) are dotted
  // identifier fragments, not prose — must not join the a11y backlog.
  const source = [
    "<div title={`${t('agent.open_config_prefix')}agents.${alias}${t('agent.open_config_suffix')}`} />",
  ].join("\n");
  const warnings = findTemplateLiteralWarnings(source, "fixture.tsx");
  assert.deepEqual(warnings, []);
});

test("hasStaticProseWords: dotted tokens don't count as words", () => {
  assert.equal(hasStaticProseWords("agents."), false);
  assert.equal(hasStaticProseWords("event.action ="), false);
});

test("hasStaticProseWords: whitespace-separated English words do count", () => {
  assert.equal(hasStaticProseWords("Move option   up"), true);
});

// ---------------------------------------------------------------------------
// countBareTextWarnings — the warn-only backlog signal
// ---------------------------------------------------------------------------

test("countBareTextWarnings counts bare English JSX text nodes", () => {
  const source = ["<label>Name</label>", "<span>×</span>"].join("\n");
  assert.equal(countBareTextWarnings(source), 1);
});

test("countBareTextWarnings does not count text already behind t()", () => {
  const source = "<label>{t('common.name')}</label>";
  assert.equal(countBareTextWarnings(source), 0);
});
