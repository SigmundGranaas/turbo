using Turboapi.Collections.domain.value;

namespace Turboapi.Collections.controller.request;

public record CreateCollectionRequest
{
    public string Name { get; init; } = null!;
    public string? Description { get; init; }
    public string? ColorHex { get; init; }
    public string? IconKey { get; init; }
    public int SortOrder { get; init; }
    public string? SavedFilter { get; init; }

    public CollectionMetadata ToValueObject() =>
        new(Name, Description, ColorHex, IconKey, SortOrder, SavedFilter);
}

public record UpdateCollectionRequest
{
    public string? Name { get; init; }
    public string? Description { get; init; }
    public string? ColorHex { get; init; }
    public string? IconKey { get; init; }
    public int? SortOrder { get; init; }
    public string? SavedFilter { get; init; }
    public bool ClearSavedFilter { get; init; }

    public CollectionMetadataUpdate ToValueObject() =>
        new(Name, Description, ColorHex, IconKey, SortOrder, SavedFilter, ClearSavedFilter);
}

public record AddItemRequest
{
    public string Type { get; init; } = null!;
    public string Uuid { get; init; } = null!;
}
