using System.Text.Json.Serialization;

namespace Turboapi.Activities.Hiking.value;

public sealed record HikingDetails
{
    [JsonPropertyName("distanceMeters")] public int DistanceMeters { get; init; }
    [JsonPropertyName("ascentMeters")] public int AscentMeters { get; init; }
    [JsonPropertyName("descentMeters")] public int DescentMeters { get; init; }
    [JsonPropertyName("elevationMinMeters")] public int ElevationMinMeters { get; init; }
    [JsonPropertyName("elevationMaxMeters")] public int ElevationMaxMeters { get; init; }
    [JsonPropertyName("difficulty")] public HikingDifficulty Difficulty { get; init; }
    [JsonPropertyName("surface")] public TrailSurface Surface { get; init; }
    [JsonPropertyName("marking")] public TrailMarking Marking { get; init; }
    [JsonPropertyName("estimatedHours")] public float? EstimatedHours { get; init; }
    [JsonPropertyName("hasWaterSources")] public bool HasWaterSources { get; init; }
    [JsonPropertyName("hasShelter")] public bool HasShelter { get; init; }
    [JsonPropertyName("waterSources")] public IReadOnlyList<WaterSource> WaterSources { get; init; } = Array.Empty<WaterSource>();

    [JsonConstructor]
    public HikingDetails(
        int distanceMeters, int ascentMeters, int descentMeters,
        int elevationMinMeters, int elevationMaxMeters,
        HikingDifficulty difficulty, TrailSurface surface, TrailMarking marking,
        float? estimatedHours, bool hasWaterSources, bool hasShelter,
        IReadOnlyList<WaterSource>? waterSources)
    {
        DistanceMeters = distanceMeters;
        AscentMeters = ascentMeters;
        DescentMeters = descentMeters;
        ElevationMinMeters = elevationMinMeters;
        ElevationMaxMeters = elevationMaxMeters;
        Difficulty = difficulty;
        Surface = surface;
        Marking = marking;
        EstimatedHours = estimatedHours;
        HasWaterSources = hasWaterSources;
        HasShelter = hasShelter;
        WaterSources = waterSources ?? Array.Empty<WaterSource>();
    }
}

public sealed record WaterSource
{
    [JsonPropertyName("lat")] public double Lat { get; init; }
    [JsonPropertyName("lon")] public double Lon { get; init; }
    [JsonPropertyName("kind")] public string Kind { get; init; }
    [JsonPropertyName("notes")] public string? Notes { get; init; }

    [JsonConstructor]
    public WaterSource(double lat, double lon, string kind, string? notes)
    {
        Lat = lat; Lon = lon;
        Kind = kind ?? throw new ArgumentNullException(nameof(kind));
        Notes = notes;
    }
}
