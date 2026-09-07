use serde::{Deserialize, Serialize};

/// Fork-local principal → allowed-agents map (`[[authz.principals]]`).
/// Absent/empty ⇒ authz not enforced (legacy shared-operator).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct AuthzConfig {
    #[serde(default, rename = "principals")]
    pub principals: Vec<PrincipalRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
pub struct PrincipalRecord {
    pub id: String,
    pub allowed_agents: Vec<String>,
    #[serde(default)]
    pub device_ids: Vec<String>,
    #[serde(default)]
    pub token_hashes: Vec<String>,
}

impl AuthzConfig {
    /// Authz is enforced once any principal is configured.
    pub fn is_enforced(&self) -> bool {
        !self.principals.is_empty()
    }

    /// Find the principal a bearer maps to, by token-hash first then device id.
    pub fn lookup(&self, token_hash: &str, device_id: Option<&str>) -> Option<&PrincipalRecord> {
        self.principals.iter().find(|p| {
            p.token_hashes.iter().any(|h| h == token_hash)
                || device_id.is_some_and(|d| p.device_ids.iter().any(|x| x == d))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> AuthzConfig {
        toml::from_str(
            r#"
            [[principals]]
            id = "alice"
            allowed_agents = ["crm-bot"]
            token_hashes = ["abc123"]

            [[principals]]
            id = "admin"
            allowed_agents = ["*"]
            device_ids = ["dev-1"]
            "#,
        )
        .unwrap()
    }

    #[test]
    fn empty_config_is_not_enforced() {
        assert!(!AuthzConfig::default().is_enforced());
    }

    #[test]
    fn nonempty_config_is_enforced() {
        assert!(cfg().is_enforced());
    }

    #[test]
    fn lookup_by_token_hash() {
        let c = cfg();
        assert_eq!(c.lookup("abc123", None).unwrap().id, "alice");
        assert!(c.lookup("nope", None).is_none());
    }

    #[test]
    fn lookup_by_device_id() {
        let c = cfg();
        assert_eq!(c.lookup("other", Some("dev-1")).unwrap().id, "admin");
    }
}
