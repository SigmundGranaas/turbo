using Turboapi.Geo.controller.request;
using Turboapi.Geo.domain.queries;

namespace Turboapi.Geo.controller.response;

public record LocationResponse
{
    public Guid Id { get; init; }
    public GeometryData Geometry { get; init; } = null!;
    public DisplayData Display { get; init; } = null!;

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
        }
    };
}

// Response model for multiple locations
public record LocationsResponse
{
    public IReadOnlyList<LocationResponse> Items { get; init; } = null!;
    public int Count { get; init; }
    public MetaData Meta { get; init; } = new MetaData();
}

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
