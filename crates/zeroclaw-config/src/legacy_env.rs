//! `VOLTD_*` env var support with `ZEROCLAW_*` accept-both fallback.
//!
//! The rebrand (`zeroclaw` → `voltd`) renames the runtime-critical env vars
//! from `ZEROCLAW_*` to `VOLTD_*`. Reads must prefer the new name but keep
//! honoring the old one so installed daemons and external scripts that still
//! export `ZEROCLAW_*` keep working. Writes / new documentation should use
//! only the new name; the old name is accept-only, never advertised.

/// Read `new`, falling back to `old` when `new` is unset. Prefers the new
/// (`VOLTD_*`) name; never removes the legacy (`ZEROCLAW_*` / `ZC_*`) fallback.
pub fn env_with_legacy(new: &str, old: &str) -> Option<String> {
    std::env::var(new).ok().or_else(|| std::env::var(old).ok())
}

/// `(new, legacy)` pairs — runtime-critical vars only. Used both to wire
/// individual env reads and to extend the CLI's env-name allowlist so both
/// spellings validate.
pub const LEGACY_ENV_ALIASES: &[(&str, &str)] = &[
    ("VOLTD_CONFIG_DIR", "ZEROCLAW_CONFIG_DIR"),
    ("VOLTD_DATA_DIR", "ZEROCLAW_DATA_DIR"),
    ("VOLTD_WORKSPACE", "ZEROCLAW_WORKSPACE"),
    ("VOLTD_SOCKET", "ZEROCLAW_SOCKET"),
    ("VOLTD_API_KEY", "ZEROCLAW_API_KEY"),
    ("VOLTD_GATEWAY_TOKEN", "ZEROCLAW_GATEWAY_TOKEN"),
    ("VOLTD_ACP_BRIDGE_TOKEN", "ZEROCLAW_ACP_BRIDGE_TOKEN"),
    ("VOLTD_ACP_PAIRING_CODE", "ZEROCLAW_ACP_PAIRING_CODE"),
    ("VOLTD_AUDIT_SIGNING_KEY", "ZEROCLAW_AUDIT_SIGNING_KEY"),
    ("VOLTD_CA_PASSPHRASE", "ZEROCLAW_CA_PASSPHRASE"),
    ("VOLTD_CA_PASSPHRASE_FILE", "ZEROCLAW_CA_PASSPHRASE_FILE"),
    ("VOLTD_PLUGIN_REGISTRY_URL", "ZEROCLAW_PLUGIN_REGISTRY_URL"),
    ("VOLTD_SLACK_BOT_TOKEN", "ZEROCLAW_SLACK_BOT_TOKEN"),
    ("VOLTD_OPEN_SKILLS_ENABLED", "ZEROCLAW_OPEN_SKILLS_ENABLED"),
    ("VOLTD_OPEN_SKILLS_DIR", "ZEROCLAW_OPEN_SKILLS_DIR"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // env::var/set_var/remove_var are process-global; guard this test so it
    // cannot interleave with another test in the same binary that touches
    // these same var names.
    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn env_prefers_new_then_legacy() {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // SAFETY: serialized by LOCK; no other thread in this test binary
        // touches VOLTD_DATA_DIR / ZEROCLAW_DATA_DIR while the guard is held.
        unsafe {
            std::env::remove_var("VOLTD_DATA_DIR");
            std::env::remove_var("ZEROCLAW_DATA_DIR");
        }
        assert_eq!(env_with_legacy("VOLTD_DATA_DIR", "ZEROCLAW_DATA_DIR"), None);

        // SAFETY: see above.
        unsafe {
            std::env::set_var("ZEROCLAW_DATA_DIR", "/legacy");
        }
        assert_eq!(
            env_with_legacy("VOLTD_DATA_DIR", "ZEROCLAW_DATA_DIR").as_deref(),
            Some("/legacy")
        );

        // SAFETY: see above.
        unsafe {
            std::env::set_var("VOLTD_DATA_DIR", "/new");
        }
        assert_eq!(
            env_with_legacy("VOLTD_DATA_DIR", "ZEROCLAW_DATA_DIR").as_deref(),
            Some("/new")
        );

        // SAFETY: see above.
        unsafe {
            std::env::remove_var("VOLTD_DATA_DIR");
            std::env::remove_var("ZEROCLAW_DATA_DIR");
        }
    }
}
