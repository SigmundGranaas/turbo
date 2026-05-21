namespace Turboapi.Auth.Presentation.Cookies
{
    public class CookieSettings
    {
        public string? Domain { get; set; }
        public string SameSite { get; set; } = "Lax";
        public bool Secure { get; set; } = true;
        public int ExpiryDays { get; set; } = 7;
        public string Path { get; set; } = "/";
    }
}