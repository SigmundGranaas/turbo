using System.Text.Json.Serialization;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Fishing.value;

/// <summary>
/// Typed conditions report returned by
/// <c>GET /api/activities/fishing/{id}/conditions</c>. Every field has a
/// name and a unit on the wire — no map-of-strings, no JSONB blob.
/// </summary>
public sealed record FishingConditionsReport
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("fetchedAt")] public DateTimeOffset FetchedAt { get; init; }

    [JsonPropertyName("weather")] public WeatherSlice Weather { get; init; }

    /// <summary>0–100 score computed by the kind's advisor. Higher is
    /// better. Null when the activity has no preferred conditions set.
    /// </summary>
    [JsonPropertyName("score")] public int? Score { get; init; }

    /// <summary>Short, free-form rationale ("calm and stable, good
    /// conditions"). The client renders verbatim — translation is the
    /// server's job if it ever does any.</summary>
    [JsonPropertyName("rationale")] public string Rationale { get; init; }

    [JsonConstructor]
    public FishingConditionsReport(
        Guid activityId, DateTimeOffset validAt, DateTimeOffset fetchedAt,
        WeatherSlice weather, int? score, string rationale)
    {
        ActivityId = activityId;
        ValidAt = validAt;
        FetchedAt = fetchedAt;
        Weather = weather;
        Score = score;
        Rationale = rationale ?? throw new ArgumentNullException(nameof(rationale));
    }
}
