using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Options;
using TurboAuthentication.configuration;
using TurboAuthentication.Extensions;
using Xunit;

namespace TurboAuthentication.Tests.Configuration
{
    public class JwtConfigTests
    {
        [Fact]
        public void BindFromConfiguration_SetsCorrectValues()
        {
            // Arrange
            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string>
                {
                    ["Jwt:Key"] = "MyTestSecretKey1234567890",
                    ["Jwt:Issuer"] = "test-issuer",
                    ["Jwt:Audience"] = "test-audience",
                    ["Jwt:TokenExpirationMinutes"] = "30",
                    ["Jwt:RefreshTokenExpirationDays"] = "14",
                    ["Jwt:ValidateIssuer"] = "true",
                    ["Jwt:ValidateAudience"] = "true",
                    ["Jwt:ValidateLifetime"] = "true",
                    ["Jwt:ValidateIssuerSigningKey"] = "true"
                })
                .Build();
            
            // Act
            var services = new ServiceCollection();
            services.AddTurboAuth(configuration);
            var serviceProvider = services.BuildServiceProvider();
            var jwtConfig = serviceProvider.GetRequiredService<IOptions<JwtConfig>>().Value;
            
            // Assert
            Assert.Equal("MyTestSecretKey1234567890", jwtConfig.Key);
            Assert.Equal("test-issuer", jwtConfig.Issuer);
            Assert.Equal("test-audience", jwtConfig.Audience);
            Assert.Equal(30, jwtConfig.TokenExpirationMinutes);
            Assert.Equal(14, jwtConfig.RefreshTokenExpirationDays);
            Assert.True(jwtConfig.ValidateIssuer);
            Assert.True(jwtConfig.ValidateAudience);
            Assert.True(jwtConfig.ValidateLifetime);
            Assert.True(jwtConfig.ValidateIssuerSigningKey);
            Assert.Equal(TimeSpan.Zero, jwtConfig.ClockSkew);
        }
        
        [Fact]
        public void BindFromConfiguration_MissingValues_UsesDefaults()
        {
            // Arrange
            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string>
                {
                    ["Jwt:Key"] = "MyTestSecretKey1234567890"
                    // Missing other properties
                })
                .Build();
            
            // Act
            var services = new ServiceCollection();
            services.AddTurboAuth(configuration);
            var serviceProvider = services.BuildServiceProvider();
            var jwtConfig = serviceProvider.GetRequiredService<IOptions<JwtConfig>>().Value;
            
