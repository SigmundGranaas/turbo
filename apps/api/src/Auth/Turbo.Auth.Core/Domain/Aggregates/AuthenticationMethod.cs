using Turboapi.Auth.Domain.Exceptions;

namespace Turboapi.Auth.Domain.Aggregates
{
    public abstract class AuthenticationMethod
    {
        public Guid Id { get; protected set; }
        public Guid AccountId { get; protected set; } // Foreign key to Account
        public string ProviderName { get; protected set; } = null!;
        public DateTime CreatedAt { get; protected set; }
        public DateTime? LastUsedAt { get; protected set; }

        protected AuthenticationMethod() 
        {
        }

        protected AuthenticationMethod(Guid id, Guid accountId, string providerName)
        {
            if (id == Guid.Empty)
                throw new DomainException("AuthenticationMethod ID cannot be empty.");
            if (accountId == Guid.Empty)
                throw new DomainException("Account ID for AuthenticationMethod cannot be empty.");
            if (string.IsNullOrWhiteSpace(providerName))
                throw new DomainException("Provider name for AuthenticationMethod cannot be empty.");

            Id = id;
            AccountId = accountId;
            ProviderName = providerName;
            CreatedAt = DateTime.UtcNow;
        }

        public void UpdateLastUsed()
        {
            LastUsedAt = DateTime.UtcNow; 
        }
    }
}