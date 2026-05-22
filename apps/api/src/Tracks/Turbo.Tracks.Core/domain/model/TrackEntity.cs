using NetTopologySuite.Geometries;

namespace Turboapi.Tracks.data.model;

/// <summary>
/// EF Core read-model entity for a Track. Stored in the
/// <c>tracks_read</c> table. The sync fields (<c>UpdatedAt</c>,
/// <c>DeletedAt</c>, <c>Version</c>) are server-attested; the projection
/// stamps them on every event.
/// </summary>
public class TrackEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }

    /// <summary>PostGIS LINESTRING in EPSG:4326.</summary>
    public required LineString Geometry { get; set; }

    /// <summary>Optional per-vertex elevations. Length matches geometry's points.</summary>
    public double[]? Elevations { get; set; }

    // Metadata
    public required string Name { get; set; }
    public string? Description { get; set; }
    public string? ColorHex { get; set; }
    public string? IconKey { get; set; }
    public string? LineStyleKey { get; set; }
    public bool Smoothing { get; set; }

    // Stats
    public double DistanceMeters { get; set; }
    public double? AscentMeters { get; set; }
    public double? DescentMeters { get; set; }
    public int? MovingTimeSeconds { get; set; }
    public DateTime? RecordedAt { get; set; }

    // Sync
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }
}
