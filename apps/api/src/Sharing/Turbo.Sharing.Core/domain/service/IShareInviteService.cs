using Turboapi.Sharing.value;

namespace Turboapi.Sharing.domain.service;

public interface IShareInviteService
{
    Task<InviteDto> CreateFriendInviteAsync(Guid inviterId, string inviteeEmail, TimeSpan? lifetime, CancellationToken cancellationToken = default);
    Task<InviteDto> CreateResourceInviteAsync(Guid inviterId, string inviteeEmail, Guid resourceId, Role role, TimeSpan? lifetime, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<InviteDto>> ListPendingForEmailAsync(string email, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<InviteDto>> ListMineAsync(Guid inviterId, CancellationToken cancellationToken = default);
    /// <summary>
    /// Redeems every pending invite addressed to <paramref name="email"/>
    /// in favour of <paramref name="userId"/>. Materializes resource invites
    /// into grants and friend invites into accepted friendships.
    /// </summary>
    Task<int> RedeemAllForUserAsync(Guid userId, string email, CancellationToken cancellationToken = default);
}

public sealed record InviteDto(
    Guid Id,
    Guid InviterId,
    string InviteeEmail,
    Guid? ResourceId,
    string? Role,
    DateTime CreatedAt,
    DateTime? ExpiresAt);
