import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isInlineControl } from './fieldForm.layout.ts';

test('inline renderers: bool/number/select', () => {
  for (const r of ['bool', 'number', 'select']) {
    assert.equal(isInlineControl(r), true, r);
  }
});

test('wide renderers stay full-width: array/object-array/secret/text/alias-ref', () => {
  for (const r of ['array', 'object-array', 'secret', 'text', 'alias-ref']) {
    assert.equal(isInlineControl(r), false, r);
  }
});

test('unknown renderer defaults to wide (safe)', () => {
  assert.equal(isInlineControl('whatever'), false);
});
