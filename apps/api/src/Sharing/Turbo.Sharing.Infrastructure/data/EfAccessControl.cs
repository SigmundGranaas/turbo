using Microsoft.EntityFrameworkCore;
using Turboapi.Sharing;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.data;

/// <summary>
/// EF Core-backed implementation of <see cref="IAccessControl"/>. Resolves
/// effective role by promoting through: public visibility (viewer), direct
/// user grant, group grant, owner.
///
/// Link grants are intentionally excluded from this resolver. They are
/// resolved at the HTTP boundary by matching the inbound <c>?token=</c>
/// against <see cref="GrantEntity.LinkToken"/>; the resulting grant is
/// then injected into request context without going through here.
/// </summary>
public sealed class EfAccessControl : IAccessControl
{
    private readonly SharingReadContext _db;

    public EfAccessControl(SharingReadContext db)
    {
        _db = db;
    }

    public async Task<bool> CanReadAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default)
    {
        var role = await EffectiveRoleAsync(userId, resourceId, cancellationToken);
        return role is not null;
    }

    public async Task<bool> CanWriteAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default)
    {
        var role = await EffectiveRoleAsync(userId, resourceId, cancellationToken);
        return role is not null && role.Value.AllowsWrite();
    }

    public async Task<EffectiveRole?> EffectiveRoleAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default)
    {
        var now = DateTime.UtcNow;

        var resource = await _db.Resources
            .AsNoTracking()
            .Where(r => r.Id == resourceId && r.DeletedAt == null)
            .Select(r => new { r.OwnerId, r.Visibility })
            .FirstOrDefaultAsync(cancellationToken);
        if (resource is null) return null;

        if (resource.OwnerId == userId) return EffectiveRole.Owner;

        var subjectUser = SubjectType.User.ToWire();
        var subjectGroup = SubjectType.Group.ToWire();

        var directGrant = await _db.Grants
            .AsNoTracking()
            .Where(g => g.ResourceId == resourceId
                        && g.SubjectType == subjectUser
                        && g.SubjectId == userId
                        && (g.ExpiresAt == null || g.ExpiresAt > now))
            .Select(g => g.Role)
            .FirstOrDefaultAsync(cancellationToken);

        EffectiveRole? best = null;
        if (directGrant is not null)
            best = EffectiveRoleExtensions.FromGrant(RoleExtensions.ParseRole(directGrant));

        // Group grant: subject_id is the group id; user must be a member.
        var groupGrant = await (from g in _db.Grants.AsNoTracking()
                                join gm in _db.GroupMembers.AsNoTracking()
                                    on g.SubjectId equals gm.GroupId
                                where g.ResourceId == resourceId
                                      && g.SubjectType == subjectGroup
                                      && gm.UserId == userId
                                      && (g.ExpiresAt == null || g.ExpiresAt > now)
                                select g.Role)
                                .FirstOrDefaultAsync(cancellationToken);
        if (groupGrant is not null)
        {
            var role = EffectiveRoleExtensions.FromGrant(RoleExtensions.ParseRole(groupGrant));
            best = best is null ? role : best.Value.Promote(role);
        }

        // Public visibility confers viewer access to anyone, after grants are
        // evaluated so an editor grant on a public resource still resolves to
        // editor for that user.
        if (resource.Visibility == Visibility.Public.ToWire())
            best = best is null ? EffectiveRole.Viewer : best.Value.Promote(EffectiveRole.Viewer);

        return best;
    }

    public async Task RequireReadAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default)
    {
        if (!await CanReadAsync(userId, resourceId, cancellationToken))
            throw new AccessDeniedException(userId, resourceId);
    }

    public async Task RequireWriteAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default)
    {
        if (!await CanWriteAsync(userId, resourceId, cancellationToken))
            throw new AccessDeniedException(userId, resourceId);
    }
}
