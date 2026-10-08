// Which control renderers are safe to place as a compact control in the right
// column of a field row. Restricted to the three whose branch in FieldRow's
// control ladder has no wide sub-cases: `bool` (BoolSwitch) and `select` (enum
// Select) are matched first, and `number` is only reached for numeric kinds —
// none of them overlaps the directory/provider/tool/alias pickers. Everything
// else (arrays, secrets, text with special handling, alias-refs, pickers,
// multiline) renders as a full-width block under the label; FieldRow also ORs
// in the dynamic wide conditions (validation, drift, open comment) on top.
const INLINE = new Set(['bool', 'number', 'select']);

export function isInlineControl(renderer: string): boolean {
  return INLINE.has(renderer);
}
