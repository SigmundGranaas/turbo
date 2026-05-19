using System.Diagnostics;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using Turboapi.Collections.data.model;
using Turboapi.Collections.domain.query;
using Turboapi.Collections.domain.value;
using Collection = Turboapi.Collections.domain.model.Collection;

namespace Turboapi.Collections.data;

public class EfCollectionWriteRepository : ICollectionWriteRepository
{
    private readonly CollectionsReadContext _context;
    private readonly ILogger<EfCollectionWriteRepository> _logger;

    public EfCollectionWriteRepository(
        CollectionsReadContext context,
        ILogger<EfCollectionWriteRepository> logger)
    {
        _context = context;
        _logger = logger;
    }

    public async Task<CollectionEntity?> GetById(Guid id)
    {
        return await _context.Collections
            .Include(c => c.Items)
            .FirstOrDefaultAsync(c => c.Id == id);
    }

    public async Task Add(CollectionEntity entity)
    {
        _context.Collections.Add(entity);
        await _context.SaveChangesAsync();
        _logger.LogInformation("Added collection {CollectionId}", entity.Id);
    }

    public async Task UpdateMetadata(Guid id, CollectionEntity updated, DateTime updatedAt)
    {
        await _context.Collections
            .Where(c => c.Id == id)
            .ExecuteUpdateAsync(setters => setters
                .SetProperty(c => c.Name, updated.Name)
                .SetProperty(c => c.Description, updated.Description)
                .SetProperty(c => c.ColorHex, updated.ColorHex)
                .SetProperty(c => c.IconKey, updated.IconKey)
                .SetProperty(c => c.SortOrder, updated.SortOrder)
                .SetProperty(c => c.SavedFilterJson, updated.SavedFilterJson)
                .SetProperty(c => c.UpdatedAt, updatedAt)
                .SetProperty(c => c.Version, c => c.Version + 1));
    }

    public async Task AddItem(Guid collectionId, string itemType, string itemUuid, DateTime updatedAt)
    {
        // ON CONFLICT DO NOTHING semantics — composite PK collision is
        // treated as idempotent.
        var existing = await _context.CollectionItems
            .FirstOrDefaultAsync(i => i.CollectionId == collectionId
                                   && i.ItemType == itemType
                                   && i.ItemUuid == itemUuid);
        if (existing is null)
        {
            _context.CollectionItems.Add(new CollectionItemEntity
            {
                CollectionId = collectionId,
                ItemType = itemType,
                ItemUuid = itemUuid,
                AddedAt = updatedAt,
            });
        }

        await _context.Collections
            .Where(c => c.Id == collectionId)
            .ExecuteUpdateAsync(setters => setters
                .SetProperty(c => c.UpdatedAt, updatedAt)
                .SetProperty(c => c.Version, c => c.Version + 1));

        await _context.SaveChangesAsync();
    }

    public async Task RemoveItem(Guid collectionId, string itemType, string itemUuid, DateTime updatedAt)
    {
        await _context.CollectionItems
            .Where(i => i.CollectionId == collectionId
                     && i.ItemType == itemType
                     && i.ItemUuid == itemUuid)
            .ExecuteDeleteAsync();

        await _context.Collections
            .Where(c => c.Id == collectionId)
            .ExecuteUpdateAsync(setters => setters
                .SetProperty(c => c.UpdatedAt, updatedAt)
                .SetProperty(c => c.Version, c => c.Version + 1));
    }

    public async Task SoftDelete(Guid id, DateTime deletedAt)
    {
        await _context.Collections
            .Where(c => c.Id == id)
            .ExecuteUpdateAsync(setters => setters
                .SetProperty(c => c.DeletedAt, deletedAt)
                .SetProperty(c => c.UpdatedAt, deletedAt)
                .SetProperty(c => c.Version, c => c.Version + 1));
        // Items rows for a tombstoned collection are deliberately kept
        // around so the read endpoint can still surface the last known
        // state on conflict resolution. The HasMany ON DELETE CASCADE
        // only fires when the row itself is physically removed.
    }

    public class EfCollectionReadRepository : ICollectionReadRepository
    {
        private readonly CollectionsReadContext _context;

        public EfCollectionReadRepository(CollectionsReadContext context) => _context = context;

        public async Task<Collection?> GetById(Guid id)
        {
            var entity = await _context.Collections
                .Include(c => c.Items)
                .AsNoTracking()
                .FirstOrDefaultAsync(c => c.Id == id);
            if (entity is null || entity.DeletedAt is not null) return null;
            return Reconstitute(entity);
        }

        public async Task<CollectionEntity?> GetEntityById(Guid id)
            => await _context.Collections
                .Include(c => c.Items)
                .AsNoTracking()
                .FirstOrDefaultAsync(c => c.Id == id);

        public async Task<IEnumerable<Collection>> GetUserCollections(Guid ownerId, int? limit = null)
        {
            IQueryable<CollectionEntity> q = _context.Collections
                .Include(c => c.Items)
                .AsNoTracking()
                .Where(c => c.OwnerId == ownerId)
                .Where(c => c.DeletedAt == null)
                .OrderBy(c => c.SortOrder)
                .ThenBy(c => c.CreatedAt);
            if (limit is { } n) q = q.Take(n);
            var entities = await q.ToListAsync();
            return entities.Select(Reconstitute);
        }

        public async Task<IEnumerable<CollectionEntity>> GetChangedSince(Guid ownerId, DateTime since, int limit)
        {
            var sinceUtc = DateTime.SpecifyKind(since.ToUniversalTime(), DateTimeKind.Utc);
            return await _context.Collections
                .Include(c => c.Items)
                .AsNoTracking()
                .Where(c => c.OwnerId == ownerId)
                .Where(c => c.UpdatedAt > sinceUtc)
                .OrderBy(c => c.UpdatedAt)
                .Take(limit)
                .ToListAsync();
        }

        public async Task<DateTime?> GetCurrentServerTime()
        {
            var conn = _context.Database.GetDbConnection();
            var wasOpen = conn.State == System.Data.ConnectionState.Open;
            if (!wasOpen) await conn.OpenAsync();
            try
            {
                await using var cmd = conn.CreateCommand();
                cmd.CommandText = "SELECT CURRENT_TIMESTAMP AT TIME ZONE 'UTC'";
                var result = await cmd.ExecuteScalarAsync();
                if (result is DateTime dt) return DateTime.SpecifyKind(dt, DateTimeKind.Utc);
                return null;
            }
            finally
            {
                if (!wasOpen) await conn.CloseAsync();
            }
        }

        private static Collection Reconstitute(CollectionEntity e)
        {
            var metadata = new CollectionMetadata(
                e.Name, e.Description, e.ColorHex, e.IconKey, e.SortOrder, e.SavedFilterJson);
            var items = e.Items.Select(i => new CollectionItemRef(i.ItemType, i.ItemUuid));
            return Collection.Reconstitute(e.Id, e.OwnerId, metadata, items);
        }
    }
}
