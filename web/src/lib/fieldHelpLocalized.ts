import { fieldDescriptions } from './api-descriptions';
import { fieldDescriptionsRu } from './api-descriptions.ru';
import { getLocale } from './i18n';

/**
 * Locale-aware field help lookup. Returns the RU translation when the active
 * locale is `ru` and a translated entry exists, else falls back to the EN
 * (codegen'd) help text. Missing RU keys transparently fall back to EN until
 * the machine-translation job backfills `api-descriptions.ru.ts`.
 */
export function localizedFieldHelp(schema: string, field: string): string | undefined {
  const en = fieldDescriptions[schema]?.[field];
  if (getLocale() === 'ru') {
    return fieldDescriptionsRu[schema]?.[field] ?? en;
  }
  return en;
}
