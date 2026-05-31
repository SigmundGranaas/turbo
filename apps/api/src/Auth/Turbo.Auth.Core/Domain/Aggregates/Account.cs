using Turbo.Messaging;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Events;
using Turboapi.Auth.Domain.Exceptions;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Domain.Aggregates
{
    public class Account : IHasDomainEvents
    {
        public Guid Id { get; private set; }
        public string Email { get; private set; } = null!;
        public bool IsActive { get; private set; } // New property
        public DateTime CreatedAt { get; private set; }
        public DateTime? LastLoginAt { get; private set; }

        private readonly List<Role> _roles = new();
        public IReadOnlyCollection<Role> Roles => _roles.AsReadOnly();

        private readonly List<AuthenticationMethod> _authenticationMethods = new();
        public IReadOnlyCollection<AuthenticationMethod> AuthenticationMethods => _authenticationMethods.AsReadOnly();

        private readonly List<RefreshToken> _refreshTokens = new();
        public IReadOnlyCollection<RefreshToken> RefreshTokens => _refreshTokens.AsReadOnly();

        private readonly List<IDomainEvent> _domainEvents = new();
        public IReadOnlyCollection<IDomainEvent> DomainEvents => _domainEvents.AsReadOnly();

        private Account() { }

        private Account(Guid id, string email)
        {
            if (id == Guid.Empty)
                throw new DomainException("Account ID cannot be empty.");

            if (string.IsNullOrWhiteSpace(email) || !IsValidEmail(email))
                throw new DomainException("Invalid email format.");

            Id = id;
            Email = email.ToLowerInvariant();
            IsActive = true; // Accounts are active by default
            CreatedAt = DateTime.UtcNow;
        }

        public static Account Create(Guid id, string email, IEnumerable<string> initialRoleNames)
        {
            if (initialRoleNames == null || !initialRoleNames.Any())
                throw new DomainException("Account must be created with at least one initial role.");
            
            var distinctRoleNames = initialRoleNames.Where(rn => !string.IsNullOrWhiteSpace(rn)).Distinct().ToList();
            if (!distinctRoleNames.Any())
                 throw new DomainException("Account must be created with at least one valid initial role name.");

            var account = new Account(id, email);

            foreach (var roleName in distinctRoleNames)
            {
                account.AddRoleInternal(roleName, true);
            }
            
            account.AddDomainEvent(new AccountCreatedEvent(account.Id, account.Email, account.CreatedAt, distinctRoleNames));
            return account;
        }

        public void Deactivate()
        {
            if (!IsActive) return;
            IsActive = false;
            // Optionally, add a domain event for deactivation
            // AddDomainEvent(new AccountDeactivatedEvent(Id, DateTime.UtcNow));
        }

        public void UpdateLastLogin()
        {
            LastLoginAt = DateTime.UtcNow;
            AddDomainEvent(new AccountLastLoginUpdatedEvent(Id, LastLoginAt.Value));
        }

        /// <summary>
        /// Emits an <see cref="AccountLoggedInEvent"/> for audit / downstream
        /// consumers (notifications, security analytics, etc.) on top of the
        /// state-change event raised by <see cref="UpdateLastLogin"/>. Both the
        /// password-login and OAuth-login flows call this so the audit feed is
        /// consistent across providers.
        /// </summary>
        public void RecordLoggedIn(Guid authMethodId, string providerName)
        {
            AddDomainEvent(new AccountLoggedInEvent(Id, authMethodId, providerName, DateTime.UtcNow));
        }

        public void AddRole(string roleName)
        {
            AddRoleInternal(roleName, false);
        }

        private void AddRoleInternal(string roleName, bool isInitialCreation)
        {
            if (string.IsNullOrWhiteSpace(roleName))
                throw new DomainException("Role name cannot be empty.");

            if (_roles.Any(r => r.Name.Equals(roleName, StringComparison.OrdinalIgnoreCase)))
            {
                if (!isInitialCreation) 
                {
                    return; 
                }
            }
            
            var newRole = new Role(Guid.NewGuid(), Id, roleName);
            _roles.Add(newRole);

            if (!isInitialCreation)
            {
                 AddDomainEvent(new RoleAddedToAccountEvent(Id, roleName, newRole.CreatedAt));
            }
        }
        
        public Result<RefreshToken, RefreshTokenError> RotateRefreshToken(
            string oldTokenString,
            string newRefreshTokenValue,
            DateTime newRefreshTokenExpiry)
        {
            var oldToken = _refreshTokens.FirstOrDefault(rt => rt.Token == oldTokenString && !rt.IsRevoked);

            if (oldToken == null)
            {
                AddDomainEvent(new SuspiciousRefreshTokenAttemptEvent(Id, oldTokenString, "Token not found or already revoked for this account."));
                return RefreshTokenError.InvalidToken;
            }

            if (oldToken.IsExpired)
            {
                oldToken.Revoke("Expired during refresh attempt");
                AddDomainEvent(new RefreshTokenRevokedEvent(Id, oldToken.Id, oldToken.RevokedReason, oldToken.RevokedAt!.Value));
                return RefreshTokenError.Expired;
            }
            
            oldToken.Revoke("Rotated: New token pair issued");
            AddDomainEvent(new RefreshTokenRevokedEvent(Id, oldToken.Id, oldToken.RevokedReason, oldToken.RevokedAt!.Value));

            var newDomainRefreshToken = RefreshToken.Create(Id, newRefreshTokenValue, newRefreshTokenExpiry);
            _refreshTokens.Add(newDomainRefreshToken);
            
            AddDomainEvent(new RefreshTokenGeneratedEvent(Id, newDomainRefreshToken.Id, newDomainRefreshToken.Token, newDomainRefreshToken.ExpiresAt, newDomainRefreshToken.CreatedAt));
            
            UpdateLastLogin();

            return newDomainRefreshToken;
        }
        
        public RefreshToken AddNewRefreshToken(string tokenValue, DateTime expiresAt)
        {
            var newDomainRefreshToken = RefreshToken.Create(Id, tokenValue, expiresAt);
            _refreshTokens.Add(newDomainRefreshToken);
            AddDomainEvent(new RefreshTokenGeneratedEvent(Id, newDomainRefreshToken.Id, newDomainRefreshToken.Token, newDomainRefreshToken.ExpiresAt, newDomainRefreshToken.CreatedAt));
            return newDomainRefreshToken;
        }
        
        public void AddPasswordAuthMethod(string password, IPasswordHasher passwordHasher)
        {
            if (string.IsNullOrWhiteSpace(password))
                throw new DomainException("Password cannot be empty.");
            if (passwordHasher == null)
                throw new ArgumentNullException(nameof(passwordHasher));

            if (_authenticationMethods.Any(am => am is PasswordAuthMethod))
                throw new DomainException("Account already has a password authentication method.");

            var passwordHash = passwordHasher.HashPassword(password);
            var authMethodId = Guid.NewGuid();
            var passwordAuth = new PasswordAuthMethod(authMethodId, Id, passwordHash);
            
            _authenticationMethods.Add(passwordAuth);
            AddDomainEvent(new PasswordAuthMethodAddedEvent(Id, authMethodId, passwordAuth.CreatedAt));
        }

        public void AddOAuthAuthMethod(string providerName, string externalUserId, 
                                       string? accessToken = null, string? oauthRefreshToken = null, DateTime? tokenExpiry = null)
        {
            if (string.IsNullOrWhiteSpace(providerName))
                throw new DomainException("OAuth Provider name cannot be empty.");
            if (string.IsNullOrWhiteSpace(externalUserId))
                throw new DomainException("OAuth ExternalUser ID cannot be empty.");
            
            if (_authenticationMethods.OfType<OAuthAuthMethod>()
                .Any(oam => oam.ProviderName.Equals(providerName, StringComparison.OrdinalIgnoreCase) &&
                             oam.ExternalUserId.Equals(externalUserId, StringComparison.OrdinalIgnoreCase)))
            {
                throw new DomainException($"OAuth method for provider '{providerName}' and external ID '{externalUserId}' already exists for this account.");
            }

            var authMethodId = Guid.NewGuid();
            var oAuthAuth = new OAuthAuthMethod(authMethodId, Id, providerName, externalUserId, accessToken, oauthRefreshToken, tokenExpiry);

            _authenticationMethods.Add(oAuthAuth);
            AddDomainEvent(new OAuthAuthMethodAddedEvent(Id, authMethodId, providerName, externalUserId, oAuthAuth.CreatedAt));
        }

        internal void AddRefreshToken(RefreshToken refreshToken)
        {
             if (refreshToken.AccountId != Id)
                throw new DomainException("RefreshToken must belong to this account.");
            _refreshTokens.Add(refreshToken);
        }

        public void ClearDomainEvents()
        {
            _domainEvents.Clear();
        }

        private void AddDomainEvent(IDomainEvent domainEvent)
        {
            _domainEvents.Add(domainEvent);
        }

        private static bool IsValidEmail(string email)
        {
            if (string.IsNullOrWhiteSpace(email))
                return false;
            try
            {
                var addr = new System.Net.Mail.MailAddress(email);
                return addr.Address == email.Trim();
            }
            catch
            {
                return false;
            }
        }

        public void RevokeRefreshToken(string tokenToRevoke, string reason)
        {
            var refreshToken = _refreshTokens.FirstOrDefault(rt => rt.Token == tokenToRevoke);

            if (refreshToken == null || refreshToken.IsRevoked)
            {
                return;
            }

            refreshToken.Revoke(reason);
            AddDomainEvent(new RefreshTokenRevokedEvent(Id, refreshToken.Id, refreshToken.RevokedReason, refreshToken.RevokedAt!.Value));
        }
    }
}