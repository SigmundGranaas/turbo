namespace Turboapi.Auth.Application.Contracts.V1.OAuth
{
    /// <summary>
    /// Represents tokens received from an OAuth provider after exchanging an authorization code.
    /// </summary>
    public record OAuthProviderTokens(
        string AccessToken,
        string? IdToken,
        string? RefreshToken,
        int? ExpiresInSeconds, // Lifetime of the access token in seconds
        string? TokenType,      // Typically "Bearer"
        string? Scope           // Scope granted by the provider
    );

    /// <summary>
    /// Standardized user information retrieved from an OAuth provider.
    /// </summary>
    public record OAuthUserInfo(
        string ExternalId,      // The unique identifier for the user at the OAuth provider
        string Email,
        bool IsEmailVerified,
        string? FirstName,
        string? LastName,
        string? FullName,
        string? PictureUrl
    );
}