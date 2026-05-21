using NetTopologySuite.Geometries;

namespace Turboapi.Activities.Packrafting.data.model;

public class PackraftingActivityEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }
    public required string Name { get; set; }
    public string? Description { get; set; }
    public required LineString Route { get; set; }

    public int DistanceMeters { get; set; }
    public int PaddleDistanceMeters { get; set; }
    public int PortageDistanceMeters { get; set; }
    public short MaxGrade { get; set; }
    public short TypicalGrade { get; set; }
    public double PutInLat { get; set; }
    public double PutInLon { get; set; }
    public double TakeOutLat { get; set; }
    public double TakeOutLon { get; set; }
    public string? NveStationCode { get; set; }
    public float? MinFlowCumecs { get; set; }
    public float? MaxFlowCumecs { get; set; }

    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }

    public List<RouteSegmentEntity> Segments { get; set; } = new();
}

public class RouteSegmentEntity
{
    public Guid ActivityId { get; set; }
    public int Ordinal { get; set; }
    public short Kind { get; set; }
    public short? Grade { get; set; }
    public int DistanceMeters { get; set; }
    public required LineString Geometry { get; set; }
    public string? Notes { get; set; }
}
