using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Text.Encodings.Web;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;

namespace TurboAuth.Handlers
{
    public class CookieJwtAuthenticationOptions : AuthenticationSchemeOptions
    {
        public CookieBuilder Cookie { get; set; }
        public TokenValidationParameters TokenValidationParameters { get; set; }
    }

    public class CookieJwtHandler : AuthenticationHandler<CookieJwtAuthenticationOptions>
    {
        private readonly ILogger<CookieJwtHandler> _logger;

        public CookieJwtHandler(
            IOptionsMonitor<CookieJwtAuthenticationOptions> options,
            ILoggerFactory logger,
            UrlEncoder encoder,
            ISystemClock clock) 
            : base(options, logger, encoder, clock)
        {
            _logger = logger.CreateLogger<CookieJwtHandler>();
        }

        protected override Task<AuthenticateResult> HandleAuthenticateAsync()
        {
            _logger.LogDebug("CookieJwtHandler: Authentication attempt");
            
            // First check for JWT Bearer token
            string authorization = Request.Headers["Authorization"];
            if (!string.IsNullOrEmpty(authorization) && authorization.StartsWith("Bearer "))
            {
                var token = authorization.Substring("Bearer ".Length).Trim();
                return ValidateTokenAsync(token);
            }
            
            // Then check for cookie
            if (Request.Cookies.TryGetValue(Options.Cookie.Name, out var cookieToken))
            {
                return ValidateTokenAsync(cookieToken);
            }
            
            return Task.FromResult(AuthenticateResult.NoResult());
        }
        
        private Task<AuthenticateResult> ValidateTokenAsync(string token)
        {
            try
            {
                var tokenHandler = new JwtSecurityTokenHandler();
                
                var principal = tokenHandler.ValidateToken(token, Options.TokenValidationParameters, out _);
                
                _logger.LogInformation("Token validated successfully - User ID: {UserId}", 
                    principal.FindFirstValue(ClaimTypes.NameIdentifier));
                
                var ticket = new AuthenticationTicket(principal, Scheme.Name);
                return Task.FromResult(AuthenticateResult.Success(ticket));
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "Token validation failed: {Message}", ex.Message);
                return Task.FromResult(AuthenticateResult.Fail(ex));
            }
        }
        
        // Handle challenges (401 responses)
        protected override async Task HandleChallengeAsync(AuthenticationProperties properties)
        {
            Response.StatusCode = 401;
            await Response.CompleteAsync();
        }
        
        // Handle forbid (403 responses)
        protected override async Task HandleForbiddenAsync(AuthenticationProperties properties)
        {
            Response.StatusCode = 403;
            await Response.CompleteAsync();
        }
    }
}