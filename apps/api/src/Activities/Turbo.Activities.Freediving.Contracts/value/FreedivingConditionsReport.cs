using System.Text.Json.Serialization;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Freediving.value;

public sealed record FreedivingConditionsReport
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("fetchedAt")] public DateTimeOffset FetchedAt { get; init; }
    [JsonPropertyName("weather")] public WeatherSlice Weather { get; init; }

    /// <summary>Sea state at the spot (m³/s current, or wave-height
    /// proxy). Null until the TidesProvider lands.</summary>
    [JsonPropertyName("seaStateSummary")] public string? SeaStateSummary { get; init; }

    [JsonPropertyName("score")] public int? Score { get; init; }
    [JsonPropertyName("rationale")] public string Rationale { get; init; }

    [JsonConstructor]
    public FreedivingConditionsReport(
        Guid activityId, DateTimeOffset validAt, DateTimeOffset fetchedAt,
        WeatherSlice weather, string? seaStateSummary, int? score, string rationale)
    {
        ActivityId = activityId;
        ValidAt = validAt;
        FetchedAt = fetchedAt;
        Weather = weather;
        SeaStateSummary = seaStateSummary;
        Score = score;
        Rationale = rationale ?? throw new ArgumentNullException(nameof(rationale));
    }
}
