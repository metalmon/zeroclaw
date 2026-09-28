// Which control renderers fit as a compact control in the right column of a
// field row. Everything else (arrays, object-arrays, secrets, provider-model,
// directory-picker, multiline) renders as a full-width block under the label;
// those extra conditions are ORed in by FieldRow on top of this predicate.
const INLINE = new Set(['bool', 'number', 'select', 'alias-ref', 'text']);

export function isInlineControl(renderer: string): boolean {
  return INLINE.has(renderer);
}
