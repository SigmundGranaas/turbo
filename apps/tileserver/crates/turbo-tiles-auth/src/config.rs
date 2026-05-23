use jsonwebtoken::{Algorithm, DecodingKey, Validation};

#[derive(Clone)]
pub struct AuthConfig {
    pub decoding_key: DecodingKey,
    pub validation: Validation,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("validation.algorithms", &self.validation.algorithms)
            .finish()
    }
}

impl AuthConfig {
    /// Build from env: `JWT_SECRET` (shared with .NET `JwtConfig.Key`),
    /// optional `JWT_ISSUER` and `JWT_AUDIENCE`.
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
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthConfigError {
    #[error("JWT_SECRET env var is required")]
    MissingSecret,
}
