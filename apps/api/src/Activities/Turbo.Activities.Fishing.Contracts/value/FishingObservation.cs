using System.Text.Json.Serialization;

namespace Turboapi.Activities.Fishing.value;

/// <summary>
/// Post-trip observation for a fishing spot. Tracks what was caught
/// (or wasn't) plus the conditions the user noted — the orchestrator's
/// pressure-trend / solunar drivers calibrate against these
/// observations over time.
/// </summary>
public sealed record FishingObservation
{
    [JsonPropertyName("caught")] public bool Caught { get; init; }
    [JsonPropertyName("species")] public string? Species { get; init; }
    [JsonPropertyName("lengthCm")] public double? LengthCm { get; init; }
    [JsonPropertyName("weightKg")] public double? WeightKg { get; init; }
    [JsonPropertyName("lure")] public string? Lure { get; init; }
    [JsonPropertyName("waterClarity")] public string? WaterClarity { get; init; }
    [JsonPropertyName("concerns")] public string? Concerns { get; init; }

    [JsonConstructor]
    public FishingObservation(
        bool caught, string? species, double? lengthCm, double? weightKg,
        string? lure, string? waterClarity, string? concerns)
    {
        Caught = caught;
        Species = species;
        LengthCm = lengthCm;
        WeightKg = weightKg;
        Lure = lure;
        WaterClarity = waterClarity;
        Concerns = concerns;
    }
}
