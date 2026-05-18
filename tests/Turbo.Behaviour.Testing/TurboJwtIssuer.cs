using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Text;
using Microsoft.IdentityModel.Tokens;

namespace Turbo.Behaviour.Testing;

/// <summary>
/// Issues JWTs that match the configuration the Auth module's
/// AddTurboSharedAuthentication validates against (signing key from
/// appsettings.Test.json, "turbo-auth" issuer, "turbo-client"
/// audience). Used by fixtures that need to authenticate a request
/// against Activity or Geo without going through the Auth host's
/// /register flow.
/// </summary>
public sealed class TurboJwtIssuer
{
    private readonly string _signingKey;

    public TurboJwtIssuer(string signingKey) => _signingKey = signingKey;

    public string Issue(Guid userId, TimeSpan? lifetime = null)
    {
        var handler = new JwtSecurityTokenHandler();
        var key = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(_signingKey));
        var descriptor = new SecurityTokenDescriptor
        {
            Subject = new ClaimsIdentity(new[]
            {
                new Claim(ClaimTypes.NameIdentifier, userId.ToString()),
            }),
            Expires = DateTime.UtcNow + (lifetime ?? TimeSpan.FromHours(1)),
            Issuer = "turbo-auth",
            Audience = "turbo-client",
            SigningCredentials = new SigningCredentials(key, SecurityAlgorithms.HmacSha256Signature),
        };
        return handler.WriteToken(handler.CreateToken(descriptor));
    }
}
