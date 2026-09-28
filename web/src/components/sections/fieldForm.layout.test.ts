import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isInlineControl } from './fieldForm.layout.ts';

test('inline renderers: bool/number/select/alias-ref/text', () => {
  for (const r of ['bool', 'number', 'select', 'alias-ref', 'text']) {
    assert.equal(isInlineControl(r), true, r);
  }
});

test('wide renderers: array/object-array/secret', () => {
  for (const r of ['array', 'object-array', 'secret']) {
    assert.equal(isInlineControl(r), false, r);
  }
});

test('unknown renderer defaults to wide (safe)', () => {
  assert.equal(isInlineControl('whatever'), false);
});
