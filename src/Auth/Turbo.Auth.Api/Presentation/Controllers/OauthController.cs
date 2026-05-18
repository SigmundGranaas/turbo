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
        public IActionResult GetAuthorizationUrl(string provider, [FromQuery] string? state)
        {
            var adapter = _providerAdapters.FirstOrDefault(p => p.ProviderName.Equals(provider, StringComparison.OrdinalIgnoreCase));
            if (adapter == null)
            {
                return NotFound($"Provider '{provider}' not supported.");
            }
            var url = adapter.GetAuthorizationUrl(state);
            return Ok(new { AuthorizationUrl = url });
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
                CreateCallbackRedirectUri(request.Provider),
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
                    return Redirect($"{frontendUrl}/login/success");
                },
                failure => HandleResult(result) // HandleResult maps the error to a status code.
            );
        }
        
        private string CreateCallbackRedirectUri(string provider)
        {
            // This might need to be more dynamic if you add more providers.
            var googleSettings = HttpContext.RequestServices.GetRequiredService<IOptions<GoogleAuthSettings>>().Value;
            return googleSettings.RedirectUri;
        }
    }
}