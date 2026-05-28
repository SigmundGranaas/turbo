using Turboapi.Sharing.value;

namespace Turboapi.Sharing.domain.service;

public interface IGrantService
{
    Task<GrantDto> GrantToUserAsync(Guid actorId, Guid resourceId, Guid userId, Role role, DateTime? expiresAt, CancellationToken cancellationToken = default);
    Task<GrantDto> GrantToGroupAsync(Guid actorId, Guid resourceId, Guid groupId, Role role, DateTime? expiresAt, CancellationToken cancellationToken = default);
    Task<LinkGrantDto> GrantAsLinkAsync(Guid actorId, Guid resourceId, Role role, DateTime? expiresAt, CancellationToken cancellationToken = default);
    Task RevokeUserAsync(Guid actorId, Guid resourceId, Guid userId, CancellationToken cancellationToken = default);
    Task RevokeGroupAsync(Guid actorId, Guid resourceId, Guid groupId, CancellationToken cancellationToken = default);
    Task RevokeLinkAsync(Guid actorId, Guid resourceId, Guid linkSubjectId, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<GrantDto>> ListForResourceAsync(Guid actorId, Guid resourceId, CancellationToken cancellationToken = default);

    /// <summary>
    /// Redeems a link grant token in favour of <paramref name="redeemerId"/>:
    /// materializes a user grant on the same resource with the link's role,
    /// so the user can find the resource via their normal sync stream and
    /// the owner can revoke per-user without touching the link.
    ///
    /// Idempotent: if the user already has a stronger grant on the
    /// resource (direct or owner), this is a no-op except for returning
    /// the resource id.
    /// </summary>
    Task<LinkRedemptionDto> RedeemLinkAsync(Guid redeemerId, string linkToken, CancellationToken cancellationToken = default);
}

public sealed record LinkRedemptionDto(Guid ResourceId, string ResourceType, string Role);

public sealed record GrantDto(
    Guid ResourceId,
    string SubjectType,
    Guid SubjectId,
    string Role,
    Guid GrantedBy,
    DateTime GrantedAt,
    DateTime? ExpiresAt);

public sealed record LinkGrantDto(
    Guid ResourceId,
    Guid SubjectId,
    string LinkToken,
    string Role,
    DateTime GrantedAt,
    DateTime? ExpiresAt);
