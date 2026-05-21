using Turboapi.Activities.Hiking.value;

namespace Turboapi.Activities.Hiking.controller.request;

public sealed class CreateHikingActivityRequest
{
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public string RouteWkt { get; set; } = string.Empty;
    public HikingDetailsDto Details { get; set; } = new();
}

public sealed class UpdateHikingActivityRequest
{
    public string? Name { get; set; }
    public string? Description { get; set; }
    public string? RouteWkt { get; set; }
    public HikingDetailsDto? Details { get; set; }
}

public sealed class HikingDetailsDto
{
    public int DistanceMeters { get; set; }
    public int AscentMeters { get; set; }
    public int DescentMeters { get; set; }
    public int ElevationMinMeters { get; set; }
    public int ElevationMaxMeters { get; set; }
    public HikingDifficulty Difficulty { get; set; }
    public TrailSurface Surface { get; set; }
    public TrailMarking Marking { get; set; }
    public float? EstimatedHours { get; set; }
    public bool HasWaterSources { get; set; }
    public bool HasShelter { get; set; }
    public List<WaterSourceDto> WaterSources { get; set; } = new();

    public HikingDetails ToValueObject() => new(
        DistanceMeters, AscentMeters, DescentMeters,
        ElevationMinMeters, ElevationMaxMeters,
        Difficulty, Surface, Marking,
        EstimatedHours, HasWaterSources, HasShelter,
        WaterSources.Select(w => new WaterSource(w.Lat, w.Lon, w.Kind, w.Notes)).ToList());
}

public sealed class WaterSourceDto
{
    public double Lat { get; set; }
    public double Lon { get; set; }
    public string Kind { get; set; } = string.Empty;
    public string? Notes { get; set; }
}
