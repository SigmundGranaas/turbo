// src/Application/Interfaces/IOAuthProviderAdapter.cs
using Turboapi.Auth.Application.Contracts.V1.OAuth;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;

namespace Turboapi.Auth.Application.Interfaces
{
    /// <summary>
    /// Defines a contract for interacting with an external OAuth 2.0 provider.
    /// </summary>
    public interface IOAuthProviderAdapter
    {
        /// <summary>
        /// Gets the provider-specific name (e.g., "Google", "Facebook").
        /// </summary>
        string ProviderName { get; }

        /// <summary>
        /// Generates the URL to redirect the user to for authorization at the OAuth provider.
        /// </summary>
        /// <param name="state">An optional opaque value used to maintain state between the request and callback.</param>
        /// <param name="redirectUriOverride">Optional override for the redirect_uri (e.g. the mobile flow's redirect); null uses the configured default.</param>
        /// <param name="scopes">Optional scopes to request. If null or empty, default scopes from configuration will be used.</param>
        /// <returns>The authorization URL.</returns>
        string GetAuthorizationUrl(string? state = null, string? redirectUriOverride = null, params string[]? scopes);

        /// <summary>
        /// Exchanges an authorization code for OAuth tokens (access token, refresh token, ID token).
        /// </summary>
        /// <param name="code">The authorization code received from the provider.</param>
        /// <param name="redirectUriOverride">Optional override for the redirect_uri, if different from configured default.</param>
        /// <returns>A result containing the OAuth tokens or an OAuthError.</returns>
        Task<Result<OAuthProviderTokens, OAuthError>> ExchangeCodeForTokensAsync(string code, string? redirectUriOverride = null);

        /// <summary>
        /// Retrieves standardized user information from the OAuth provider using an access token.
        /// </summary>
        /// <param name="providerAccessToken">The access token obtained from the provider.</param>
        /// <returns>A result containing the standardized user info or an OAuthError.</returns>
        Task<Result<OAuthUserInfo, OAuthError>> GetUserInfoAsync(string providerAccessToken);

        /// <summary>
        /// (Optional) Refreshes an access token using a refresh token.
        /// Not all providers support this or expose it in the same way.
        /// </summary>
        /// <param name="refreshToken">The refresh token.</param>
        /// <returns>A result containing the new OAuth tokens (typically a new access token, potentially a new refresh token) or an OAuthError.</returns>
        Task<Result<OAuthProviderTokens, OAuthError>> RefreshAccessTokenAsync(string refreshToken);
    }
}