using System;
using Turboapi.Auth.Domain.Exceptions;

namespace Turboapi.Auth.Domain.Aggregates
{
    public class RefreshToken
    {
        public Guid Id { get; private set; }
        public Guid AccountId { get; private set; } 
        public string Token { get; private set; } = null!;
        public DateTime ExpiresAt { get; private set; }
        public DateTime CreatedAt { get; private set; }
        public bool IsRevoked { get; private set; }
        public DateTime? RevokedAt { get; private set; }
        public string? RevokedReason { get; private set; }

        private RefreshToken() { }

        private RefreshToken(Guid accountId, string token, DateTime expiresAt, DateTime? createdDate, Guid? tokenId, bool skipValidation)
        {
            // *** FIX: Validate tokenId *before* assigning it ***
            var effectiveTokenId = tokenId ?? Guid.NewGuid();
            if (effectiveTokenId == Guid.Empty)
                throw new DomainException("RefreshToken ID cannot be empty.");
            
            if (accountId == Guid.Empty)
                throw new DomainException("Account ID for RefreshToken cannot be empty.");
            if (string.IsNullOrWhiteSpace(token))
                throw new DomainException("Token string for RefreshToken cannot be empty.");
            
            if (!skipValidation && expiresAt <= DateTime.UtcNow)
                throw new DomainException("RefreshToken expiration must be in the future.");

            Id = effectiveTokenId;
            AccountId = accountId;
            Token = token;
            ExpiresAt = expiresAt.ToUniversalTime(); 
            CreatedAt = createdDate ?? DateTime.UtcNow; 
            IsRevoked = false;
        }

        public static RefreshToken Create(Guid accountId, string token, DateTime expiresAt)
        {
            return new RefreshToken(accountId, token, expiresAt, null, null, false);
        }
        
        public static RefreshToken Create(Guid accountId, string token, DateTime expiresAt, DateTime createdDate)
        {
            var skipValidation = expiresAt <= DateTime.UtcNow;
            return new RefreshToken(accountId, token, expiresAt, createdDate, null, skipValidation);
        }
        
        public static RefreshToken Create(Guid tokenId, Guid accountId, string token, DateTime expiresAt)
        {
            return new RefreshToken(accountId, token, expiresAt, null, tokenId, false);
        }

        public bool IsExpired => DateTime.UtcNow >= ExpiresAt;

        public void Revoke(string? reason = null)
        {
            if (IsRevoked) return; 

            IsRevoked = true;
            RevokedAt = DateTime.UtcNow; 
            RevokedReason = reason;
        }
    }
}