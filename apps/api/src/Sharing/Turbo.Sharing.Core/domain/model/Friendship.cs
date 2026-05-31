using Turboapi.Sharing.domain.exception;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.domain.model;

/// <summary>
/// Friend graph edge. Stored once per pair using canonical ordering
/// (lower_user_id &lt; higher_user_id) so a (a,b) and (b,a) request collapse
/// to one row. <see cref="InitiatorId"/> records who sent the request.
/// </summary>
public class Friendship
{
    public Guid LowerUserId { get; private set; }
    public Guid HigherUserId { get; private set; }
    public Guid InitiatorId { get; private set; }
    public FriendshipStatus Status { get; private set; }
    public DateTime CreatedAt { get; private set; }
    public DateTime? AcceptedAt { get; private set; }

    private Friendship() { }

    public static Friendship Request(Guid initiatorId, Guid otherUserId)
    {
        if (initiatorId == otherUserId)
            throw new ArgumentException("Cannot befriend yourself", nameof(otherUserId));
        if (initiatorId == Guid.Empty || otherUserId == Guid.Empty)
            throw new ArgumentException("User ids must be non-empty");

        var (lower, higher) = Canonicalize(initiatorId, otherUserId);
        return new Friendship
        {
            LowerUserId = lower,
            HigherUserId = higher,
            InitiatorId = initiatorId,
            Status = FriendshipStatus.Pending,
            CreatedAt = DateTime.UtcNow,
        };
    }

    public void Accept(Guid acceptingUserId)
    {
        if (acceptingUserId == InitiatorId)
            throw new InvalidOperationException("The initiator cannot accept their own request.");
        if (acceptingUserId != LowerUserId && acceptingUserId != HigherUserId)
            throw new InvalidOperationException("Only the invited user can accept the request.");
        if (Status != FriendshipStatus.Pending)
            throw new InvalidOperationException($"Cannot accept a friendship in status {Status}.");

        Status = FriendshipStatus.Accepted;
        AcceptedAt = DateTime.UtcNow;
    }

    public void Block(Guid blockingUserId)
    {
        if (blockingUserId != LowerUserId && blockingUserId != HigherUserId)
            throw new InvalidOperationException("Only a participant can block the friendship.");
        Status = FriendshipStatus.Blocked;
    }

    public bool Involves(Guid userId) => userId == LowerUserId || userId == HigherUserId;

    public Guid Other(Guid userId)
    {
        if (userId == LowerUserId) return HigherUserId;
        if (userId == HigherUserId) return LowerUserId;
        throw new ArgumentException("User is not a participant in this friendship.", nameof(userId));
    }

    public static (Guid Lower, Guid Higher) Canonicalize(Guid a, Guid b)
        => a.CompareTo(b) < 0 ? (a, b) : (b, a);

    public static Friendship Reconstitute(
        Guid lowerUserId, Guid higherUserId, Guid initiatorId,
        FriendshipStatus status, DateTime createdAt, DateTime? acceptedAt)
        => new()
        {
            LowerUserId = lowerUserId,
            HigherUserId = higherUserId,
            InitiatorId = initiatorId,
            Status = status,
            CreatedAt = createdAt,
            AcceptedAt = acceptedAt,
        };
}
