namespace Turboapi.Auth.Infrastructure.Auth.OAuthProviders
{
    public class GoogleAuthSettings
    {
        public string ClientId { get; set; } = string.Empty;
        public string ClientSecret { get; set; } = string.Empty;
        public string RedirectUri { get; set; } = string.Empty; // Default (web) Redirect URI

        /// <summary>
        /// Redirect URI for the MOBILE (native app) flow. Google returns the code
        /// here (an https URL registered in the Google console), and this endpoint
        /// bounces it into the app via its custom scheme — see OAuthController's
        /// mobile-callback. Both the authorization request AND the mobile-signin
        /// code exchange use this exact value (Google requires them to match). If
        /// left blank it is derived from the request host at
        /// <c>/api/auth/oauth/{provider}/mobile-callback</c>.
        /// </summary>
        public string MobileRedirectUri { get; set; } = string.Empty;

        /// <summary>
        /// Custom-scheme deep link the mobile-callback redirects to so the native
        /// app catches the code (the app claims this in its manifest).
        /// </summary>
        public string MobileReturnScheme { get; set; } = "turbo://oauth";

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