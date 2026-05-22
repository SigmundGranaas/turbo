namespace Turboapi.Auth.Infrastructure.Auth.OAuthProviders
{
    public class GoogleAuthSettings
    {
        public string ClientId { get; set; } = string.Empty;
        public string ClientSecret { get; set; } = string.Empty;
        public string RedirectUri { get; set; } = string.Empty; // Default Redirect URI

        public string AuthorizationEndpoint { get; set; } = "https://accounts.google.com/o/oauth2/v2/auth";
        public string TokenEndpoint { get; set; } = "https://oauth2.googleapis.com/token";
        public string UserInfoEndpoint { get; set; } = "https://openidconnect.googleapis.com/v1/userinfo";

        /// <summary>
        /// Default scopes to request if not overridden.
        /// Standard OIDC scopes: "openid", "profile", "email".
        /// </summary>
        public string[] DefaultScopes { get; set; } = { "openid", "email", "profile" };
    }
}