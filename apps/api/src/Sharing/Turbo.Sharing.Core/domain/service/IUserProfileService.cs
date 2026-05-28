namespace Turboapi.Sharing.domain.service;

/// <summary>
/// Per-user sharing identity: the friend code that friends use to find
/// each other. Generated lazily on first read of the calling user's own
/// profile; subsequent reads return the same code.
/// </summary>
public interface IUserProfileService
{
    /// <summary>
    /// Returns the calling user's friend code, generating one if this is
    /// the first call. Idempotent.
    /// </summary>
    Task<UserProfileDto> EnsureProfileAsync(Guid userId, CancellationToken cancellationToken = default);

    /// <summary>
    /// Resolves a friend code (case-insensitive) to a user id, or null
    /// if no profile matches. Used by the friend-add flow.
    /// </summary>
    Task<Guid?> LookupByCodeAsync(string friendCode, CancellationToken cancellationToken = default);
}

public sealed record UserProfileDto(Guid UserId, string FriendCode, DateTime CreatedAt);
