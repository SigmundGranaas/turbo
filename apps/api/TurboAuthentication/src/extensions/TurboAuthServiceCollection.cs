using System.Text;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.IdentityModel.Tokens;
using TurboAuth.Handlers;
using TurboAuthentication.configuration;

namespace TurboAuthentication.Extensions;

public static class TurboAuthServiceCollection
    {
       public static IServiceCollection AddTurboAuth(
        this IServiceCollection services, 
        IConfiguration configuration)
    {
        // Get configuration from flattened sections
        var jwtConfig = new JwtConfig();
        var cookieConfig = new CookieConfig();
        
        configuration.GetSection("Jwt").Bind(jwtConfig);
        configuration.GetSection("Cookie").Bind(cookieConfig);
        
        // Apply environment variables directly
        ApplyEnvironmentVariables(jwtConfig, cookieConfig);
        
        // Register options for DI
        services.Configure<JwtConfig>(opt => 
        {
            opt.Key = jwtConfig.Key;
            opt.Issuer = jwtConfig.Issuer;
            opt.Audience = jwtConfig.Audience;
            opt.TokenExpirationMinutes = jwtConfig.TokenExpirationMinutes;
            opt.RefreshTokenExpirationDays = jwtConfig.RefreshTokenExpirationDays;
            opt.ValidateIssuer = jwtConfig.ValidateIssuer;
            opt.ValidateAudience = jwtConfig.ValidateAudience;
            opt.ValidateLifetime = jwtConfig.ValidateLifetime;
            opt.ValidateIssuerSigningKey = jwtConfig.ValidateIssuerSigningKey;
            opt.ClockSkew = jwtConfig.ClockSkew;
        });
        
        services.Configure<CookieConfig>(opt => 
        {
            opt.Name = cookieConfig.Name;
            opt.HttpOnly = cookieConfig.HttpOnly;
            opt.SameSite = cookieConfig.SameSite;
            opt.SecurePolicy = cookieConfig.SecurePolicy;
        });
        
        // Configure custom authentication handler
        services.AddAuthentication("TurboAuth")
            .AddScheme<CookieJwtAuthenticationOptions, CookieJwtHandler>("TurboAuth", opt =>
            {
                // Configure cookie options
                opt.Cookie = new CookieBuilder
                {
                    Name = cookieConfig.Name,
                    HttpOnly = cookieConfig.HttpOnly,
                    SameSite = cookieConfig.SameSite,
                    SecurePolicy = cookieConfig.SecurePolicy
                };
                
                // Configure JWT validation
                opt.TokenValidationParameters = new TokenValidationParameters
                {
                    ValidateIssuerSigningKey = jwtConfig.ValidateIssuerSigningKey,
                    IssuerSigningKey = new SymmetricSecurityKey(
                        Encoding.UTF8.GetBytes(jwtConfig.Key)),
                    ValidateIssuer = jwtConfig.ValidateIssuer,
                    ValidateAudience = jwtConfig.ValidateAudience,
                    ValidIssuer = jwtConfig.Issuer,
                    ValidAudience = jwtConfig.Audience,
                    ClockSkew = jwtConfig.ClockSkew
                };
            });
        
        return services;
    }

    private static void ApplyEnvironmentVariables(JwtConfig jwtConfig, CookieConfig cookieConfig)
    {
        if (!string.IsNullOrEmpty(Environment.GetEnvironmentVariable("JWT_KEY")))
            jwtConfig.Key = Environment.GetEnvironmentVariable("JWT_KEY");
        
        if (!string.IsNullOrEmpty(Environment.GetEnvironmentVariable("JWT_ISSUER")))
            jwtConfig.Issuer = Environment.GetEnvironmentVariable("JWT_ISSUER");
        
        if (!string.IsNullOrEmpty(Environment.GetEnvironmentVariable("JWT_AUDIENCE")))
            jwtConfig.Audience = Environment.GetEnvironmentVariable("JWT_AUDIENCE");
        
        if (int.TryParse(Environment.GetEnvironmentVariable("JWT_EXPIRATION_MINUTES"), out var expMins))
            jwtConfig.TokenExpirationMinutes = expMins;
        
        if (int.TryParse(Environment.GetEnvironmentVariable("JWT_REFRESH_EXPIRATION_DAYS"), out var expDays))
            jwtConfig.RefreshTokenExpirationDays = expDays;
        
        // Cookie Config
        if (!string.IsNullOrEmpty(Environment.GetEnvironmentVariable("COOKIE_NAME")))
            cookieConfig.Name = Environment.GetEnvironmentVariable("COOKIE_NAME");
        
        if (bool.TryParse(Environment.GetEnvironmentVariable("COOKIE_HTTP_ONLY"), out var httpOnly))
            cookieConfig.HttpOnly = httpOnly;
        
        if (Enum.TryParse<SameSiteMode>(Environment.GetEnvironmentVariable("COOKIE_SAME_SITE"), out var sameSite))
            cookieConfig.SameSite = sameSite;
        
        if (Enum.TryParse<CookieSecurePolicy>(Environment.GetEnvironmentVariable("COOKIE_SECURE"), out var securePolicy))
            cookieConfig.SecurePolicy = securePolicy;
    }
}
