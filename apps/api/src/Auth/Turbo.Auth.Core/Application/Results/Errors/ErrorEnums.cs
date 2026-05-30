namespace Turboapi.Auth.Application.Results.Errors
{
    public enum GenericError
    {
        None = 0,
        Unknown,
        NotFound,
        InvalidInput
    }

    public enum RefreshTokenError
    {
        None = 0,
        InvalidToken = 1,
        Expired = 2,
        Revoked = 3,
        AccountNotFound = 4,
        StorageFailure = 5,
        TokenGenerationFailed // Added based on IAuthTokenService
    }

    public enum OAuthError // Errors from IOAuthProviderAdapter
    {
        None = 0,
        ConfigurationError,
        NetworkError,
        TokenExchangeFailed,
        UserInfoFailed,
        InvalidCode,
        ProviderDeniedAccess,
        EmailNotVerified, // This might be more of a policy, but adapter can detect
        InvalidState,
        MissingRequiredToken,
        TokenValidationError // e.g. ID token validation failed
    }

    public enum RegistrationError
    {
        None = 0,
        EmailAlreadyExists,
        WeakPassword,
        AccountCreationFailed,
        AuthMethodCreationFailed,
        TokenGenerationFailed,
        EventPublishFailed,
        InvalidInput
    }

    public enum LoginError
    {
        None = 0,
        InvalidCredentials,         
        AccountNotFound,            
        PasswordMethodNotFound,     
        AccountLocked, // Placeholder, not fully implemented yet
        AuthMethodVerificationFailed,
        TokenGenerationFailed,
        EventPublishFailed,
        InvalidInput
    }

    public enum OAuthLoginError // Errors from AuthenticateWithOAuthCommandHandler
    {
        None = 0,
        ProviderError,          // Generic error from the OAuth provider interaction (adapter)
        AccountLinkageFailed,   // Failed to link OAuth to an existing account
        AccountCreationFailed,  // Failed to create a new account for the OAuth user
        TokenGenerationFailed,  // Failed to generate internal system tokens
        EmailNotVerified,       // Policy: Email from provider was not verified
        EventPublishFailed,     // Failed to publish domain events post-login/registration
        UnsupportedProvider,    // The requested OAuth provider is not configured/supported
        InvalidState // If state validation fails (though not explicitly in current command)
    }
    
    public enum SessionValidationError
    {
        None = 0,
        TokenInvalid,
        TokenExpired,
        UserNotFound,
        AccountInactive
    }

    public enum ChangePasswordError
    {
        None = 0,
        AccountNotFound,        // No account for the authenticated id
        OAuthOnlyAccount,       // Account has no password method (e.g. Google) -> cannot change a password it doesn't have
        InvalidCurrentPassword, // Supplied current password did not match
        WeakPassword,           // New password failed policy
        InvalidInput            // Malformed request (e.g. confirmation mismatch)
    }

    public enum UpdateProfileError
    {
        None = 0,
        AccountNotFound,
        InvalidInput // e.g. display name too long
    }
}