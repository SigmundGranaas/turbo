using Turboapi.Sharing.value;

namespace Turboapi.Sharing.domain.service;

public interface IFriendshipService
{
    Task<FriendshipDto> RequestAsync(Guid initiatorId, Guid otherUserId, CancellationToken cancellationToken = default);
    Task<FriendshipDto> AcceptAsync(Guid acceptingUserId, Guid otherUserId, CancellationToken cancellationToken = default);
    Task BlockAsync(Guid blockingUserId, Guid otherUserId, CancellationToken cancellationToken = default);
    Task RemoveAsync(Guid userId, Guid otherUserId, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<FriendshipDto>> ListAsync(Guid userId, FriendshipStatus? status, CancellationToken cancellationToken = default);
}

public sealed record FriendshipDto(
    Guid OtherUserId,
    Guid InitiatorId,
    string Status,
    DateTime CreatedAt,
    DateTime? AcceptedAt);
