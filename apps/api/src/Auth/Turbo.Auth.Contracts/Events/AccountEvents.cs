using System.Text.Json.Serialization;
using Turbo.Messaging;

namespace Turboapi.Auth.Domain.Events
{
    public record AccountCreatedEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("email")] string Email,
        [property: JsonPropertyName("createdAt")] DateTime CreatedAt,
        [property: JsonPropertyName("initialRoles")] IEnumerable<string> InitialRoles
    ) : DomainEvent, IAccountAssociatedEvent;

    public record AccountLastLoginUpdatedEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("lastLoginAt")] DateTime LastLoginAt
    ) : DomainEvent, IAccountAssociatedEvent;

    public record RoleAddedToAccountEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("roleName")] string RoleName,
        [property: JsonPropertyName("addedAt")] DateTime AddedAt
    ) : DomainEvent, IAccountAssociatedEvent;

    public record PasswordAuthMethodAddedEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("authMethodId")] Guid AuthMethodId,
        [property: JsonPropertyName("addedAt")] DateTime AddedAt
    ) : DomainEvent, IAccountAssociatedEvent;

    public record OAuthAuthMethodAddedEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("authMethodId")] Guid AuthMethodId,
        [property: JsonPropertyName("providerName")] string ProviderName,
        [property: JsonPropertyName("externalUserId")] string ExternalUserId,
        [property: JsonPropertyName("addedAt")] DateTime AddedAt
    ) : DomainEvent, IAccountAssociatedEvent;

    public record AccountLoggedInEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("authMethodId")] Guid AuthMethodId,
        [property: JsonPropertyName("providerName")] string ProviderName,
        [property: JsonPropertyName("loggedInAt")] DateTime LoggedInAt
    ) : DomainEvent, IAccountAssociatedEvent;

    public record RefreshTokenGeneratedEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("refreshTokenId")] Guid RefreshTokenId,
        [property: JsonPropertyName("tokenIdentifier")] string TokenIdentifier,
        [property: JsonPropertyName("expiresAt")] DateTime ExpiresAt,
        [property: JsonPropertyName("generatedAt")] DateTime GeneratedAt
    ) : DomainEvent, IAccountAssociatedEvent;

    public record RefreshTokenRevokedEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("refreshTokenId")] Guid RefreshTokenId,
        [property: JsonPropertyName("revocationReason")] string? RevocationReason,
        [property: JsonPropertyName("revokedAt")] DateTime RevokedAt
    ) : DomainEvent, IAccountAssociatedEvent;

    public record SuspiciousRefreshTokenAttemptEvent(
        [property: JsonPropertyName("accountId")] Guid AccountId,
        [property: JsonPropertyName("tokenAttempted")] string TokenAttempted,
        [property: JsonPropertyName("reason")] string Reason
    ) : DomainEvent, IAccountAssociatedEvent;
}
