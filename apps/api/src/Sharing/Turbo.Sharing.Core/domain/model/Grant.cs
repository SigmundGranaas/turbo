using Medo;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.domain.model;

/// <summary>
/// One access grant on one resource. The grant's primary key is
/// (resource_id, subject_type, subject_id) so a (user, group, link) can
/// hold at most one grant per resource — re-granting upgrades the role.
///
/// Link grants set <see cref="SubjectId"/> to a synthetic
/// <c>Guid.Empty</c> placeholder and store the redemption token in
/// <see cref="LinkToken"/>. They are resolved at the HTTP boundary, not
/// through the per-subject join.
/// </summary>
public class Grant
{
    public Guid ResourceId { get; private set; }
    public SubjectType SubjectType { get; private set; }
    public Guid SubjectId { get; private set; }
    public Role Role { get; private set; }
    public Guid GrantedBy { get; private set; }
    public DateTime GrantedAt { get; private set; }
    public DateTime? ExpiresAt { get; private set; }
    public string? LinkToken { get; private set; }

    private Grant() { }

    public static Grant ToUser(Guid resourceId, Guid userId, Role role, Guid grantedBy, DateTime? expiresAt = null)
    {
        if (userId == Guid.Empty)
            throw new ArgumentException("User grant requires a non-empty user id", nameof(userId));
        return new Grant
        {
            ResourceId = resourceId,
            SubjectType = SubjectType.User,
            SubjectId = userId,
            Role = role,
            GrantedBy = grantedBy,
            GrantedAt = DateTime.UtcNow,
            ExpiresAt = expiresAt,
        };
    }

    public static Grant ToGroup(Guid resourceId, Guid groupId, Role role, Guid grantedBy, DateTime? expiresAt = null)
    {
        if (groupId == Guid.Empty)
            throw new ArgumentException("Group grant requires a non-empty group id", nameof(groupId));
        return new Grant
        {
            ResourceId = resourceId,
            SubjectType = SubjectType.Group,
            SubjectId = groupId,
            Role = role,
            GrantedBy = grantedBy,
            GrantedAt = DateTime.UtcNow,
            ExpiresAt = expiresAt,
        };
    }

    public static Grant AsLink(Guid resourceId, Role role, Guid grantedBy, DateTime? expiresAt = null)
    {
        var token = Uuid7.NewUuid7().ToString();
        return new Grant
        {
            ResourceId = resourceId,
            SubjectType = SubjectType.Link,
            // Composite PK requires non-null subject_id; use a synthetic per-token UUID
            // so multiple link grants on the same resource don't collide.
            SubjectId = Uuid7.NewUuid7().ToGuid(),
            Role = role,
            GrantedBy = grantedBy,
            GrantedAt = DateTime.UtcNow,
            ExpiresAt = expiresAt,
            LinkToken = token,
        };
    }

    public void UpgradeRole(Role newRole)
    {
        if (newRole == Role) return;
        Role = newRole;
    }

    public bool IsActive(DateTime asOf)
        => ExpiresAt is null || ExpiresAt > asOf;

    public static Grant Reconstitute(
        Guid resourceId, SubjectType subjectType, Guid subjectId, Role role,
        Guid grantedBy, DateTime grantedAt, DateTime? expiresAt, string? linkToken)
        => new()
        {
            ResourceId = resourceId,
            SubjectType = subjectType,
            SubjectId = subjectId,
            Role = role,
            GrantedBy = grantedBy,
            GrantedAt = grantedAt,
            ExpiresAt = expiresAt,
            LinkToken = linkToken,
        };
}
