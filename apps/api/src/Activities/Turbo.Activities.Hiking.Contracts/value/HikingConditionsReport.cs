using System.Text.Json.Serialization;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Hiking.value;

public sealed record HikingConditionsReport
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("fetchedAt")] public DateTimeOffset FetchedAt { get; init; }
    [JsonPropertyName("weather")] public WeatherSlice Weather { get; init; }
    [JsonPropertyName("score")] public int? Score { get; init; }
    [JsonPropertyName("rationale")] public string Rationale { get; init; }

    [JsonConstructor]
    public HikingConditionsReport(
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
