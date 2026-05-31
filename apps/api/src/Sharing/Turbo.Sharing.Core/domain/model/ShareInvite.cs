using Medo;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.domain.model;

/// <summary>
/// A pending share or friend invite addressed to an email that may not yet
/// belong to a signed-up account. On signup (or first-seen email) the
/// invite materializes into a friendship and/or a grant.
///
/// <see cref="ResourceId"/> is null for pure friend invites; both
/// <see cref="ResourceId"/> and <see cref="Role"/> are set for entity
/// invites.
/// </summary>
public class ShareInvite
{
    public Guid Id { get; private set; }
    public Guid InviterId { get; private set; }
    public string InviteeEmail { get; private set; } = string.Empty;
    public Guid? ResourceId { get; private set; }
    public Role? Role { get; private set; }
    public DateTime CreatedAt { get; private set; }
    public DateTime? ExpiresAt { get; private set; }
    public DateTime? RedeemedAt { get; private set; }
    public Guid? RedeemedByUserId { get; private set; }

    private ShareInvite() { }

    public static ShareInvite ForFriendship(Guid inviterId, string inviteeEmail, TimeSpan? lifetime = null)
        => Create(inviterId, inviteeEmail, resourceId: null, role: null, lifetime);

    public static ShareInvite ForResource(
        Guid inviterId, string inviteeEmail, Guid resourceId, Role role, TimeSpan? lifetime = null)
        => Create(inviterId, inviteeEmail, resourceId, role, lifetime);

    private static ShareInvite Create(
        Guid inviterId, string inviteeEmail, Guid? resourceId, Role? role, TimeSpan? lifetime)
    {
        if (string.IsNullOrWhiteSpace(inviteeEmail))
            throw new ArgumentException("Invitee email must not be empty", nameof(inviteeEmail));
        var now = DateTime.UtcNow;
        return new ShareInvite
        {
            Id = Uuid7.NewUuid7().ToGuid(),
            InviterId = inviterId,
            InviteeEmail = inviteeEmail.Trim().ToLowerInvariant(),
            ResourceId = resourceId,
            Role = role,
            CreatedAt = now,
            ExpiresAt = lifetime is null ? null : now.Add(lifetime.Value),
        };
    }

    public void Redeem(Guid redeemingUserId)
    {
        if (RedeemedAt is not null)
            throw new InvalidOperationException("Invite has already been redeemed.");
        if (ExpiresAt is not null && ExpiresAt < DateTime.UtcNow)
            throw new InvalidOperationException("Invite has expired.");
        RedeemedAt = DateTime.UtcNow;
        RedeemedByUserId = redeemingUserId;
    }

    public bool IsActive(DateTime asOf)
        => RedeemedAt is null && (ExpiresAt is null || ExpiresAt > asOf);

    public static ShareInvite Reconstitute(
        Guid id, Guid inviterId, string inviteeEmail, Guid? resourceId, Role? role,
        DateTime createdAt, DateTime? expiresAt, DateTime? redeemedAt, Guid? redeemedByUserId)
        => new()
        {
            Id = id,
            InviterId = inviterId,
            InviteeEmail = inviteeEmail,
            ResourceId = resourceId,
            Role = role,
            CreatedAt = createdAt,
            ExpiresAt = expiresAt,
            RedeemedAt = redeemedAt,
            RedeemedByUserId = redeemedByUserId,
        };
}