            // Assert
            Assert.Equal("MyTestSecretKey1234567890", jwtConfig.Key);
            Assert.Equal(string.Empty, jwtConfig.Issuer);
            Assert.Equal(string.Empty, jwtConfig.Audience);
            Assert.Equal(60, jwtConfig.TokenExpirationMinutes); // Default
            Assert.Equal(7, jwtConfig.RefreshTokenExpirationDays); // Default
            Assert.True(jwtConfig.ValidateIssuer); // Default
            Assert.True(jwtConfig.ValidateAudience); // Default
            Assert.True(jwtConfig.ValidateLifetime); // Default
            Assert.True(jwtConfig.ValidateIssuerSigningKey); // Default
            Assert.Equal(TimeSpan.Zero, jwtConfig.ClockSkew); // Default
        }
        
        [Fact]
        public void EnvironmentVariables_OverrideConfiguration()
        {
            try
            {
                // Arrange - Set environment variables
                Environment.SetEnvironmentVariable("JWT_KEY", "EnvVarSecretKey");
                Environment.SetEnvironmentVariable("JWT_ISSUER", "env-issuer");
                Environment.SetEnvironmentVariable("JWT_AUDIENCE", "env-audience");
                Environment.SetEnvironmentVariable("JWT_EXPIRATION_MINUTES", "45");
                Environment.SetEnvironmentVariable("JWT_REFRESH_EXPIRATION_DAYS", "21");
                
                var configuration = new ConfigurationBuilder()
                    .AddInMemoryCollection(new Dictionary<string, string>
                    {
                        ["Jwt:Key"] = "ConfigSecretKey",
                        ["Jwt:Issuer"] = "config-issuer",
                        ["Jwt:Audience"] = "config-audience",
                        ["Jwt:TokenExpirationMinutes"] = "30",
                        ["Jwt:RefreshTokenExpirationDays"] = "14"
                    })
                    .Build();
                
                // Act
                var services = new ServiceCollection();
                services.AddTurboAuth(configuration);
                var serviceProvider = services.BuildServiceProvider();
                var jwtConfig = serviceProvider.GetRequiredService<IOptions<JwtConfig>>().Value;
                
                // Assert
                Assert.Equal("EnvVarSecretKey", jwtConfig.Key);
                Assert.Equal("env-issuer", jwtConfig.Issuer);
                Assert.Equal("env-audience", jwtConfig.Audience);
                Assert.Equal(45, jwtConfig.TokenExpirationMinutes);
                Assert.Equal(21, jwtConfig.RefreshTokenExpirationDays);
            }
            finally
            {
                // Cleanup
                Environment.SetEnvironmentVariable("JWT_KEY", null);
                Environment.SetEnvironmentVariable("JWT_ISSUER", null);
                Environment.SetEnvironmentVariable("JWT_AUDIENCE", null);
                Environment.SetEnvironmentVariable("JWT_EXPIRATION_MINUTES", null);
                Environment.SetEnvironmentVariable("JWT_REFRESH_EXPIRATION_DAYS", null);
            }
        }
    }

    public class CookieConfigTests
    {
        [Fact]
        public void BindFromConfiguration_SetsCorrectValues()
        {
            // Arrange
            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string>
                {
                    ["Cookie:Name"] = "CustomAuthCookie",
                    ["Cookie:HttpOnly"] = "true",
                    ["Cookie:SameSite"] = "Strict",
                    ["Cookie:SecurePolicy"] = "Always"
                })
                .Build();
            
            // Act
            var services = new ServiceCollection();
            services.AddTurboAuth(configuration);
            var serviceProvider = services.BuildServiceProvider();
            var cookieConfig = serviceProvider.GetRequiredService<IOptions<CookieConfig>>().Value;
            
            // Assert
            Assert.Equal("CustomAuthCookie", cookieConfig.Name);
            Assert.True(cookieConfig.HttpOnly);
            Assert.Equal(SameSiteMode.Strict, cookieConfig.SameSite);
            Assert.Equal(CookieSecurePolicy.Always, cookieConfig.SecurePolicy);
        }
        
        [Fact]
        public void BindFromConfiguration_MissingValues_UsesDefaults()
        {
            // Arrange
            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string>())
                .Build();
            
            // Act
            var services = new ServiceCollection();
            services.AddTurboAuth(configuration);
            var serviceProvider = services.BuildServiceProvider();
            var cookieConfig = serviceProvider.GetRequiredService<IOptions<CookieConfig>>().Value;
            
            // Assert
            Assert.Equal("TurboAuth.AccessToken", cookieConfig.Name); // Default
            Assert.True(cookieConfig.HttpOnly); // Default
            Assert.Equal(SameSiteMode.Lax, cookieConfig.SameSite); // Default
            Assert.Equal(CookieSecurePolicy.SameAsRequest, cookieConfig.SecurePolicy); // Default
        }
        
        [Fact]
        public void EnvironmentVariables_OverrideConfiguration()
        {
            try
            {
                // Arrange - Set environment variables
                Environment.SetEnvironmentVariable("COOKIE_NAME", "EnvVarCookie");
                Environment.SetEnvironmentVariable("COOKIE_HTTP_ONLY", "false");
                Environment.SetEnvironmentVariable("COOKIE_SAME_SITE", "None");
                Environment.SetEnvironmentVariable("COOKIE_SECURE", "Always");
                
                var configuration = new ConfigurationBuilder()
                    .AddInMemoryCollection(new Dictionary<string, string>
                    {
                        ["Cookie:Name"] = "ConfigCookie",
                        ["Cookie:HttpOnly"] = "true",
                        ["Cookie:SameSite"] = "Strict",
                        ["Cookie:SecurePolicy"] = "None"
                    })
                    .Build();
                
                // Act
                var services = new ServiceCollection();
                services.AddTurboAuth(configuration);
                var serviceProvider = services.BuildServiceProvider();
                var cookieConfig = serviceProvider.GetRequiredService<IOptions<CookieConfig>>().Value;
                
                // Assert
                Assert.Equal("EnvVarCookie", cookieConfig.Name);
                Assert.False(cookieConfig.HttpOnly);
                Assert.Equal(SameSiteMode.None, cookieConfig.SameSite);
                Assert.Equal(CookieSecurePolicy.Always, cookieConfig.SecurePolicy);
            }
            finally
            {
                // Cleanup
                Environment.SetEnvironmentVariable("COOKIE_NAME", null);
                Environment.SetEnvironmentVariable("COOKIE_HTTP_ONLY", null);
                Environment.SetEnvironmentVariable("COOKIE_SAME_SITE", null);
                Environment.SetEnvironmentVariable("COOKIE_SECURE", null);
            }
        }
    }
    
    public class AuthSchemeTests
    {
        [Fact]
        public async Task AddTurboAuth_RegistersAuthenticationScheme()
        {
            // Arrange
            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string>
                {
                    ["Jwt:Key"] = "MyTestSecretKey1234567890"
                })
                .Build();
            
            // Act
            var services = new ServiceCollection();
            services.AddTurboAuth(configuration);
            
            // Assert - Verify the authentication services were registered
            var authBuilder = services.BuildServiceProvider()
                .GetRequiredService<IAuthenticationSchemeProvider>();
            
            // Use async/await instead of GetAwaiter().GetResult()
            var scheme = await authBuilder.GetSchemeAsync("TurboAuth");
            Assert.NotNull(scheme);
            Assert.Equal("TurboAuth", scheme.Name);
        }
        
        [Fact]
        public async Task AddTurboAuth_ConfiguresAuthOptions()
        {
            // Arrange
            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string>
                {
                    ["Jwt:Key"] = "MyTestSecretKey1234567890",
                    ["Jwt:Issuer"] = "test-issuer",
                    ["Cookie:Name"] = "CustomAuthCookie"
                })
                .Build();
            
            // Act - Register services directly without WebApplication for simpler test
            var services = new ServiceCollection();
            services.AddTurboAuth(configuration);
            var serviceProvider = services.BuildServiceProvider();
            
            // Assert - Verify the auth configurations are registered properly
            Assert.NotNull(serviceProvider.GetService<IOptions<JwtConfig>>());
            Assert.NotNull(serviceProvider.GetService<IOptions<CookieConfig>>());
            
            // Verify the scheme is registered
            var authProvider = serviceProvider.GetRequiredService<IAuthenticationSchemeProvider>();
            var scheme = await authProvider.GetSchemeAsync("TurboAuth");
            Assert.NotNull(scheme);
        }
    }
}