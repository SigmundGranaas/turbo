using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Text;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.IdentityModel.Tokens;
using TurboAuthentication.Extensions;
using Xunit;

namespace TurboAuthentication.Tests.Handlers
{
    public class CookieJwtHandlerTests
    {
        private readonly string _validKey = "ThisIsAValidSecretKeyWithAtLeast32Chars";
        private readonly string _validIssuer = "test-issuer";
        private readonly string _validAudience = "test-audience";
        private readonly string _cookieName = "TurboAuth.AccessToken";

        private TestServer CreateTestServer(Dictionary<string, string> configValues = null)
        {
            var config = configValues ?? new Dictionary<string, string>
            {
                ["Jwt:Key"] = _validKey,
                ["Jwt:Issuer"] = _validIssuer,
                ["Jwt:Audience"] = _validAudience,
                ["Cookie:Name"] = _cookieName
            };

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(config)
                .Build();

            var builder = new WebHostBuilder()
                .ConfigureServices(services =>
                {
                    services.AddTurboAuth(configuration);
                    services.AddRouting();
                })
                .Configure(app =>
                {
                   
                    app.UseRouting();
                    
                    app.UseAuthentication();
                    
                    app.UseEndpoints(endpoints =>
                    {
                        endpoints.MapGet("/public", async context =>
                        {
                            await context.Response.WriteAsync("Public");
                        });

                        endpoints.MapGet("/protected", async context =>
                        {
                            if (!context.User.Identity.IsAuthenticated)
                            {
                                context.Response.StatusCode = 401;
                                return;
                            }
                            
                            var userId = context.User.FindFirstValue(ClaimTypes.NameIdentifier);
                            await context.Response.WriteAsync($"Protected: {userId}");
                        });
                    });
                    
                    

                });

            return new TestServer(builder);
        }

        private string GenerateToken(
            string userId = "test-user",
            string name = "Test User",
            string[] roles = null,
            DateTime? expiration = null,
            string key = null,
            string issuer = null,
            string audience = null)
        {
            var securityKey = new SymmetricSecurityKey(
                Encoding.UTF8.GetBytes(key ?? _validKey));
            var credentials = new SigningCredentials(
                securityKey, SecurityAlgorithms.HmacSha256);
            
            var claims = new List<Claim>
            {
                new Claim(ClaimTypes.NameIdentifier, userId),
                new Claim(ClaimTypes.Name, name)
            };
            
            if (roles != null)
            {
                foreach (var role in roles)
                {
                    claims.Add(new Claim(ClaimTypes.Role, role));
                }
            }
            
            var tokenExpiration = expiration ?? DateTime.UtcNow.AddMinutes(30);
            
            var token = new JwtSecurityToken(
                issuer ?? _validIssuer,
                audience ?? _validAudience,
                claims,
                expires: tokenExpiration,
                signingCredentials: credentials);
            
            return new JwtSecurityTokenHandler().WriteToken(token);
        }

        [Fact]
        public async Task PublicEndpoint_NoToken_ReturnsSuccess()
        {
            // Arrange
            var server = CreateTestServer();
            var client = server.CreateClient();
            
            // Act
            var response = await client.GetAsync("/public");
            
            // Assert
            response.EnsureSuccessStatusCode();
            var content = await response.Content.ReadAsStringAsync();
            Assert.Equal("Public", content);
        }
        
        [Fact]
        public async Task ProtectedEndpoint_NoToken_ReturnsUnauthorized()
        {
            // Arrange
            var server = CreateTestServer();
            var client = server.CreateClient();
            
            // Act
            var response = await client.GetAsync("/protected");
            
            // Assert
            Assert.Equal(System.Net.HttpStatusCode.Unauthorized, response.StatusCode);
        }
        
        [Fact]
        public async Task ProtectedEndpoint_ValidBearerToken_ReturnsSuccess()
        {
            // Arrange
            var server = CreateTestServer();
            var client = server.CreateClient();
            var token = GenerateToken(userId: "user123");
            client.DefaultRequestHeaders.Authorization = 
                new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", token);
            
            // Act
            var response = await client.GetAsync("/protected");
            
            // Assert
            response.EnsureSuccessStatusCode();
            var content = await response.Content.ReadAsStringAsync();
            Assert.Equal("Protected: user123", content);
        }
        
