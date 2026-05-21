using Turboapi.Collections.domain.queries;

namespace Turboapi.Collections.controller.response;

public record CollectionResponse
{
    public Guid Id { get; init; }
    public string Name { get; init; } = null!;
    public string? Description { get; init; }
    public string? ColorHex { get; init; }
    public string? IconKey { get; init; }
    public int SortOrder { get; init; }
    public string? SavedFilter { get; init; }
    public IReadOnlyList<ItemRefDto> Items { get; init; } = Array.Empty<ItemRefDto>();

    public DateTime? CreatedAt { get; init; }
    public DateTime? UpdatedAt { get; init; }
    public long? Version { get; init; }

    public static CollectionResponse FromDto(CollectionData data) => new()
    {
        Id = data.Id,
        Name = data.Metadata.Name,
        Description = data.Metadata.Description,
        ColorHex = data.Metadata.ColorHex,
        IconKey = data.Metadata.IconKey,
        SortOrder = data.Metadata.SortOrder,
        SavedFilter = data.Metadata.SavedFilterJson,
        Items = data.Items.Select(i => new ItemRefDto(i.Type, i.Uuid)).ToList(),
        CreatedAt = data.CreatedAt == default ? null : data.CreatedAt,
        UpdatedAt = data.UpdatedAt == default ? null : data.UpdatedAt,
        Version = data.Version == 0 ? null : data.Version,
    };
}

public record ItemRefDto(string Type, string Uuid);

public record CollectionsResponse
{
    public IReadOnlyList<CollectionResponse> Items { get; init; } = Array.Empty<CollectionResponse>();
    public int Count { get; init; }
}

public record CollectionsDeltaResponse
{
    public IReadOnlyList<CollectionResponse> Items { get; init; } = Array.Empty<CollectionResponse>();
    public IReadOnlyList<TombstoneResponse> Deleted { get; init; } = Array.Empty<TombstoneResponse>();
    public string? NextCursor { get; init; }
    public DateTime ServerTime { get; init; }
}

public record TombstoneResponse(Guid Id, DateTime DeletedAt, long Version);

public record ErrorResponse
{
    public string Title { get; init; } = null!;
    public string Detail { get; init; } = null!;
    public string Type { get; init; } = "https://tools.ietf.org/html/rfc7231#section-6.5.1";

    public ErrorResponse(string title, string detail)
    {
        Title = title;
        Detail = detail;
    }
}

public record ConflictResponse(string Title, string Detail, long CurrentVersion, CollectionResponse? Current);
