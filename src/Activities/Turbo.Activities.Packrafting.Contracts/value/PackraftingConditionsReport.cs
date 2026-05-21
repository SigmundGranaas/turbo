using System.Text.Json.Serialization;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Packrafting.value;

public sealed record PackraftingConditionsReport
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("fetchedAt")] public DateTimeOffset FetchedAt { get; init; }
    [JsonPropertyName("weather")] public WeatherSlice Weather { get; init; }

    /// <summary>Current flow in m³/s from the linked NVE station. Null
    /// when no station is configured or the NveRiverFlowProvider isn't
    /// running.</summary>
    [JsonPropertyName("currentFlowCumecs")] public float? CurrentFlowCumecs { get; init; }

    /// <summary>Trend over the last 24h: "rising", "stable", "falling"
    /// or null if not enough data.</summary>
    [JsonPropertyName("flowTrend")] public string? FlowTrend { get; init; }

    [JsonPropertyName("score")] public int? Score { get; init; }
    [JsonPropertyName("rationale")] public string Rationale { get; init; }

    [JsonConstructor]
    public PackraftingConditionsReport(
        Guid activityId, DateTimeOffset validAt, DateTimeOffset fetchedAt,
        WeatherSlice weather, float? currentFlowCumecs, string? flowTrend,
        int? score, string rationale)
    {
        ActivityId = activityId;
        ValidAt = validAt;
        FetchedAt = fetchedAt;
        Weather = weather;
        CurrentFlowCumecs = currentFlowCumecs;
        FlowTrend = flowTrend;
        Score = score;
        Rationale = rationale ?? throw new ArgumentNullException(nameof(rationale));
    }
}