        [Fact]
        public async Task ProtectedEndpoint_ValidCookieToken_ReturnsSuccess()
        {
            // Arrange
            var server = CreateTestServer();
            var client = server.CreateClient();
            var token = GenerateToken(userId: "cookie-user");
            
            // Create request with cookie
            var request = new HttpRequestMessage(HttpMethod.Get, "/protected");
            request.Headers.Add("Cookie", $"{_cookieName}={token}");
            
            // Act
            var response = await client.SendAsync(request);
            
            // Assert
            response.EnsureSuccessStatusCode();
            var content = await response.Content.ReadAsStringAsync();
            Assert.Equal("Protected: cookie-user", content);
        }
        
        [Fact]
        public async Task ProtectedEndpoint_ExpiredToken_ReturnsUnauthorized()
        {
            // Arrange
            var server = CreateTestServer();
            var client = server.CreateClient();
            var expiredToken = GenerateToken(
                userId: "user123", 
                expiration: DateTime.UtcNow.AddMinutes(-5)); // Expired token
            client.DefaultRequestHeaders.Authorization = 
                new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", expiredToken);
            
            // Act
            var response = await client.GetAsync("/protected");
            
            // Assert
            Assert.Equal(System.Net.HttpStatusCode.Unauthorized, response.StatusCode);
        }
        
        [Fact]
        public async Task ProtectedEndpoint_InvalidIssuer_ReturnsUnauthorized()
        {
            // Arrange
            var server = CreateTestServer();
            var client = server.CreateClient();
            var invalidToken = GenerateToken(
                userId: "user123", 
                issuer: "invalid-issuer");
            client.DefaultRequestHeaders.Authorization = 
                new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", invalidToken);
            
            // Act
            var response = await client.GetAsync("/protected");
            
            // Assert
            Assert.Equal(System.Net.HttpStatusCode.Unauthorized, response.StatusCode);
        }
        
        [Fact]
        public async Task TokenPrecedence_BothTokensPresent_UsesBearerToken()
        {
            // Arrange
            var server = CreateTestServer();
            var client = server.CreateClient();
            
            // Create bearer token for user1
            var bearerToken = GenerateToken(userId: "bearer-user");
            client.DefaultRequestHeaders.Authorization = 
                new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", bearerToken);
            
            // Create cookie token for user2
            var cookieToken = GenerateToken(userId: "cookie-user");
            
            // Create request with both authentication methods
            var request = new HttpRequestMessage(HttpMethod.Get, "/protected");
            request.Headers.Add("Cookie", $"{_cookieName}={cookieToken}");
            
            // Act
            var response = await client.SendAsync(request);
            
            // Assert
            response.EnsureSuccessStatusCode();
            var content = await response.Content.ReadAsStringAsync();
            Assert.Equal("Protected: bearer-user", content); // Bearer token should take precedence
        }
        
        [Fact]
        public async Task DisabledValidations_AllowsInvalidValues()
        {
            // Arrange - Set up config with disabled validations
            var config = new Dictionary<string, string>
            {
                ["Jwt:Key"] = _validKey,
                ["Jwt:ValidateIssuer"] = "false",
                ["Jwt:ValidateAudience"] = "false",
            };
            
            var server = CreateTestServer(config);
            var client = server.CreateClient();
            
            // Create token with invalid issuer and audience
            var token = GenerateToken(
                userId: "user123",
                issuer: "wrong-issuer", 
                audience: "wrong-audience");
                
            client.DefaultRequestHeaders.Authorization = 
                new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", token);
            
            // Act
            var response = await client.GetAsync("/protected");
            
            // Assert - Should be successful despite wrong issuer/audience
            response.EnsureSuccessStatusCode();
            var content = await response.Content.ReadAsStringAsync();
            Assert.Equal("Protected: user123", content);
        }
        
        [Fact]
        public async Task CustomCookieName_UsesCorrectCookieName()
        {
            // Arrange - Set up config with custom cookie name
            var customCookieName = "CustomAuthCookie";
            var config = new Dictionary<string, string>
            {
                ["Jwt:Key"] = _validKey,
                ["Jwt:Issuer"] = _validIssuer,
                ["Jwt:Audience"] = _validAudience,
                ["Cookie:Name"] = customCookieName
            };
            
            var server = CreateTestServer(config);
            var client = server.CreateClient();
            
            var token = GenerateToken(userId: "cookie-user");
            
            // Create request with cookie using custom name
            var request = new HttpRequestMessage(HttpMethod.Get, "/protected");
            request.Headers.Add("Cookie", $"{customCookieName}={token}");
            
            // Act
            var response = await client.SendAsync(request);
            
            // Assert
            response.EnsureSuccessStatusCode();
            var content = await response.Content.ReadAsStringAsync();
            Assert.Equal("Protected: cookie-user", content);
        }
    }
}