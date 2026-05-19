using Turboapi.Collections.data.model;
using Turboapi.Collections.domain.queries;
using Turboapi.Collections.domain.value;

namespace Turboapi.Collections.domain.query;

public class GetCollectionByIdHandler
{
    private readonly ICollectionReadRepository _read;
    public GetCollectionByIdHandler(ICollectionReadRepository read) => _read = read;

    public async Task<CollectionData?> Handle(GetCollectionByIdQuery query)
    {
        var entity = await _read.GetEntityById(query.CollectionId);
        if (entity is null) return null;
        if (entity.OwnerId != query.Owner) return null;
        if (entity.DeletedAt is not null) return null;
        return entity.ToData();
    }
}

public class GetUserCollectionsHandler
{
    private readonly ICollectionReadRepository _read;
    public GetUserCollectionsHandler(ICollectionReadRepository read) => _read = read;

    public async Task<IEnumerable<CollectionData>> Handle(GetUserCollectionsQuery query)
    {
        var collections = await _read.GetUserCollections(query.Owner, query.Limit);
        return collections.Select(c => new CollectionData(
            c.Id, c.OwnerId, c.Metadata,
            c.Items.ToList(),
            CreatedAt: default, UpdatedAt: default, DeletedAt: null, Version: 0));
    }
}

public class GetCollectionsChangedSinceHandler
{
    private readonly ICollectionReadRepository _read;
    public GetCollectionsChangedSinceHandler(ICollectionReadRepository read) => _read = read;

    public async Task<CollectionDeltaResult> Handle(GetCollectionsChangedSinceQuery query)
    {
        var rows = (await _read.GetChangedSince(query.Owner, query.Since, query.Limit)).ToList();
        var items = rows.Where(r => r.DeletedAt is null).Select(r => r.ToData()).ToList();
        var deleted = rows
            .Where(r => r.DeletedAt is not null)
            .Select(r => new CollectionTombstoneData(r.Id, r.DeletedAt!.Value, r.Version))
            .ToList();
        var serverTime = await _read.GetCurrentServerTime() ?? DateTime.UtcNow;
        return new CollectionDeltaResult(items, deleted, serverTime);
    }
}

internal static class CollectionEntityMapper
{
    public static CollectionData ToData(this CollectionEntity e)
    {
        var metadata = new CollectionMetadata(
            e.Name, e.Description, e.ColorHex, e.IconKey, e.SortOrder, e.SavedFilterJson);
        var items = e.Items
            .Select(i => new CollectionItemRef(i.ItemType, i.ItemUuid))
            .ToList();
        return new CollectionData(
            e.Id, e.OwnerId, metadata, items,
            e.CreatedAt, e.UpdatedAt, e.DeletedAt, e.Version);
    }
}
