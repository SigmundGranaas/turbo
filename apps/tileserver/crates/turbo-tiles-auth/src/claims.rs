use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Role names match the rows seeded by the .NET migration
/// `SeedAdminRoles`. The Rust side treats them as opaque strings so
/// adding a new role doesn't require a code change here.
pub type Role = String;

/// JWT claims as emitted by the .NET `JwtService`.
///
/// The `role` claim arrives under the URN
/// `http://schemas.microsoft.com/ws/2008/06/identity/claims/role`
/// because .NET serializes `ClaimTypes.Role` that way. We rename it on
/// deserialize so consumers see the conventional `roles: Vec<String>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    #[serde(default)]
    pub email: Option<String>,
    pub exp: i64,
    #[serde(default)]
    pub iss: Option<String>,
    #[serde(default)]
    pub aud: Option<String>,
    #[serde(
        rename = "http://schemas.microsoft.com/ws/2008/06/identity/claims/role",
        default,
        deserialize_with = "deserialize_roles"
    )]
    pub roles: Vec<Role>,
}

impl Claims {
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

/// .NET emits a single role as a bare string and multiple roles as a
/// JSON array. Accept both.
fn deserialize_roles<'de, D>(de: D) -> Result<Vec<Role>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(de)?;
    match value {
        serde_json::Value::Null => Ok(Vec::new()),
        serde_json::Value::String(s) => Ok(vec![s]),
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .map(|v| match v {
                serde_json::Value::String(s) => Ok(s),
                other => Err(D::Error::custom(format!(
                    "expected string role, got {other}"
                ))),
            })
            .collect(),
        other => Err(D::Error::custom(format!(
            "expected string or array of strings for roles, got {other}"
        ))),
    }
}
