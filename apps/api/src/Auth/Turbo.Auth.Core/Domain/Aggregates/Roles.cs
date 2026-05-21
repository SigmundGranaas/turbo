using Turboapi.Auth.Domain.Exceptions;

namespace Turboapi.Auth.Domain.Aggregates
{
    public class Role
    {
        public Guid Id { get; private set; }
        public Guid AccountId { get; private set; } 
        public string Name { get; private set; }
        public DateTime CreatedAt { get; private set; }

        // Private constructor for EF Core - NO validation
        private Role() 
        {
            // EF Core will populate properties
        }

        internal Role(Guid id, Guid accountId, string name)
        {
            if (string.IsNullOrWhiteSpace(name))
                throw new DomainException("Role name cannot be empty.");
            if (id == Guid.Empty)
                throw new DomainException("Role ID cannot be empty.");
            if (accountId == Guid.Empty)
                throw new DomainException("Account ID for role cannot be empty.");

            Id = id;
            AccountId = accountId;
            Name = name;
            CreatedAt = DateTime.UtcNow; // Ensure UTC
        }
    }
}