using Medo;

namespace Turboapi.Sharing.domain.model;

/// <summary>
/// A named circle of users that can hold grants. The grant's subject_id
/// is the group id; the access check joins through <see cref="GroupMember"/>
/// to determine if the requesting user is in the group.
/// </summary>
public class Group
{
    public Guid Id { get; private set; }
    public Guid OwnerId { get; private set; }
    public string Name { get; private set; } = string.Empty;
    public DateTime CreatedAt { get; private set; }
    public DateTime UpdatedAt { get; private set; }

    private Group() { }

    public static Group Create(Guid ownerId, string name)
    {
        if (string.IsNullOrWhiteSpace(name))
            throw new ArgumentException("Group name must not be empty", nameof(name));
        var now = DateTime.UtcNow;
        return new Group
        {
            Id = Uuid7.NewUuid7().ToGuid(),
            OwnerId = ownerId,
            Name = name.Trim(),
            CreatedAt = now,
            UpdatedAt = now,
        };
    }

    public void Rename(Guid actorId, string name)
    {
        if (actorId != OwnerId)
            throw new InvalidOperationException("Only the group owner can rename.");
        if (string.IsNullOrWhiteSpace(name))
            throw new ArgumentException("Group name must not be empty", nameof(name));
        Name = name.Trim();
        UpdatedAt = DateTime.UtcNow;
    }

    public static Group Reconstitute(Guid id, Guid ownerId, string name, DateTime createdAt, DateTime updatedAt)
        => new()
        {
            Id = id,
            OwnerId = ownerId,
            Name = name,
            CreatedAt = createdAt,
            UpdatedAt = updatedAt,
        };
}

public class GroupMember
{
    public Guid GroupId { get; private set; }
    public Guid UserId { get; private set; }
    public GroupMemberRole Role { get; private set; }
    public DateTime JoinedAt { get; private set; }

    private GroupMember() { }

    public static GroupMember Add(Guid groupId, Guid userId, GroupMemberRole role = GroupMemberRole.Member)
        => new()
        {
            GroupId = groupId,
            UserId = userId,
            Role = role,
            JoinedAt = DateTime.UtcNow,
        };

    public static GroupMember Reconstitute(Guid groupId, Guid userId, GroupMemberRole role, DateTime joinedAt)
        => new()
        {
            GroupId = groupId,
            UserId = userId,
            Role = role,
            JoinedAt = joinedAt,
        };
}

public enum GroupMemberRole
{
    Member = 0,
    Admin = 1,
}

public static class GroupMemberRoleExtensions
{
    public static string ToWire(this GroupMemberRole role) => role switch
    {
        GroupMemberRole.Member => "member",
        GroupMemberRole.Admin => "admin",
        _ => throw new ArgumentOutOfRangeException(nameof(role), role, null),
    };

    public static GroupMemberRole ParseGroupMemberRole(string raw) => raw switch
    {
        "member" => GroupMemberRole.Member,
        "admin" => GroupMemberRole.Admin,
        _ => throw new ArgumentException($"Unknown group member role '{raw}'", nameof(raw)),
    };
}
