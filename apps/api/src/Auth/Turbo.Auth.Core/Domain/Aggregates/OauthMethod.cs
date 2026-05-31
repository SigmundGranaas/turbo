using Turboapi.Auth.Domain.Exceptions;

namespace Turboapi.Auth.Domain.Aggregates
{
    public class OAuthAuthMethod : AuthenticationMethod
    {
        public string ExternalUserId { get; private set; } = null!;
        public string? AccessToken { get; private set; } 
        public string? OAuthRefreshToken { get; private set; } 
        public DateTime? TokenExpiry { get; private set; }

        // Private constructor for EF Core - NO validation, NO base constructor call with parameters
        private OAuthAuthMethod() : base()
        {
            // EF Core will populate properties
        }
        
        public OAuthAuthMethod(Guid id, Guid accountId, string providerName, string externalUserId, 
            string? accessToken = null, string? oauthRefreshToken = null, DateTime? tokenExpiry = null)
            : base(id, accountId, providerName)
        {
            if (string.IsNullOrWhiteSpace(externalUserId))
                throw new DomainException("External User ID cannot be empty for OAuthAuthMethod.");

            ExternalUserId = externalUserId;
            AccessToken = accessToken;
            OAuthRefreshToken = oauthRefreshToken;
            TokenExpiry = tokenExpiry?.ToUniversalTime(); // Ensure UTC if provided
        }

        public void UpdateTokens(string? newAccessToken, string? newOAuthRefreshToken, DateTime? newTokenExpiry)
        {
            AccessToken = newAccessToken;
            OAuthRefreshToken = newOAuthRefreshToken;
            TokenExpiry = newTokenExpiry?.ToUniversalTime(); // Ensure UTC if provided
            UpdateLastUsed(); 
        }
    }
}