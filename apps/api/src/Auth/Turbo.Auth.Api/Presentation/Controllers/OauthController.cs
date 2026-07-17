using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Contracts.V1.OAuth;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Application.UseCases.Commands.AuthenticateWithOAuth;
using Turboapi.Auth.Infrastructure.Auth;
using Turboapi.Auth.Infrastructure.Auth.OAuthProviders;
using Turboapi.Auth.Presentation.Cookies;

namespace Turboapi.Auth.Presentation.Controllers
{
    [Route("api/auth/[controller]")]
    public class OAuthController : BaseApiController
    {
        private readonly ICommandHandler<AuthenticateWithOAuthCommand, Result<AuthTokenResponse, OAuthLoginError>> _authHandler;
        private readonly IEnumerable<IOAuthProviderAdapter> _providerAdapters;
        private readonly ICookieManager _cookieManager;
        private readonly JwtConfig _jwtConfig;
        private readonly IConfiguration _configuration;

        public OAuthController(
            ICommandHandler<AuthenticateWithOAuthCommand, Result<AuthTokenResponse, OAuthLoginError>> authHandler,
            IEnumerable<IOAuthProviderAdapter> providerAdapters,
            ICookieManager cookieManager,
            IOptions<JwtConfig> jwtConfig,
            IConfiguration configuration)
        {
            _authHandler = authHandler;
            _providerAdapters = providerAdapters;
            _cookieManager = cookieManager;
            _jwtConfig = jwtConfig.Value;
            _configuration = configuration;
        }

        [HttpGet("{provider}/url")]
        public IActionResult GetAuthorizationUrl(string provider, [FromQuery] string? state, [FromQuery] bool mobile = false)
        {
            var adapter = _providerAdapters.FirstOrDefault(p => p.ProviderName.Equals(provider, StringComparison.OrdinalIgnoreCase));
            if (adapter == null)
            {
                return NotFound($"Provider '{provider}' not supported.");
            }
            // Native (mobile) clients pass ?mobile=true so Google returns the code to
            // the mobile-callback hop (which bounces it into the app), NOT the web
            // callback that redirects to the web frontend. Web clients omit it.
            var redirectOverride = mobile ? MobileRedirectUri(provider) : null;
            var url = adapter.GetAuthorizationUrl(state, redirectOverride);
            return Ok(new { AuthorizationUrl = url });
        }

        /// <summary>
        /// The MOBILE OAuth hop. Google redirects here (an https URL it will accept
        /// as a redirect_uri for a Web client) with the authorization code; we bounce
        /// it into the native app via its custom scheme (turbo://oauth?code=…). The
        /// app then POSTs the code to <c>mobile-signin</c>, which exchanges it against
        /// this SAME redirect_uri. No cookies, no token exchange happen here.
        /// </summary>
        [HttpGet("{provider}/mobile-callback")]
        public IActionResult MobileCallback(string provider, [FromQuery] string? code, [FromQuery] string? state, [FromQuery] string? error)
        {
            var googleSettings = HttpContext.RequestServices.GetRequiredService<IOptions<GoogleAuthSettings>>().Value;
            var scheme = string.IsNullOrWhiteSpace(googleSettings.MobileReturnScheme) ? "turbo://oauth" : googleSettings.MobileReturnScheme;

            var query = new List<string>();
            if (!string.IsNullOrEmpty(code)) query.Add($"code={Uri.EscapeDataString(code)}");
            if (!string.IsNullOrEmpty(state)) query.Add($"state={Uri.EscapeDataString(state)}");
            if (!string.IsNullOrEmpty(error)) query.Add($"error={Uri.EscapeDataString(error)}");
            var sep = scheme.Contains('?') ? "&" : "?";
            var target = query.Count > 0 ? $"{scheme}{sep}{string.Join("&", query)}" : scheme;
            return Redirect(target);
        }

