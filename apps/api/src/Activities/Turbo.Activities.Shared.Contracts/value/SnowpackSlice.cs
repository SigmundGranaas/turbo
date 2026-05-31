using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Aggregated regObs observations within a radius and recency window of a
/// query point. Critical for backcountry ski — surfaces the gap between
/// Varsom's regional bulletin and the slope the user actually wants to
/// ski. Also feeds the xc-ski synthesizer (recent slide activity nearby
/// is a quiet "skip it" signal).
/// </summary>
public sealed record SnowpackSlice
{
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }

    /// <summary>Distinct weak-layer observations reported in the window
    /// (e.g. "persistent_slab", "buried_surface_hoar"). Free-form regObs
    /// codes; the synthesizer maps to severity locally.</summary>
    [JsonPropertyName("weakLayers")] public IReadOnlyList<string> WeakLayers { get; init; }

    /// <summary>Number of slide events reported in the window.</summary>
    [JsonPropertyName("recentSlideActivity")] public int RecentSlideActivity { get; init; }

    /// <summary>Stability-test results (free-form short codes — "ECTP12",
    /// "RB3", …). Empty when no tests reported.</summary>
    [JsonPropertyName("stabilityTests")] public IReadOnlyList<string> StabilityTests { get; init; }

    [JsonPropertyName("observationCount")] public int ObservationCount { get; init; }

    [JsonConstructor]
    public SnowpackSlice(
        DateTimeOffset validAt,
        IReadOnlyList<string> weakLayers,
        int recentSlideActivity,
        IReadOnlyList<string> stabilityTests,
        int observationCount)
    {
        ValidAt = validAt;
        WeakLayers = weakLayers ?? Array.Empty<string>();
        RecentSlideActivity = recentSlideActivity;
        StabilityTests = stabilityTests ?? Array.Empty<string>();
        ObservationCount = observationCount;
    }
}

/// <summary>
/// regObs-backed source. Implementations look up community + professional
/// observations within a configurable radius (default ~10 km) of the
/// query point in the last <c>lookbackDays</c>.
/// </summary>
public interface ISnowpackProvider
{
    string Key { get; }

    Task<SnowpackSlice> GetAsync(
        double latitude, double longitude,
        DateTimeOffset at,
        int lookbackDays,
        CancellationToken cancellationToken);
}
