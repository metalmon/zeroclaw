import { AcpJsonRpcError } from './acp';
import { ApiError } from './api';
import { t } from './i18n';

/**
 * Central client-side mapper for panel-visible SERVER error messages.
 *
 * The server has no per-request localization — every error it returns is
 * plain English. Rather than translate server-side, panel-facing error
 * sites carry a stable, machine-readable code the client can map to a
 * localized headline:
 *   - ACP JSON-RPC error frames: `data.reason` (see `AcpJsonRpcError`).
 *   - REST `ConfigApiError` bodies: `.code` (see `ApiError`, `ConfigApiCode`).
 *
 * When the reason/code is present and mapped, this returns the localized
 * headline (`t('acp_error.<reason>')` / `t('error.<code>')`) with the
 * server's own English `message` appended as detail. When it is absent or
 * unmapped, this falls back to the server's `message` verbatim — never a
 * raw translation key — so an as-yet-untagged error site still reads as a
 * normal (English) error instead of a broken-looking `acp_error.foo`.
 */
export function formatServerError(err: unknown, fallback: string): string {
  if (err instanceof AcpJsonRpcError) {
    if (err.reason && ACP_ERROR_REASONS.has(err.reason)) {
      return `${t(`acp_error.${err.reason}`)}: ${err.message}`;
    }
    return err.message;
  }
  if (err instanceof ApiError) {
    const { code, message } = err.envelope;
    if (CONFIG_API_CODES.has(code)) {
      return `${t(`error.${code}`)}: ${message}`;
    }
    return message;
  }
  if (err instanceof Error) return err.message;
  return fallback;
}

// Kept as an explicit set (rather than deriving from the catalog) so an
// unrelated `acp_error.*`/`error.*` catalog addition can never silently
// start being treated as a reason/code this mapper recognizes.
const ACP_ERROR_REASONS = new Set([
  'pair_first',
  'not_entitled',
  'invalid_pair_code',
  'pending_principal',
  'paired_not_entitled',
  'agent_not_permitted',
]);

// Mirrors `ConfigApiCode` (crates/zeroclaw-config/src/api_error.rs).
const CONFIG_API_CODES = new Set([
  'path_not_found',
  'validation_failed',
  'config_changed_externally',
  'reload_failed',
  'op_not_supported',
  'secret_test_forbidden',
  'value_type_mismatch',
  'required_field_empty',
  'invalid_numeric_range',
  'invalid_format',
  'invalid_enum_variant',
  'dangling_reference',
  'forbidden',
  'conflict',
  'internal_error',
]);
