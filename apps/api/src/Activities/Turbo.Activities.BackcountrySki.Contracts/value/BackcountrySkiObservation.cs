using System.Text.Json.Serialization;

namespace Turboapi.Activities.BackcountrySki.value;

/// <summary>
/// Post-tour observation for a backcountry ski route. Safety-critical:
/// the orchestrator's <c>nearby_obs</c> and <c>weak_layers</c> drivers
/// fan in these reports, and the regObs feed is the gap between
/// Varsom's regional bulletin and what's actually happening on this
/// slope.
/// </summary>
public sealed record BackcountrySkiObservation
{
    /// <summary>Free-form code: <c>"powder"</c>, <c>"hard_pack"</c>,
    /// <c>"breakable_crust"</c>, <c>"wind_slab_present"</c>,
    /// <c>"corn"</c>, etc.</summary>
    [JsonPropertyName("snowConditionSummary")] public string? SnowConditionSummary { get; init; }

    [JsonPropertyName("breakableCrust")] public bool? BreakableCrust { get; init; }

    /// <summary>1–5; the user's read of the day's danger, calibrated
    /// against Varsom's regional level. A persistent gap between
    /// observed and bulletin levels is a useful learning signal.</summary>
    [JsonPropertyName("observedDangerLevel")] public short? ObservedDangerLevel { get; init; }

    /// <summary>Signs of instability codes: <c>"recent_avalanche"</c>,
    /// <c>"whoomphing"</c>, <c>"shooting_cracks"</c>, etc.</summary>
    [JsonPropertyName("signsOfInstability")] public IReadOnlyList<string> SignsOfInstability { get; init; }

    [JsonPropertyName("concerns")] public string? Concerns { get; init; }

    [JsonConstructor]
    public BackcountrySkiObservation(
        string? snowConditionSummary,
        bool? breakableCrust,
        short? observedDangerLevel,
        IReadOnlyList<string>? signsOfInstability,
        string? concerns)
    {
        SnowConditionSummary = snowConditionSummary;
        BreakableCrust = breakableCrust;
        ObservedDangerLevel = observedDangerLevel;
        SignsOfInstability = signsOfInstability ?? Array.Empty<string>();
        Concerns = concerns;
    }
}
