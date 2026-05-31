using Microsoft.EntityFrameworkCore;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.domain.exception;
using Turboapi.Sharing.domain.model;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.data;

public sealed class EfShareInviteService : IShareInviteService
{
    private readonly SharingReadContext _db;

    public EfShareInviteService(SharingReadContext db) => _db = db;

    public async Task<InviteDto> CreateFriendInviteAsync(Guid inviterId, string inviteeEmail, TimeSpan? lifetime, CancellationToken cancellationToken = default)
    {
        var domain = ShareInvite.ForFriendship(inviterId, inviteeEmail, lifetime);
        return await InsertAsync(domain, cancellationToken);
    }

    public async Task<InviteDto> CreateResourceInviteAsync(Guid inviterId, string inviteeEmail, Guid resourceId, Role role, TimeSpan? lifetime, CancellationToken cancellationToken = default)
    {
        var owner = await _db.Resources.AsNoTracking()
            .Where(r => r.Id == resourceId && r.DeletedAt == null)
            .Select(r => (Guid?)r.OwnerId)
            .FirstOrDefaultAsync(cancellationToken)
            ?? throw new ResourceNotFoundException(resourceId);
        if (owner != inviterId)
            throw new AccessDeniedException(inviterId, resourceId);

        var domain = ShareInvite.ForResource(inviterId, inviteeEmail, resourceId, role, lifetime);
        return await InsertAsync(domain, cancellationToken);
    }

    public async Task<IReadOnlyList<InviteDto>> ListPendingForEmailAsync(string email, CancellationToken cancellationToken = default)
    {
        var normalized = email.Trim().ToLowerInvariant();
        var rows = await _db.ShareInvites
            .AsNoTracking()
            .Where(i => i.InviteeEmail == normalized && i.RedeemedAt == null)
            .ToListAsync(cancellationToken);
        return rows.Select(ToDto).ToList();
    }

    public async Task<IReadOnlyList<InviteDto>> ListMineAsync(Guid inviterId, CancellationToken cancellationToken = default)
    {
        var rows = await _db.ShareInvites
            .AsNoTracking()
            .Where(i => i.InviterId == inviterId)
            .ToListAsync(cancellationToken);
        return rows.Select(ToDto).ToList();
    }

    public async Task<int> RedeemAllForUserAsync(Guid userId, string email, CancellationToken cancellationToken = default)
    {
        var normalized = email.Trim().ToLowerInvariant();
        var pending = await _db.ShareInvites
            .Where(i => i.InviteeEmail == normalized && i.RedeemedAt == null)
            .ToListAsync(cancellationToken);

        var now = DateTime.UtcNow;
        var redeemed = 0;
        foreach (var entity in pending)
        {
            if (entity.ExpiresAt is not null && entity.ExpiresAt < now) continue;
            if (entity.InviterId == userId) continue;       // safety: don't grant to self

            entity.RedeemedAt = now;
            entity.RedeemedByUserId = userId;

            if (entity.ResourceId is { } rid && entity.Role is { } roleWire)
            {
                // Materialize into a user grant. Upsert if one already exists.
                var subjectType = SubjectType.User.ToWire();
                var existing = await _db.Grants.FirstOrDefaultAsync(g =>
                    g.ResourceId == rid && g.SubjectType == subjectType && g.SubjectId == userId,
                    cancellationToken);
                if (existing is null)
                {
                    _db.Grants.Add(new GrantEntity
                    {
                        ResourceId = rid,
                        SubjectType = subjectType,
                        SubjectId = userId,
                        Role = roleWire,
                        GrantedBy = entity.InviterId,
                        GrantedAt = now,
                    });
                }
                else
                {
                    // Promote role if the invite is stronger than the current grant.
                    if (RoleExtensions.ParseRole(roleWire) == Role.Editor)
                        existing.Role = roleWire;
                }
            }

            // Materialize friendship as accepted, in canonical order.
            await UpsertFriendshipAsync(entity.InviterId, userId, cancellationToken);
            redeemed++;
        }
        await _db.SaveChangesAsync(cancellationToken);
        return redeemed;
    }

    private async Task UpsertFriendshipAsync(Guid inviter, Guid invitee, CancellationToken cancellationToken)
    {
        if (inviter == invitee) return;
        var (lower, higher) = Friendship.Canonicalize(inviter, invitee);
        var existing = await _db.Friendships
            .FirstOrDefaultAsync(f => f.LowerUserId == lower && f.HigherUserId == higher, cancellationToken);
        var now = DateTime.UtcNow;
        if (existing is null)
        {
            _db.Friendships.Add(new FriendshipEntity
            {
                LowerUserId = lower,
                HigherUserId = higher,
                InitiatorId = inviter,
                Status = FriendshipStatus.Accepted.ToWire(),
                AcceptedAt = now,
            });
        }
        else if (existing.Status != FriendshipStatus.Accepted.ToWire())
        {
            existing.Status = FriendshipStatus.Accepted.ToWire();
            existing.AcceptedAt = now;
        }
    }

    private async Task<InviteDto> InsertAsync(ShareInvite domain, CancellationToken cancellationToken)
    {
        var entity = new ShareInviteEntity
        {
            Id = domain.Id,
            InviterId = domain.InviterId,
            InviteeEmail = domain.InviteeEmail,
            ResourceId = domain.ResourceId,
            Role = domain.Role?.ToWire(),
            ExpiresAt = domain.ExpiresAt,
        };
        _db.ShareInvites.Add(entity);
        await _db.SaveChangesAsync(cancellationToken);
        await _db.Entry(entity).ReloadAsync(cancellationToken);
        return ToDto(entity);
    }

    private static InviteDto ToDto(ShareInviteEntity entity)
        => new(entity.Id, entity.InviterId, entity.InviteeEmail, entity.ResourceId,
               entity.Role, entity.CreatedAt, entity.ExpiresAt);
}
