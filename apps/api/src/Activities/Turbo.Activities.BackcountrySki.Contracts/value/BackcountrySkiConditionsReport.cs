using System.Text.Json.Serialization;
using Turboapi.Activities.value;

namespace Turboapi.Activities.BackcountrySki.value;

/// <summary>
/// Typed conditions report for a backcountry ski route. Carries the
/// same <see cref="WeatherSlice"/> the other kinds use, plus
/// kind-specific avalanche fields (currently always null pending a
/// real <c>VarsomAvalancheProvider</c> integration — the wire shape is
/// in place so the client can render the slot once data arrives).
/// </summary>
public sealed record BackcountrySkiConditionsReport
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("fetchedAt")] public DateTimeOffset FetchedAt { get; init; }

    [JsonPropertyName("weather")] public WeatherSlice Weather { get; init; }

    /// <summary>Varsom avalanche danger level 1–5 if available.
    /// Null until <c>VarsomAvalancheProvider</c> ships.</summary>
    [JsonPropertyName("avalancheLevel")] public int? AvalancheLevel { get; init; }

    /// <summary>Short summary string from Varsom. Null until provider
    /// ships.</summary>
    [JsonPropertyName("avalancheSummary")] public string? AvalancheSummary { get; init; }

    [JsonPropertyName("score")] public int? Score { get; init; }
    [JsonPropertyName("rationale")] public string Rationale { get; init; }

    [JsonConstructor]
    public BackcountrySkiConditionsReport(
        Guid activityId, DateTimeOffset validAt, DateTimeOffset fetchedAt,
        WeatherSlice weather,
        int? avalancheLevel, string? avalancheSummary,
        int? score, string rationale)
    {
        ActivityId = activityId;
        ValidAt = validAt;
        FetchedAt = fetchedAt;
        Weather = weather;
        AvalancheLevel = avalancheLevel;
        AvalancheSummary = avalancheSummary;
        Score = score;
        Rationale = rationale ?? throw new ArgumentNullException(nameof(rationale));
    }
}
