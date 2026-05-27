namespace Turboapi.Sharing.data.model;

/// <summary>EF Core row for the sharing.resources table.</summary>
public class ResourceEntity
{
    public required Guid Id { get; set; }
    public required string Type { get; set; }
    public required Guid OwnerId { get; set; }
    public required string Visibility { get; set; }
    public long Version { get; set; }
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
}

/// <summary>
/// EF Core row for the sharing.grants table. Composite key is
/// (ResourceId, SubjectType, SubjectId); link grants populate
/// <see cref="LinkToken"/> with a per-grant secret.
/// </summary>
public class GrantEntity
{
    public required Guid ResourceId { get; set; }
    public required string SubjectType { get; set; }
    public required Guid SubjectId { get; set; }
    public required string Role { get; set; }
    public required Guid GrantedBy { get; set; }
    public DateTime GrantedAt { get; set; }
    public DateTime? ExpiresAt { get; set; }
    public string? LinkToken { get; set; }
}

/// <summary>
/// EF Core row for the sharing.friendships table. Canonical ordering:
/// LowerUserId &lt; HigherUserId.
/// </summary>
public class FriendshipEntity
{
    public required Guid LowerUserId { get; set; }
    public required Guid HigherUserId { get; set; }
    public required Guid InitiatorId { get; set; }
    public required string Status { get; set; }
    public DateTime CreatedAt { get; set; }
    public DateTime? AcceptedAt { get; set; }
}

public class GroupEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }
    public required string Name { get; set; }
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
}

public class GroupMemberEntity
{
    public required Guid GroupId { get; set; }
    public required Guid UserId { get; set; }
    public required string Role { get; set; }
    public DateTime JoinedAt { get; set; }
}

public class ShareInviteEntity
{
    public required Guid Id { get; set; }
    public required Guid InviterId { get; set; }
    public required string InviteeEmail { get; set; }
    public Guid? ResourceId { get; set; }
    public string? Role { get; set; }
    public DateTime CreatedAt { get; set; }
    public DateTime? ExpiresAt { get; set; }
    public DateTime? RedeemedAt { get; set; }
    public Guid? RedeemedByUserId { get; set; }
}
