using System;
using Turboapi.Auth.Domain.Constants;
using Turboapi.Auth.Domain.Exceptions;

namespace Turboapi.Auth.Domain.Aggregates
{
    public class PasswordAuthMethod : AuthenticationMethod
    {
        public string PasswordHash { get; private set; }

        // Private constructor for EF Core - NO validation, NO base constructor call with parameters
        private PasswordAuthMethod() 
        {
        }

        public PasswordAuthMethod(Guid id, Guid accountId, string passwordHash)
            : base(id, accountId, AuthProviderNames.Password)
        {
            if (string.IsNullOrWhiteSpace(passwordHash))
                throw new DomainException("Password hash cannot be empty for PasswordAuthMethod.");
            PasswordHash = passwordHash;
        }

        /// <summary>
        /// Replaces the stored hash. Callers are responsible for having
        /// verified the current password and produced the new hash via
        /// <c>IPasswordHasher</c>; this method only mutates state.
        /// </summary>
        public void UpdatePasswordHash(string newPasswordHash)
        {
            if (string.IsNullOrWhiteSpace(newPasswordHash))
                throw new DomainException("Password hash cannot be empty for PasswordAuthMethod.");
            PasswordHash = newPasswordHash;
            UpdateLastUsed();
        }
    }
}