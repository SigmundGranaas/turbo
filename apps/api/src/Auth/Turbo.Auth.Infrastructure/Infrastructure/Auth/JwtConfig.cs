namespace Turboapi.Auth.Infrastructure.Auth
{
    public class JwtConfig
    {
        public string Key { get; set; } = "default-super-secret-key-needs-to-be-long-enough-for-hs256";
        public string Issuer { get; set; } = "default-issuer";
        public string Audience { get; set; } = "default-audience";
        public int TokenExpirationMinutes { get; set; } = 15;
        public int RefreshTokenExpirationDays { get; set; } = 7;
    }
}