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
}

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
