using Turboapi.Geo.controller.request;
using Turboapi.Geo.domain.queries;

namespace Turboapi.Geo.controller.response;

public record LocationResponse
{
    public Guid Id { get; init; }
    public GeometryData Geometry { get; init; } = null!;
    public DisplayData Display { get; init; } = null!;

    public DateTime? CreatedAt { get; init; }
    public DateTime? UpdatedAt { get; init; }
    public DateTime? DeletedAt { get; init; }
    public long? Version { get; init; }

    public static LocationResponse FromDto(LocationData data) => new()
    {
        Id = data.id,
        Geometry = new GeometryData
        {
            Longitude = data.geometry.Longitude,
            Latitude = data.geometry.Latitude,
        },
        Display = new DisplayData
        {
            Name = data.displayInformation.Name,
            Description = data.displayInformation.Description,
            Icon = data.displayInformation.Icon
        },
        CreatedAt = data.createdAt,
        UpdatedAt = data.updatedAt,
        DeletedAt = data.deletedAt,
        Version = data.version,
    };
}

public record LocationsResponse
{
    public IReadOnlyList<LocationResponse> Items { get; init; } = null!;
    public int Count { get; init; }
    public MetaData Meta { get; init; } = new MetaData();
}

public record LocationsDeltaResponse
{
    public IReadOnlyList<LocationResponse> Items { get; init; } = Array.Empty<LocationResponse>();
    public IReadOnlyList<TombstoneResponse> Deleted { get; init; } = Array.Empty<TombstoneResponse>();
    public string? NextCursor { get; init; }
    public DateTime ServerTime { get; init; }
}

public record TombstoneResponse(Guid Id, DateTime DeletedAt, long Version);

public record MetaData
{
    public string? NextCursor { get; init; }
    public int TotalCount { get; init; }
}

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

public record ConflictResponse(string Title, string Detail, long CurrentVersion, LocationResponse? Current);
