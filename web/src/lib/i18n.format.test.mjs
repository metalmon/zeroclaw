import assert from 'node:assert/strict';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { createJiti } from 'jiti';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// i18n.ts imports ./api -> ./basePath, which reads `window.__ZEROCLAW_BASE__`
// at module-eval time. Stub a minimal `window` before importing so the module
// graph loads cleanly under plain Node (no jsdom/happy-dom needed — the new
// formatting helpers and setLocale's `document` write are both guarded).
async function loadI18n() {
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: { __ZEROCLAW_BASE__: '' },
  });
  const jiti = createJiti(import.meta.url, { tsconfigPaths: true });
  const mod = await jiti.import(path.join(__dirname, 'i18n.ts'), { default: false });
  delete globalThis.window;
  return mod;
}

test('fmtRelative renders Cyrillic relative time under ru locale', async () => {
  const { setLocale, fmtRelative } = await loadI18n();
  setLocale('ru');
  const label = fmtRelative(-2, 'hour');
  assert.match(label, /[Ѐ-ӿ]/, `expected Cyrillic output, got "${label}"`);
  assert.match(label, /назад|час/i, `expected "назад"/"час" in "${label}"`);
});

test('fmtNumber uses ru grouping/decimal conventions', async () => {
  const { setLocale, fmtNumber } = await loadI18n();
  setLocale('ru');
  const formatted = fmtNumber(1234.5);
  // Russian locale groups with a non-breaking space and uses a comma decimal
  // separator (e.g. "1 234,5"), unlike English's "1,234.5".
  assert.match(formatted, /^1[\s ]234,5$/, `unexpected ru formatting: "${formatted}"`);
});

test('switching locale changes fmtNumber and fmtRelative output', async () => {
  const { setLocale, fmtNumber, fmtRelative } = await loadI18n();

  setLocale('en');
  const enNumber = fmtNumber(1234.5);
  const enRelative = fmtRelative(-2, 'hour');

  setLocale('ru');
  const ruNumber = fmtNumber(1234.5);
  const ruRelative = fmtRelative(-2, 'hour');

  assert.notEqual(enNumber, ruNumber);
  assert.notEqual(enRelative, ruRelative);
});

test('fmtDate/fmtTime/fmtNumber fall back gracefully on invalid input instead of throwing', async () => {
  const { setLocale, fmtDate, fmtTime, fmtNumber } = await loadI18n();
  setLocale('en');
  assert.doesNotThrow(() => fmtDate('not-a-date'));
  assert.doesNotThrow(() => fmtTime('not-a-date'));
  assert.equal(fmtNumber(Number.NaN), 'NaN');
});

test('t() interpolates {name} tokens from vars, leaving unknown tokens as-is', async () => {
  const { t, setLocale } = await loadI18n();
  setLocale('en');
  // 'nav.cmdk.more' resolves (in en) to "+{value} more — keep typing".
  assert.equal(t('nav.cmdk.more', { value: 3 }), '+3 more — keep typing');
  // No vars -> unchanged (backward-compatible) behavior: token left raw.
  assert.equal(t('nav.cmdk.more'), '+{value} more — keep typing');
  // Unknown token in the string is left untouched when not present in vars.
  assert.equal(t('nav.cmdk.more', { other: 'x' }), '+{value} more — keep typing');
});

test('setLocale updates document.documentElement.lang when a document is present', async () => {
  const { setLocale } = await loadI18n();
  const fakeDocumentElement = { lang: '' };
  Object.defineProperty(globalThis, 'document', {
    configurable: true,
    value: { documentElement: fakeDocumentElement },
  });
  try {
    setLocale('ru');
    assert.equal(fakeDocumentElement.lang, 'ru');
    setLocale('en');
    assert.equal(fakeDocumentElement.lang, 'en');
  } finally {
    delete globalThis.document;
  }
});
