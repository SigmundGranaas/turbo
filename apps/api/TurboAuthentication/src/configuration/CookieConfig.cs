using Microsoft.AspNetCore.Http;

namespace TurboAuthentication.configuration;

public class CookieConfig
{
    public string Name { get; set; } = "TurboAuth.AccessToken";
    public bool HttpOnly { get; set; } = true;
    public SameSiteMode SameSite { get; set; } = SameSiteMode.Lax;
    public CookieSecurePolicy SecurePolicy { get; set; } = CookieSecurePolicy.SameAsRequest;
}