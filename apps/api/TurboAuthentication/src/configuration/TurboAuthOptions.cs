namespace TurboAuthentication.configuration;

public class TurboAuthOptions
{
    public JwtConfig JwtConfig { get; set; } = new();
    public CookieConfig CookieConfig { get; set; } = new();
    public bool UseCustomHandler { get; set; } = true;
}