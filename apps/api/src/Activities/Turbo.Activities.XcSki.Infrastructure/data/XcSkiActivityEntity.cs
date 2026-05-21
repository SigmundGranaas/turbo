using NetTopologySuite.Geometries;

namespace Turboapi.Activities.XcSki.data.model;

public class XcSkiActivityEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }
    public required string Name { get; set; }
    public string? Description { get; set; }
    public required LineString Route { get; set; }

    public int DistanceMeters { get; set; }
    public int AscentMeters { get; set; }
    public int DescentMeters { get; set; }
    public short Technique { get; set; }
    public short GroomingStatus { get; set; }
    public bool IsLit { get; set; }
    public bool RequiresSeasonPass { get; set; }
    public string? GroomingFeedKey { get; set; }

    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }
}
