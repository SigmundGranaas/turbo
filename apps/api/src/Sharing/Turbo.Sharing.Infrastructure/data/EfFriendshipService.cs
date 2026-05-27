using Microsoft.EntityFrameworkCore;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.domain.exception;
using Turboapi.Sharing.domain.model;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.data;

public sealed class EfFriendshipService : IFriendshipService
{
    private readonly SharingReadContext _db;

    public EfFriendshipService(SharingReadContext db) => _db = db;

    public async Task<FriendshipDto> RequestAsync(Guid initiatorId, Guid otherUserId, CancellationToken cancellationToken = default)
    {
        var (lower, higher) = Friendship.Canonicalize(initiatorId, otherUserId);
        var existing = await _db.Friendships
            .FirstOrDefaultAsync(f => f.LowerUserId == lower && f.HigherUserId == higher, cancellationToken);
        if (existing is not null)
            throw new FriendshipAlreadyExistsException(initiatorId, otherUserId);

        var domain = Friendship.Request(initiatorId, otherUserId);
        var entity = new FriendshipEntity
        {
            LowerUserId = domain.LowerUserId,
            HigherUserId = domain.HigherUserId,
            InitiatorId = domain.InitiatorId,
            Status = domain.Status.ToWire(),
            AcceptedAt = domain.AcceptedAt,
        };
        _db.Friendships.Add(entity);
        await _db.SaveChangesAsync(cancellationToken);
        await _db.Entry(entity).ReloadAsync(cancellationToken);
        return ToDto(entity, viewer: initiatorId);
    }

    public async Task<FriendshipDto> AcceptAsync(Guid acceptingUserId, Guid otherUserId, CancellationToken cancellationToken = default)
    {
        var (lower, higher) = Friendship.Canonicalize(acceptingUserId, otherUserId);
        var entity = await _db.Friendships
            .FirstOrDefaultAsync(f => f.LowerUserId == lower && f.HigherUserId == higher, cancellationToken)
            ?? throw new InvalidOperationException("Friendship request not found.");

        var domain = Friendship.Reconstitute(
            entity.LowerUserId, entity.HigherUserId, entity.InitiatorId,
            FriendshipStatusExtensions.ParseFriendshipStatus(entity.Status),
            entity.CreatedAt, entity.AcceptedAt);
        domain.Accept(acceptingUserId);

        entity.Status = domain.Status.ToWire();
        entity.AcceptedAt = domain.AcceptedAt;
        await _db.SaveChangesAsync(cancellationToken);
        return ToDto(entity, viewer: acceptingUserId);
    }

    public async Task BlockAsync(Guid blockingUserId, Guid otherUserId, CancellationToken cancellationToken = default)
    {
        var (lower, higher) = Friendship.Canonicalize(blockingUserId, otherUserId);
        var entity = await _db.Friendships
            .FirstOrDefaultAsync(f => f.LowerUserId == lower && f.HigherUserId == higher, cancellationToken)
            ?? new FriendshipEntity
            {
                LowerUserId = lower,
                HigherUserId = higher,
                InitiatorId = blockingUserId,
                Status = FriendshipStatus.Blocked.ToWire(),
            };

        entity.Status = FriendshipStatus.Blocked.ToWire();
        if (entity.LowerUserId == default)
            _db.Friendships.Add(entity);

        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task RemoveAsync(Guid userId, Guid otherUserId, CancellationToken cancellationToken = default)
    {
        var (lower, higher) = Friendship.Canonicalize(userId, otherUserId);
        var entity = await _db.Friendships
            .FirstOrDefaultAsync(f => f.LowerUserId == lower && f.HigherUserId == higher, cancellationToken);
        if (entity is null) return;
        _db.Friendships.Remove(entity);
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task<IReadOnlyList<FriendshipDto>> ListAsync(Guid userId, FriendshipStatus? status, CancellationToken cancellationToken = default)
    {
        var query = _db.Friendships
            .AsNoTracking()
            .Where(f => f.LowerUserId == userId || f.HigherUserId == userId);
        if (status is not null)
            query = query.Where(f => f.Status == status.Value.ToWire());
        var rows = await query.ToListAsync(cancellationToken);
        return rows.Select(r => ToDto(r, viewer: userId)).ToList();
    }

    private static FriendshipDto ToDto(FriendshipEntity entity, Guid viewer)
    {
        var other = viewer == entity.LowerUserId ? entity.HigherUserId : entity.LowerUserId;
        return new FriendshipDto(other, entity.InitiatorId, entity.Status, entity.CreatedAt, entity.AcceptedAt);
    }
}
