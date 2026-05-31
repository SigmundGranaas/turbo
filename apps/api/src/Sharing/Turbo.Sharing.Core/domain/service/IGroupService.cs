namespace Turboapi.Sharing.domain.service;

public interface IGroupService
{
    Task<GroupDto> CreateAsync(Guid ownerId, string name, CancellationToken cancellationToken = default);
    Task<GroupDto?> GetAsync(Guid actorId, Guid groupId, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<GroupDto>> ListMineAsync(Guid userId, CancellationToken cancellationToken = default);
    Task DeleteAsync(Guid actorId, Guid groupId, CancellationToken cancellationToken = default);
    Task RenameAsync(Guid actorId, Guid groupId, string name, CancellationToken cancellationToken = default);
    Task AddMemberAsync(Guid actorId, Guid groupId, Guid userId, CancellationToken cancellationToken = default);
    Task RemoveMemberAsync(Guid actorId, Guid groupId, Guid userId, CancellationToken cancellationToken = default);
}

public sealed record GroupDto(
    Guid Id,
    Guid OwnerId,
    string Name,
    DateTime CreatedAt,
    DateTime UpdatedAt,
    IReadOnlyList<GroupMemberDto> Members);

public sealed record GroupMemberDto(Guid UserId, string Role, DateTime JoinedAt);
