using Microsoft.EntityFrameworkCore;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.domain.exception;
using Turboapi.Sharing.domain.model;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.data;

/// <summary>
/// Issues and revokes grants. Only the resource owner can administer
/// grants on a resource — this is intentional for the foundation; later
/// we can extend to delegated administrators via group-admin roles.
/// </summary>
public sealed class EfGrantService : IGrantService
{
    private readonly SharingReadContext _db;

    public EfGrantService(SharingReadContext db) => _db = db;

    public async Task<GrantDto> GrantToUserAsync(Guid actorId, Guid resourceId, Guid userId, Role role, DateTime? expiresAt, CancellationToken cancellationToken = default)
    {
        await RequireOwner(actorId, resourceId, cancellationToken);
        if (userId == actorId)
            throw new ArgumentException("Cannot grant to yourself.", nameof(userId));

        var domain = Grant.ToUser(resourceId, userId, role, actorId, expiresAt);
        return await UpsertAsync(domain, cancellationToken);
    }

    public async Task<GrantDto> GrantToGroupAsync(Guid actorId, Guid resourceId, Guid groupId, Role role, DateTime? expiresAt, CancellationToken cancellationToken = default)
    {
        await RequireOwner(actorId, resourceId, cancellationToken);
        var groupExists = await _db.Groups.AsNoTracking().AnyAsync(g => g.Id == groupId, cancellationToken);
        if (!groupExists) throw new InvalidOperationException("Group not found.");

        var domain = Grant.ToGroup(resourceId, groupId, role, actorId, expiresAt);
        return await UpsertAsync(domain, cancellationToken);
    }

    public async Task<LinkGrantDto> GrantAsLinkAsync(Guid actorId, Guid resourceId, Role role, DateTime? expiresAt, CancellationToken cancellationToken = default)
    {
        await RequireOwner(actorId, resourceId, cancellationToken);
        var domain = Grant.AsLink(resourceId, role, actorId, expiresAt);
        var entity = ToEntity(domain);
        _db.Grants.Add(entity);
        await _db.SaveChangesAsync(cancellationToken);
        return new LinkGrantDto(
            entity.ResourceId, entity.SubjectId, entity.LinkToken!, entity.Role,
            entity.GrantedAt, entity.ExpiresAt);
    }

    public async Task RevokeUserAsync(Guid actorId, Guid resourceId, Guid userId, CancellationToken cancellationToken = default)
        => await RevokeAsync(actorId, resourceId, SubjectType.User, userId, cancellationToken);

    public async Task RevokeGroupAsync(Guid actorId, Guid resourceId, Guid groupId, CancellationToken cancellationToken = default)
        => await RevokeAsync(actorId, resourceId, SubjectType.Group, groupId, cancellationToken);

    public async Task RevokeLinkAsync(Guid actorId, Guid resourceId, Guid linkSubjectId, CancellationToken cancellationToken = default)
        => await RevokeAsync(actorId, resourceId, SubjectType.Link, linkSubjectId, cancellationToken);

    public async Task<IReadOnlyList<GrantDto>> ListForResourceAsync(Guid actorId, Guid resourceId, CancellationToken cancellationToken = default)
    {
        await RequireOwner(actorId, resourceId, cancellationToken);
        var rows = await _db.Grants
            .AsNoTracking()
            .Where(g => g.ResourceId == resourceId)
            .ToListAsync(cancellationToken);
        return rows.Select(g => new GrantDto(
            g.ResourceId, g.SubjectType, g.SubjectId, g.Role,
            g.GrantedBy, g.GrantedAt, g.ExpiresAt)).ToList();
    }

    private async Task<GrantDto> UpsertAsync(Grant domain, CancellationToken cancellationToken)
    {
        var subjectType = domain.SubjectType.ToWire();
        var existing = await _db.Grants
            .FirstOrDefaultAsync(g =>
                g.ResourceId == domain.ResourceId
                && g.SubjectType == subjectType
                && g.SubjectId == domain.SubjectId,
                cancellationToken);

        if (existing is null)
        {
            var entity = ToEntity(domain);
            _db.Grants.Add(entity);
            await _db.SaveChangesAsync(cancellationToken);
            await _db.Entry(entity).ReloadAsync(cancellationToken);
            return ToDto(entity);
        }

        existing.Role = domain.Role.ToWire();
        existing.ExpiresAt = domain.ExpiresAt;
        existing.GrantedBy = domain.GrantedBy;
        await _db.SaveChangesAsync(cancellationToken);
        return ToDto(existing);
    }

    private async Task RevokeAsync(Guid actorId, Guid resourceId, SubjectType subjectType, Guid subjectId, CancellationToken cancellationToken)
    {
        await RequireOwner(actorId, resourceId, cancellationToken);
        var wireSubject = subjectType.ToWire();
        var entity = await _db.Grants
            .FirstOrDefaultAsync(g =>
                g.ResourceId == resourceId
                && g.SubjectType == wireSubject
                && g.SubjectId == subjectId,
                cancellationToken);
        if (entity is null) return;
        _db.Grants.Remove(entity);
        await _db.SaveChangesAsync(cancellationToken);
    }

    private async Task RequireOwner(Guid actorId, Guid resourceId, CancellationToken cancellationToken)
    {
        var owner = await _db.Resources
            .AsNoTracking()
            .Where(r => r.Id == resourceId && r.DeletedAt == null)
            .Select(r => (Guid?)r.OwnerId)
            .FirstOrDefaultAsync(cancellationToken)
            ?? throw new ResourceNotFoundException(resourceId);
        if (owner != actorId)
            throw new AccessDeniedException(actorId, resourceId);
    }

    private static GrantEntity ToEntity(Grant domain) => new()
    {
        ResourceId = domain.ResourceId,
        SubjectType = domain.SubjectType.ToWire(),
        SubjectId = domain.SubjectId,
        Role = domain.Role.ToWire(),
        GrantedBy = domain.GrantedBy,
        GrantedAt = domain.GrantedAt,
        ExpiresAt = domain.ExpiresAt,
        LinkToken = domain.LinkToken,
    };

    private static GrantDto ToDto(GrantEntity g)
        => new(g.ResourceId, g.SubjectType, g.SubjectId, g.Role, g.GrantedBy, g.GrantedAt, g.ExpiresAt);
}
