//! JWT validation for tokens issued by the .NET `apps/api/Turbo.Auth.Api`
//! service. HS256 with the shared secret in `JwtConfig.Key`.
//!
//! Important: .NET's `ClaimTypes.Role` serializes the role claim under the
//! long URN `http://schemas.microsoft.com/ws/2008/06/identity/claims/role`,
//! not the conventional `roles`. The `Claims` deserializer renames it.

pub mod claims;
pub mod config;
pub mod extractor;

pub use claims::{Claims, Role};
pub use config::AuthConfig;
pub use extractor::{Admin, AuthError, AuthState, AuthUser, Curator, RequireRole, RoleSpec};
