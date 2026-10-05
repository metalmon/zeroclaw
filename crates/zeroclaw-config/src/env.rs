//! Operator-facing environment variables.
//!
//! The product name is Volt, so operators set `VOLTD_<NAME>`. The upstream
//! `ZEROCLAW_<NAME>` spelling is still honoured as a fallback so an
//! upstream-style deployment keeps working; when both are set, `VOLTD_` wins.

/// Prefix of the operator-facing variables.
pub const ENV_PREFIX: &str = "VOLTD_";
/// Upstream prefix, read only as a fallback.
pub const LEGACY_ENV_PREFIX: &str = "ZEROCLAW_";

/// The operator-facing name for `suffix`: `VOLTD_<suffix>`.
pub fn name(suffix: &str) -> String {
    format!("{ENV_PREFIX}{suffix}")
}

/// `VOLTD_<suffix>`, else `ZEROCLAW_<suffix>`.
pub fn var(suffix: &str) -> Result<String, std::env::VarError> {
    match std::env::var(name(suffix)) {
        Err(std::env::VarError::NotPresent) => {
            std::env::var(format!("{LEGACY_ENV_PREFIX}{suffix}"))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(name: &str, value: Option<&str>) {
        // SAFETY: the suffixes below are unique to this module's tests.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }

    #[test]
    fn product_name_wins_over_upstream_name() {
        set("VOLTD_ENV_MODULE_TEST_A", Some("volt"));
        set("ZEROCLAW_ENV_MODULE_TEST_A", Some("upstream"));
        assert_eq!(var("ENV_MODULE_TEST_A").as_deref(), Ok("volt"));
        set("VOLTD_ENV_MODULE_TEST_A", None);
        set("ZEROCLAW_ENV_MODULE_TEST_A", None);
    }

    #[test]
    fn upstream_name_is_the_fallback() {
        set("ZEROCLAW_ENV_MODULE_TEST_B", Some("upstream"));
        assert_eq!(var("ENV_MODULE_TEST_B").as_deref(), Ok("upstream"));
        set("ZEROCLAW_ENV_MODULE_TEST_B", None);
        assert_eq!(
            var("ENV_MODULE_TEST_B"),
            Err(std::env::VarError::NotPresent)
        );
    }

    #[test]
    fn name_uses_the_product_prefix() {
        assert_eq!(name("CONFIG_DIR"), "VOLTD_CONFIG_DIR");
    }
}
