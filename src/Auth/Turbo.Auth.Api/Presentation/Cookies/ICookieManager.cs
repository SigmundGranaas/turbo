namespace Turboapi.Auth.Presentation.Cookies
{
    public interface ICookieManager
    {
        void SetAuthCookies(string accessToken, string refreshToken, int accessTokenExpiryMinutes);
        void ClearAuthCookies();
        string? GetAccessToken();
        string? GetRefreshToken();
    }
}