        /// <summary>
        /// A dedicated endpoint for mobile/API clients to sign in with an OAuth provider's code.
        /// This endpoint always returns JSON and never redirects.
        /// </summary>
        [HttpPost("mobile-signin")]
        [Produces("application/json")]
        [ProducesResponseType(typeof(AuthTokenResponse), StatusCodes.Status200OK)]
        [ProducesResponseType(typeof(Application.Contracts.V1.Common.ErrorResponse), StatusCodes.Status400BadRequest)]
        [ProducesResponseType(typeof(Application.Contracts.V1.Common.ErrorResponse), StatusCodes.Status403Forbidden)]
        [ProducesResponseType(typeof(Application.Contracts.V1.Common.ErrorResponse), StatusCodes.Status404NotFound)]
        public async Task<IActionResult> MobileSignIn([FromBody] MobileSignInRequest request)
        {
            var command = new AuthenticateWithOAuthCommand(
                request.Provider,
                request.Code,
                // The code was obtained via the mobile-callback redirect_uri, so the
                // exchange MUST present that same value (Google enforces the match).
                MobileRedirectUri(request.Provider),
                request.State);

            var result = await _authHandler.Handle(command, HttpContext.RequestAborted);

            // This endpoint *only* returns JSON. It never sets cookies or redirects.
            return HandleResult(result);
        }

        /// <summary>
        /// This endpoint handles the callback from the OAuth provider for the web-based redirect flow.
        /// It sets session cookies and redirects the browser to the frontend.
        /// </summary>
        [HttpGet("{provider}/callback")]
        public async Task<IActionResult> Callback(string provider, [FromQuery] string code, [FromQuery] string? state)
        {
            var command = new AuthenticateWithOAuthCommand(
                provider,
                code,
                CreateCallbackRedirectUri(provider),
                state);

            var result = await _authHandler.Handle(command, HttpContext.RequestAborted);
            
            return result.Match<IActionResult>(
                success =>
                {
                    // For web flow, set the session cookies
                    _cookieManager.SetAuthCookies(success.AccessToken, success.RefreshToken, _jwtConfig.TokenExpirationMinutes);

                    // For web clients (browsers), redirect to the frontend success page.
                    var frontendUrl = _configuration.GetValue<string>("FrontendUrl") ?? "http://localhost:8080";

                    // The SPA can encode a return path in the OAuth `state`
                    // parameter (URL-encoded, relative to FrontendUrl). Honour
                    // it as long as it stays under the same origin — otherwise
                    // this becomes an open-redirect vector. Falls back to
                    // /login/success so legacy callers are unaffected.
                    var returnPath = ResolveSafeReturnPath(state);
                    return Redirect($"{frontendUrl}{returnPath}");
                },
                failure => HandleResult(result) // HandleResult maps the error to a status code.
            );
        }

        /// <summary>
        /// Pull a SPA-supplied return path out of the OAuth state, accept it
        /// only if it's a relative path (no scheme, no host), and default to
        /// /login/success otherwise.
        /// </summary>
        private static string ResolveSafeReturnPath(string? state)
        {
            const string fallback = "/login/success";
            if (string.IsNullOrWhiteSpace(state)) return fallback;

            string decoded;
            try
            {
                decoded = System.Net.WebUtility.UrlDecode(state);
            }
            catch
            {
                return fallback;
            }

            // Must start with '/', must NOT start with '//' (protocol-relative),
            // and must not contain a scheme. Cheap-and-correct check.
            if (string.IsNullOrEmpty(decoded)) return fallback;
            if (!decoded.StartsWith('/')) return fallback;
            if (decoded.StartsWith("//")) return fallback;
            if (decoded.Contains(':')) return fallback;
            if (decoded.Contains('\\')) return fallback;

            return decoded;
        }
        
        private string CreateCallbackRedirectUri(string provider)
        {
            // This might need to be more dynamic if you add more providers.
            var googleSettings = HttpContext.RequestServices.GetRequiredService<IOptions<GoogleAuthSettings>>().Value;
            return googleSettings.RedirectUri;
        }

        /// <summary>
        /// The redirect_uri used by BOTH the mobile authorization request and the
        /// mobile-signin code exchange (Google requires them to be identical). Uses
        /// the configured <see cref="GoogleAuthSettings.MobileRedirectUri"/>, else
        /// derives the mobile-callback URL off the current request host so a missing
        /// config still works in dev.
        /// </summary>
        private string MobileRedirectUri(string provider)
        {
            var googleSettings = HttpContext.RequestServices.GetRequiredService<IOptions<GoogleAuthSettings>>().Value;
            if (!string.IsNullOrWhiteSpace(googleSettings.MobileRedirectUri))
            {
                return googleSettings.MobileRedirectUri;
            }
            return $"{Request.Scheme}://{Request.Host}/api/auth/oauth/{provider.ToLowerInvariant()}/mobile-callback";
        }
    }
}