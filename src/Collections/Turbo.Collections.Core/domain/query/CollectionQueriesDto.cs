using Turboapi.Collections.domain.value;

namespace Turboapi.Collections.domain.queries;

public record GetCollectionByIdQuery(Guid CollectionId, Guid Owner);

public record GetUserCollectionsQuery(Guid Owner, int? Limit = null);

public record GetCollectionsChangedSinceQuery(Guid Owner, DateTime Since, int Limit = 500);

public record CollectionData(
    Guid Id,
    Guid OwnerId,
    CollectionMetadata Metadata,
    IReadOnlyList<CollectionItemRef> Items,
    DateTime CreatedAt,
    DateTime UpdatedAt,
    DateTime? DeletedAt,
    long Version);

public record CollectionTombstoneData(Guid Id, DateTime DeletedAt, long Version);

public record CollectionDeltaResult(
    IReadOnlyList<CollectionData> Items,
    IReadOnlyList<CollectionTombstoneData> Deleted,
    DateTime ServerTime);
