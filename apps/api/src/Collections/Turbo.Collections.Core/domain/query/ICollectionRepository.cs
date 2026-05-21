using Turboapi.Collections.data.model;
using Turboapi.Collections.domain.model;

namespace Turboapi.Collections.domain.query;

public interface ICollectionReadRepository
{
    Task<Collection?> GetById(Guid id);

    /// <summary>
    /// Raw read-model row (including sync fields). Used by the
    /// controller to set <c>ETag</c> headers and to run the
    /// <c>If-Match</c> optimistic-concurrency check.
    /// </summary>
    Task<CollectionEntity?> GetEntityById(Guid id);

    Task<IEnumerable<Collection>> GetUserCollections(Guid ownerId, int? limit = null);

    /// <summary>
    /// Read-model rows whose <c>UpdatedAt</c> is strictly greater than
    /// <paramref name="since"/>. Includes tombstoned rows so the client
    /// can learn about deletions on its next pull.
    /// </summary>
    Task<IEnumerable<CollectionEntity>> GetChangedSince(Guid ownerId, DateTime since, int limit);

    Task<DateTime?> GetCurrentServerTime();
}

public interface ICollectionWriteRepository
{
    Task<CollectionEntity?> GetById(Guid id);
    Task Add(CollectionEntity entity);
    Task UpdateMetadata(Guid id, CollectionEntity updated, DateTime updatedAt);
    Task AddItem(Guid collectionId, string itemType, string itemUuid, DateTime updatedAt);
    Task RemoveItem(Guid collectionId, string itemType, string itemUuid, DateTime updatedAt);
    Task SoftDelete(Guid id, DateTime deletedAt);
}
