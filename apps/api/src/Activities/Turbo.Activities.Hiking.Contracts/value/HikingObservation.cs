using System.Text.Json.Serialization;

namespace Turboapi.Activities.Hiking.value;

/// <summary>
/// Post-hike observation. Trail condition + closures + water-source
/// confirmation. Feeds the orchestrator's <c>nearby_obs</c> driver and
/// surfaces real-world reports about state of marking / fords /
/// snow-line.
/// </summary>
public sealed record HikingObservation
{
    [JsonPropertyName("trailCondition")] public string? TrailCondition { get; init; }
    [JsonPropertyName("snowAt")] public double? SnowAt { get; init; }
    [JsonPropertyName("waterSourcesFlowing")] public bool? WaterSourcesFlowing { get; init; }
    [JsonPropertyName("markingState")] public string? MarkingState { get; init; }
    [JsonPropertyName("concerns")] public string? Concerns { get; init; }

    [JsonConstructor]
    public HikingObservation(
        string? trailCondition, double? snowAt, bool? waterSourcesFlowing,
        string? markingState, string? concerns)
    {
        TrailCondition = trailCondition;
        SnowAt = snowAt;
        WaterSourcesFlowing = waterSourcesFlowing;
        MarkingState = markingState;
        Concerns = concerns;
    }
}
