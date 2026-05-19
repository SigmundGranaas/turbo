using Turboapi.Tracks.controller.request;
using Turboapi.Tracks.domain.queries;
using Turboapi.Tracks.domain.query;
using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.controller.response;

public record TrackResponse
{
    public Guid Id { get; init; }
    public GeometryDto Geometry { get; init; } = null!;
    public MetadataDto Metadata { get; init; } = null!;
    public StatsDto Stats { get; init; } = null!;

    public DateTime? CreatedAt { get; init; }
    public DateTime? UpdatedAt { get; init; }
    public long? Version { get; init; }

    public static TrackResponse FromDto(TrackData data) => new()
    {
        Id = data.Id,
        Geometry = new GeometryDto
        {
            Points = data.Geometry.Points.Select(p => new PointDto { Longitude = p.Longitude, Latitude = p.Latitude }).ToList(),
            Elevations = data.Geometry.Elevations?.ToList(),
        },
        Metadata = new MetadataDto
        {
            Name = data.Metadata.Name,
            Description = data.Metadata.Description,
            ColorHex = data.Metadata.ColorHex,
            IconKey = data.Metadata.IconKey,
            LineStyleKey = data.Metadata.LineStyleKey,
            Smoothing = data.Metadata.Smoothing,
        },
        Stats = new StatsDto
        {
            DistanceMeters = data.Stats.DistanceMeters,
            AscentMeters = data.Stats.AscentMeters,
            DescentMeters = data.Stats.DescentMeters,
            MovingTimeSeconds = data.Stats.MovingTimeSeconds,
            RecordedAt = data.Stats.RecordedAt,
        },
        CreatedAt = data.CreatedAt == default ? null : data.CreatedAt,
        UpdatedAt = data.UpdatedAt == default ? null : data.UpdatedAt,
        Version = data.Version == 0 ? null : data.Version,
    };
}

public record TracksResponse
{
    public IReadOnlyList<TrackResponse> Items { get; init; } = Array.Empty<TrackResponse>();
    public int Count { get; init; }
}

public record TracksDeltaResponse
{
    public IReadOnlyList<TrackResponse> Items { get; init; } = Array.Empty<TrackResponse>();
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

public record ConflictResponse(string Title, string Detail, long CurrentVersion, TrackResponse Current);
