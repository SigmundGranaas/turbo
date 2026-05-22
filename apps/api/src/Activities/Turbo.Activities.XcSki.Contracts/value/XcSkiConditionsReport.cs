using System.Text.Json.Serialization;
using Turboapi.Activities.value;

namespace Turboapi.Activities.XcSki.value;

public sealed record XcSkiConditionsReport
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("fetchedAt")] public DateTimeOffset FetchedAt { get; init; }
    [JsonPropertyName("weather")] public WeatherSlice Weather { get; init; }

    /// <summary>Most-recent grooming status from the external feed
    /// (e.g. Skisporet). Null until the SkisporetGroomingProvider lands;
    /// when null the report falls back to the stored
    /// <c>XcSkiDetails.GroomingStatus</c> field.</summary>
    [JsonPropertyName("liveGroomingHoursAgo")] public int? LiveGroomingHoursAgo { get; init; }

    [JsonPropertyName("score")] public int? Score { get; init; }
    [JsonPropertyName("rationale")] public string Rationale { get; init; }

    [JsonConstructor]
    public XcSkiConditionsReport(
        Guid activityId, DateTimeOffset validAt, DateTimeOffset fetchedAt,
        WeatherSlice weather, int? liveGroomingHoursAgo, int? score, string rationale)
    {
        ActivityId = activityId;
        ValidAt = validAt;
        FetchedAt = fetchedAt;
        Weather = weather;
        LiveGroomingHoursAgo = liveGroomingHoursAgo;
        Score = score;
        Rationale = rationale ?? throw new ArgumentNullException(nameof(rationale));
    }
}
