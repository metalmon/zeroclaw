use serde::{Deserialize, Serialize};

/// Fork-local principal → allowed-agents map (`[[authz.principals]]`).
/// Absent/empty ⇒ authz not enforced (legacy shared-operator).
///
/// Mirrors the `[mcp]` / `[[mcp.servers]]` shape: this wrapper struct
/// derives `Configurable` with `#[prefix = "authz"]`, and its `principals`
/// Vec is `#[nested]` + `#[natural_key = "id"]` so each element is a real
/// `Configurable` child (see `PrincipalRecord`) reachable at
/// `authz.principals.<id>.<field>` instead of a single opaque leaf prop.
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, zeroclaw_macros::Configurable,
)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "authz"]
pub struct AuthzConfig {
    /// Configured principals. `#[natural_key = "id"]` opts the Vec into
    /// per-element property routing (`authz.principals.<id>.<field>`),
    /// matching `mcp.servers` / `model_routes`.
    #[serde(default, rename = "principals")]
    #[nested]
    #[natural_key = "id"]
    pub principals: Vec<PrincipalRecord>,
}

/// One fork-local principal: the bearer identity (token hash / device id)
/// mapped to the set of agent aliases it may reach. `#[prefix =
/// "authz.principals"]` is the full dotted path (parent `authz` +
/// this field's own name `principals`), matching `McpServerConfig`'s
/// `#[prefix = "mcp.servers"]`. Every field carries `#[serde(default)]` so
/// `create_map_key` can default-construct a blank element from `{}` before
/// the natural key (`id`) is injected via `set_prop`.
#[derive(
    Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, zeroclaw_macros::Configurable,
)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[prefix = "authz.principals"]
pub struct PrincipalRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
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
