using NetTopologySuite.Geometries;

namespace Turboapi.Activities.Fishing.data.model;

/// <summary>
/// Typed read-model entity for a fishing activity. Lives in the
/// <c>fishing.activities</c> table — every field is a typed column, never
/// a JSONB blob. Owned-collection tables for target species and depth
/// samples enforce the same rule (no <c>species text[]</c> shortcut).
/// </summary>
public class FishingActivityEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }

    public required string Name { get; set; }
    public string? Description { get; set; }

    /// <summary>PostGIS POINT in EPSG:4326.</summary>
    public required Point Geometry { get; set; }

    public short WaterKind { get; set; }
    public short ShoreOrBoat { get; set; }
    public string? AccessNotes { get; set; }

    public short? PreferredPressureMinHpa { get; set; }
    public short? PreferredPressureMaxHpa { get; set; }
    public float? PreferredWindMaxMs { get; set; }

    // Sync
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }

    // Owned collections — typed, never JSON.
    public List<TargetSpeciesEntity> TargetSpecies { get; set; } = new();
    public List<DepthSampleEntity> DepthSamples { get; set; } = new();
}

public class TargetSpeciesEntity
{
    public Guid ActivityId { get; set; }
    public required string SpeciesCode { get; set; }
    public string? Notes { get; set; }
}

public class DepthSampleEntity
{
    public Guid ActivityId { get; set; }
    public int Ordinal { get; set; }
    public double Lat { get; set; }
    public double Lon { get; set; }
    public float DepthMeters { get; set; }
}
