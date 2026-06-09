use jsonwebtoken::{Algorithm, DecodingKey, Validation};

#[derive(Clone)]
pub struct AuthConfig {
    pub decoding_key: DecodingKey,
    pub validation: Validation,
    /// `false` when no `JWT_SECRET` was configured. The server still boots,
    /// but token-validating extractors (`AuthUser` / `RequireRole`) reject
    /// every request — public endpoints (tiles, basemap, routing) are
    /// unaffected, only the auth-gated `/admin` + debug surface is closed.
    pub enabled: bool,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("enabled", &self.enabled)
            .field("validation.algorithms", &self.validation.algorithms)
            .finish()
    }
}

impl AuthConfig {
    /// Build from env: `JWT_SECRET` (shared with .NET `JwtConfig.Key`),
    /// optional `JWT_ISSUER` and `JWT_AUDIENCE`. Errors if no secret is set —
    /// callers that should tolerate a public-only deployment use
    /// [`AuthConfig::from_env_lenient`] instead.
    pub fn from_env() -> Result<Self, AuthConfigError> {
        let secret = std::env::var("JWT_SECRET").map_err(|_| AuthConfigError::MissingSecret)?;
        let mut validation = Validation::new(Algorithm::HS256);
        if let Ok(iss) = std::env::var("JWT_ISSUER") {
            validation.set_issuer(&[iss]);
        }
        if let Ok(aud) = std::env::var("JWT_AUDIENCE") {
            validation.set_audience(&[aud]);
        }
        Ok(Self {
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            validation,
            enabled: true,
        })
    }

    /// Like [`AuthConfig::from_env`] but never fails: when `JWT_SECRET` is
    /// absent it returns a disabled config so the server can still serve its
    /// public endpoints. The caller should log that auth is off.
    pub fn from_env_lenient() -> Self {
        Self::from_env().unwrap_or_else(|_| Self::disabled())
    }

    /// A config that rejects every authenticated request. The decoding key is
    /// a placeholder that is never consulted (extractors short-circuit on
    /// `enabled == false`).
    pub fn disabled() -> Self {
        Self {
            decoding_key: DecodingKey::from_secret(&[]),
            validation: Validation::new(Algorithm::HS256),
            enabled: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthConfigError {
    #[error("JWT_SECRET env var is required")]
    MissingSecret,
}
