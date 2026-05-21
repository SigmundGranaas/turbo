using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.controller.request;

public record CreateTrackRequest
{
    public GeometryDto Geometry { get; init; } = null!;
    public MetadataDto Metadata { get; init; } = null!;
    public StatsDto Stats { get; init; } = null!;
}

public record UpdateTrackRequest
{
    public GeometryDto? Geometry { get; init; }
    public MetadataChangesetDto? Metadata { get; init; }
    public StatsDto? Stats { get; init; }
}

public record GeometryDto
{
    public List<PointDto> Points { get; init; } = new();
    public List<double>? Elevations { get; init; }

    public TrackGeometry ToValueObject()
    {
        var points = Points
            .Select(p => new GeoPoint(p.Longitude, p.Latitude))
            .ToList();
        return new TrackGeometry(points, Elevations);
    }
}

public record PointDto
{
    public double Longitude { get; init; }
    public double Latitude { get; init; }
}

public record MetadataDto
{
    public string Name { get; init; } = null!;
    public string? Description { get; init; }
    public string? ColorHex { get; init; }
    public string? IconKey { get; init; }
    public string? LineStyleKey { get; init; }
    public bool Smoothing { get; init; }

    public TrackMetadata ToValueObject() =>
        new(Name, Description, ColorHex, IconKey, LineStyleKey, Smoothing);
}

public record MetadataChangesetDto
{
    public string? Name { get; init; }
    public string? Description { get; init; }
    public string? ColorHex { get; init; }
    public string? IconKey { get; init; }
    public string? LineStyleKey { get; init; }
    public bool? Smoothing { get; init; }

    public TrackMetadataUpdate ToValueObject() =>
        new(Name, Description, ColorHex, IconKey, LineStyleKey, Smoothing);
}

public record StatsDto
{
    public double DistanceMeters { get; init; }
    public double? AscentMeters { get; init; }
    public double? DescentMeters { get; init; }
    public int? MovingTimeSeconds { get; init; }
    public DateTime? RecordedAt { get; init; }

    public TrackStats ToValueObject() =>
        new(DistanceMeters, AscentMeters, DescentMeters, MovingTimeSeconds, RecordedAt);
}
