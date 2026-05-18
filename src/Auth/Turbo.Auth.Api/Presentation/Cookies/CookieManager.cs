using Microsoft.Extensions.Options;

namespace Turboapi.Auth.Presentation.Cookies
{
    public class CookieManager : ICookieManager
    {
        private readonly IHttpContextAccessor _httpContextAccessor;
        private readonly CookieSettings _settings;
        public const string AccessTokenCookieName = "TurboAuth.AccessToken";
        public const string RefreshTokenCookieName = "TurboAuth.RefreshToken";

        private HttpContext HttpContext => _httpContextAccessor.HttpContext ??
            throw new InvalidOperationException("HttpContext is not available.");

        public CookieManager(IHttpContextAccessor httpContextAccessor, IOptions<CookieSettings> settings)
        {
            _httpContextAccessor = httpContextAccessor;
            _settings = settings.Value;
        }

        public void SetAuthCookies(string accessToken, string refreshToken, int accessTokenExpiryMinutes)
        {
            var sameSiteMode = Enum.Parse<SameSiteMode>(_settings.SameSite, true);

            // Create separate options for each cookie
            var accessTokenOptions = new CookieOptions
            {
                HttpOnly = true,
                Secure = _settings.Secure,
                SameSite = sameSiteMode,
                Path = _settings.Path,
                Domain = _settings.Domain,
                Expires = DateTime.UtcNow.AddMinutes(accessTokenExpiryMinutes)
            };

            var refreshTokenOptions = new CookieOptions
            {
                HttpOnly = true,
                Secure = _settings.Secure,
                SameSite = sameSiteMode,
                Path = _settings.Path,
                Domain = _settings.Domain,
                Expires = DateTime.UtcNow.AddDays(_settings.ExpiryDays)
            };

            HttpContext.Response.Cookies.Append(AccessTokenCookieName, accessToken, accessTokenOptions);
            HttpContext.Response.Cookies.Append(RefreshTokenCookieName, refreshToken, refreshTokenOptions);
        }

        public void ClearAuthCookies()
        {
            var baseOptions = new CookieOptions
            {
                HttpOnly = true,
                Secure = _settings.Secure,
                SameSite = Enum.Parse<SameSiteMode>(_settings.SameSite, true),
                Path = _settings.Path,
                Domain = _settings.Domain
            };

            HttpContext.Response.Cookies.Delete(AccessTokenCookieName, baseOptions);
            HttpContext.Response.Cookies.Delete(RefreshTokenCookieName, baseOptions);
        }

        public string? GetAccessToken() => HttpContext.Request.Cookies[AccessTokenCookieName];
        public string? GetRefreshToken() => HttpContext.Request.Cookies[RefreshTokenCookieName];
    }
}