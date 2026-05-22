namespace Turboapi.Collections.data.model;

/// <summary>
/// EF Core read-model entity for a Collection. The items live in a
/// sibling table (<see cref="CollectionItemEntity"/>) joined by
/// <see cref="Id"/>; the read repository loads them together.
/// </summary>
public class CollectionEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }

    public required string Name { get; set; }
    public string? Description { get; set; }
    public string? ColorHex { get; set; }
    public string? IconKey { get; set; }
    public int SortOrder { get; set; }
    public string? SavedFilterJson { get; set; }

    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }

    public List<CollectionItemEntity> Items { get; set; } = new();
}

/// <summary>
/// Polymorphic membership row. Composite primary key is
/// (collection_id, item_type, item_uuid) — same shape as the Flutter
/// client's local schema.
/// </summary>
public class CollectionItemEntity
{
    public required Guid CollectionId { get; set; }
    public required string ItemType { get; set; }
    public required string ItemUuid { get; set; }
    public DateTime AddedAt { get; set; }
}
