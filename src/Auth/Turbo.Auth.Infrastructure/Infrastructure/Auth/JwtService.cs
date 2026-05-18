using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Security.Cryptography;
using System.Text;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Domain.Aggregates;

namespace Turboapi.Auth.Infrastructure.Auth
{
    public class JwtService : IAuthTokenService
    {
        private readonly JwtConfig _jwtConfig;
        private readonly ILogger<JwtService> _logger;

        public JwtService(
            IOptions<JwtConfig> jwtConfig,
            ILogger<JwtService> logger)
        {
            _jwtConfig = jwtConfig?.Value ?? throw new ArgumentNullException(nameof(jwtConfig));
            _logger = logger ?? throw new ArgumentNullException(nameof(logger));
        }

        public Task<NewTokenStrings> GenerateNewTokenStringsAsync(Account account)
        {
            if (account == null) throw new ArgumentNullException(nameof(account));
            _logger.LogInformation("Generating new token strings without persistence for account {AccountId}", account.Id);

            var accessTokenString = GenerateAccessTokenInternal(account);
            var refreshTokenString = GenerateRefreshTokenStringInternal();
            var refreshTokenExpiresAt = DateTime.UtcNow.AddDays(_jwtConfig.RefreshTokenExpirationDays);
            
            return Task.FromResult(new NewTokenStrings(accessTokenString, refreshTokenString, refreshTokenExpiresAt));
        }

        public Task<ClaimsPrincipal?> ValidateAccessTokenAsync(string token)
        {
            if (string.IsNullOrWhiteSpace(token))
                return Task.FromResult<ClaimsPrincipal?>(null);

            var tokenHandler = new JwtSecurityTokenHandler();
            var key = Encoding.UTF8.GetBytes(_jwtConfig.Key);
            try
            {
                var principal = tokenHandler.ValidateToken(token, new TokenValidationParameters
                {
                    ValidateIssuerSigningKey = true,
                    IssuerSigningKey = new SymmetricSecurityKey(key),
                    ValidateIssuer = true,
                    ValidIssuer = _jwtConfig.Issuer,
                    ValidateAudience = true,
                    ValidAudience = _jwtConfig.Audience,
                    ValidateLifetime = true,
                    ClockSkew = TimeSpan.Zero
                }, out SecurityToken validatedToken);

                if (validatedToken is not JwtSecurityToken jwtSecurityToken ||
                    !jwtSecurityToken.Header.Alg.Equals(SecurityAlgorithms.HmacSha256, StringComparison.InvariantCultureIgnoreCase))
                {
                    _logger.LogWarning("Access token validation failed: Invalid algorithm or token type.");
                    return Task.FromResult<ClaimsPrincipal?>(null);
                }
                
                return Task.FromResult<ClaimsPrincipal?>(principal);
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Access token validation failed: {ErrorMessage}", ex.Message);
                return Task.FromResult<ClaimsPrincipal?>(null);
            }
        }
        
        private string GenerateAccessTokenInternal(Account account)
        {
             var claims = new List<Claim>
            {
                new(JwtRegisteredClaimNames.Sub, account.Id.ToString()),
                new(JwtRegisteredClaimNames.Email, account.Email),
                new(JwtRegisteredClaimNames.Jti, Guid.NewGuid().ToString()),
                new(JwtRegisteredClaimNames.Iat, DateTimeOffset.UtcNow.ToUnixTimeSeconds().ToString(), ClaimValueTypes.Integer64)
            };
            foreach (var role in account.Roles)
            {
                claims.Add(new Claim(ClaimTypes.Role, role.Name));
            }

            var key = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(_jwtConfig.Key));
            var creds = new SigningCredentials(key, SecurityAlgorithms.HmacSha256);
            var expires = DateTime.UtcNow.AddMinutes(_jwtConfig.TokenExpirationMinutes);

            var accessTokenDescriptor = new JwtSecurityToken(
                issuer: _jwtConfig.Issuer,
                audience: _jwtConfig.Audience,
                claims: claims,
                notBefore: DateTime.UtcNow,
                expires: expires,
                signingCredentials: creds
            );
            return new JwtSecurityTokenHandler().WriteToken(accessTokenDescriptor);
        }
        
        private string GenerateRefreshTokenStringInternal()
        {
            var randomNumber = new byte[64];
            using (var rng = RandomNumberGenerator.Create())
            {
                rng.GetBytes(randomNumber);
            }
            return Convert.ToBase64String(randomNumber);
        }
    }
}