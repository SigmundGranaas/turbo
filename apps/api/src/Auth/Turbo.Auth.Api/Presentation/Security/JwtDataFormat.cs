using System.IdentityModel.Tokens.Jwt;
using System.Text;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Authentication.Cookies;
using Microsoft.IdentityModel.Tokens;
using Turboapi.Auth.Infrastructure.Auth;

namespace Turboapi.Auth.Presentation.Security
{
    /// <summary>
    /// A custom data format implementation that uses JWT validation logic
    /// to protect and unprotect the authentication ticket stored in a cookie.
    /// This allows the cookie authentication middleware to understand and validate
    /// our JWT access tokens.
    /// </summary>
    public class JwtDataFormat : ISecureDataFormat<AuthenticationTicket>
    {
        private readonly TokenValidationParameters _validationParameters;

        public JwtDataFormat(JwtConfig jwtConfig)
        {
            _validationParameters = new TokenValidationParameters
            {
                ValidateIssuerSigningKey = true,
                IssuerSigningKey = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(jwtConfig.Key)),
                ValidateIssuer = true,
                ValidIssuer = jwtConfig.Issuer,
                ValidateAudience = true,
                ValidAudience = jwtConfig.Audience,
                ValidateLifetime = true,
                ClockSkew = TimeSpan.Zero
            };
        }

        public string Protect(AuthenticationTicket data) => throw new NotImplementedException();
        public string Protect(AuthenticationTicket data, string? purpose) => throw new NotImplementedException();

        public AuthenticationTicket? Unprotect(string? protectedText) => Unprotect(protectedText, null);

        public AuthenticationTicket? Unprotect(string? protectedText, string? purpose)
        {
            if (string.IsNullOrWhiteSpace(protectedText))
            {
                return null;
            }

            try
            {
                var handler = new JwtSecurityTokenHandler();
                var principal = handler.ValidateToken(protectedText, _validationParameters, out var validatedToken);

                // The validated token's identity is wrapped in an AuthenticationTicket.
                // The scheme name must match the one used by the cookie middleware.
                return new AuthenticationTicket(principal, CookieAuthenticationDefaults.AuthenticationScheme);
            }
            catch (Exception)
            {
                // Token validation failed (expired, invalid signature, etc.)
                return null;
            }
        }
    }
}