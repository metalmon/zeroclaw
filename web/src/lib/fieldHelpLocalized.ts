import { fieldDescriptions } from './api-descriptions';
import { fieldDescriptionsRu } from './api-descriptions.ru';
import { getLocale } from './i18n';

/**
 * Field help is generated from upstream Rust doc comments, which still name
 * the product "ZeroClaw" and its CLI `zeroclaw`. Rebrand prose at read time
 * so the regenerated EN catalog needs no post-processing step. Identifiers
 * are kept verbatim: `ZEROCLAW_*` env vars, `~/.zeroclaw`, crate/package
 * names (`zeroclaw-labs`, `zeroclaw-runtime`, `zeroclaw_log`), wire values
 * (`http:ZeroClawOperator`, proxy scope `zeroclaw`) and quoted defaults
 * (`"ZeroClaw"`, `"hey zeroclaw"`).
 */
export function rebrandHelp(text: string): string {
  return text
    .replace(/`zeroclaw (?=[a-z])/g, '`voltd ')
    .replace(/(?<![\w"?:@])ZeroClaw(?![A-Za-z])/g, 'Volt');
}

/**
 * Locale-aware field help lookup. Returns the RU translation when the active
 * locale is `ru` and a translated entry exists, else falls back to the EN
 * (codegen'd) help text. Missing RU keys transparently fall back to EN until
 * the machine-translation job backfills `api-descriptions.ru.ts`.
 */
export function localizedFieldHelp(schema: string, field: string): string | undefined {
  const en = fieldDescriptions[schema]?.[field];
  const text = getLocale() === 'ru' ? (fieldDescriptionsRu[schema]?.[field] ?? en) : en;
  return text === undefined ? undefined : rebrandHelp(text);
}
