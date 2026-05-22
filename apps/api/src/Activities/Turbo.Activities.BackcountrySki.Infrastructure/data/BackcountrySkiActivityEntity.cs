using NetTopologySuite.Geometries;

namespace Turboapi.Activities.BackcountrySki.data.model;

/// <summary>
/// Typed read-model entity. The single source of truth for a backcountry
/// ski activity. Every kind-specific field is its own column; aspect mix
/// and route legs are owned-collection tables.
/// </summary>
public class BackcountrySkiActivityEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }
    public required string Name { get; set; }
    public string? Description { get; set; }

    /// <summary>PostGIS LINESTRING in EPSG:4326.</summary>
    public required LineString Route { get; set; }

    public int AscentMeters { get; set; }
    public int DescentMeters { get; set; }
    public int DistanceMeters { get; set; }
    public int ElevationMinMeters { get; set; }
    public int ElevationMaxMeters { get; set; }

    public short AtesRating { get; set; }
    public short? DominantAspect { get; set; }
    public int? VarsomRegionId { get; set; }
    public short? PreferredAvalancheMaxLevel { get; set; }

    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }

    public List<AspectShareEntity> AspectMix { get; set; } = new();
    public List<RouteLegEntity> Legs { get; set; } = new();
}

public class AspectShareEntity
{
    public Guid ActivityId { get; set; }
    public short Aspect { get; set; }
    public float Fraction { get; set; }
}

public class RouteLegEntity
{
    public Guid ActivityId { get; set; }
    public int Ordinal { get; set; }
    public short LegKind { get; set; }
    public int StartElevationMeters { get; set; }
    public int EndElevationMeters { get; set; }
    public required LineString Geometry { get; set; }
}
