using Microsoft.EntityFrameworkCore;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.domain.model;
using Turboapi.Sharing.domain.service;

namespace Turboapi.Sharing.data;

public sealed class EfGroupService : IGroupService
{
    private readonly SharingReadContext _db;

    public EfGroupService(SharingReadContext db) => _db = db;

    public async Task<GroupDto> CreateAsync(Guid ownerId, string name, CancellationToken cancellationToken = default)
    {
        var domain = Group.Create(ownerId, name);
        var entity = new GroupEntity
        {
            Id = domain.Id,
            OwnerId = domain.OwnerId,
            Name = domain.Name,
            UpdatedAt = domain.UpdatedAt,
        };
        _db.Groups.Add(entity);
        // Creator joins as admin so they have implicit membership for queries.
        _db.GroupMembers.Add(new GroupMemberEntity
        {
            GroupId = domain.Id,
            UserId = ownerId,
            Role = GroupMemberRole.Admin.ToWire(),
            JoinedAt = DateTime.UtcNow,
        });
        await _db.SaveChangesAsync(cancellationToken);
        await _db.Entry(entity).ReloadAsync(cancellationToken);
        return await ToDto(entity, cancellationToken);
    }

    public async Task<GroupDto?> GetAsync(Guid actorId, Guid groupId, CancellationToken cancellationToken = default)
    {
        var entity = await _db.Groups.AsNoTracking()
            .FirstOrDefaultAsync(g => g.Id == groupId, cancellationToken);
        if (entity is null) return null;
        var isMember = await _db.GroupMembers
            .AsNoTracking()
            .AnyAsync(m => m.GroupId == groupId && m.UserId == actorId, cancellationToken);
        if (!isMember) return null;
        return await ToDto(entity, cancellationToken);
    }

    public async Task<IReadOnlyList<GroupDto>> ListMineAsync(Guid userId, CancellationToken cancellationToken = default)
    {
        var ids = await _db.GroupMembers
            .AsNoTracking()
            .Where(m => m.UserId == userId)
            .Select(m => m.GroupId)
            .ToListAsync(cancellationToken);
        var groups = await _db.Groups
            .AsNoTracking()
            .Where(g => ids.Contains(g.Id))
            .ToListAsync(cancellationToken);
        var dtos = new List<GroupDto>(groups.Count);
        foreach (var g in groups) dtos.Add(await ToDto(g, cancellationToken));
        return dtos;
    }

    public async Task DeleteAsync(Guid actorId, Guid groupId, CancellationToken cancellationToken = default)
    {
        var entity = await _db.Groups
            .FirstOrDefaultAsync(g => g.Id == groupId, cancellationToken);
        if (entity is null) return;
        if (entity.OwnerId != actorId)
            throw new UnauthorizedAccessException("Only the group owner can delete it.");
        _db.Groups.Remove(entity);
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task RenameAsync(Guid actorId, Guid groupId, string name, CancellationToken cancellationToken = default)
    {
        var entity = await _db.Groups
            .FirstOrDefaultAsync(g => g.Id == groupId, cancellationToken)
            ?? throw new InvalidOperationException("Group not found.");
        var domain = Group.Reconstitute(entity.Id, entity.OwnerId, entity.Name, entity.CreatedAt, entity.UpdatedAt);
        domain.Rename(actorId, name);
        entity.Name = domain.Name;
        entity.UpdatedAt = domain.UpdatedAt;
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task AddMemberAsync(Guid actorId, Guid groupId, Guid userId, CancellationToken cancellationToken = default)
    {
        var group = await _db.Groups.AsNoTracking()
            .FirstOrDefaultAsync(g => g.Id == groupId, cancellationToken)
            ?? throw new InvalidOperationException("Group not found.");
        await RequireGroupAdmin(actorId, groupId, group.OwnerId, cancellationToken);

        var exists = await _db.GroupMembers
            .AnyAsync(m => m.GroupId == groupId && m.UserId == userId, cancellationToken);
        if (exists) return;

        _db.GroupMembers.Add(new GroupMemberEntity
        {
            GroupId = groupId,
            UserId = userId,
            Role = GroupMemberRole.Member.ToWire(),
            JoinedAt = DateTime.UtcNow,
        });
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task RemoveMemberAsync(Guid actorId, Guid groupId, Guid userId, CancellationToken cancellationToken = default)
    {
        var group = await _db.Groups.AsNoTracking()
            .FirstOrDefaultAsync(g => g.Id == groupId, cancellationToken)
            ?? throw new InvalidOperationException("Group not found.");
        // A user may always remove themselves. Otherwise the actor must be admin.
        if (actorId != userId)
            await RequireGroupAdmin(actorId, groupId, group.OwnerId, cancellationToken);

        var member = await _db.GroupMembers
            .FirstOrDefaultAsync(m => m.GroupId == groupId && m.UserId == userId, cancellationToken);
        if (member is null) return;
        _db.GroupMembers.Remove(member);
        await _db.SaveChangesAsync(cancellationToken);
    }

    private async Task RequireGroupAdmin(Guid actorId, Guid groupId, Guid ownerId, CancellationToken cancellationToken)
    {
        if (actorId == ownerId) return;
        var actor = await _db.GroupMembers.AsNoTracking()
            .FirstOrDefaultAsync(m => m.GroupId == groupId && m.UserId == actorId, cancellationToken);
        if (actor is null || actor.Role != GroupMemberRole.Admin.ToWire())
            throw new UnauthorizedAccessException("Only a group admin can perform this action.");
    }

    private async Task<GroupDto> ToDto(GroupEntity entity, CancellationToken cancellationToken)
    {
        var members = await _db.GroupMembers
            .AsNoTracking()
            .Where(m => m.GroupId == entity.Id)
            .Select(m => new GroupMemberDto(m.UserId, m.Role, m.JoinedAt))
            .ToListAsync(cancellationToken);
        return new GroupDto(entity.Id, entity.OwnerId, entity.Name, entity.CreatedAt, entity.UpdatedAt, members);
    }
}
