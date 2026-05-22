using NetTopologySuite.Geometries;

namespace Turboapi.Activities.Hiking.data.model;

public class HikingActivityEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }
    public required string Name { get; set; }
    public string? Description { get; set; }
    public required LineString Route { get; set; }

    public int DistanceMeters { get; set; }
    public int AscentMeters { get; set; }
    public int DescentMeters { get; set; }
    public int ElevationMinMeters { get; set; }
    public int ElevationMaxMeters { get; set; }

    public short Difficulty { get; set; }
    public short Surface { get; set; }
    public short Marking { get; set; }
    public float? EstimatedHours { get; set; }
    public bool HasWaterSources { get; set; }
    public bool HasShelter { get; set; }

    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }

    public List<WaterSourceEntity> WaterSources { get; set; } = new();
}

public class WaterSourceEntity
{
    public Guid ActivityId { get; set; }
    public int Ordinal { get; set; }
    public double Lat { get; set; }
    public double Lon { get; set; }
    public required string Kind { get; set; }
    public string? Notes { get; set; }
}
