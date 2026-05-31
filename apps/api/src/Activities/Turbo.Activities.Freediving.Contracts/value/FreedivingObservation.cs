using System.Text.Json.Serialization;

namespace Turboapi.Activities.Freediving.value;

/// <summary>
/// User-contributed post-visit observation for a freediving spot. The
/// headline field is <see cref="VisibilityMeters"/> — the ground-truth
/// counterpart to the orchestrator's computed <c>viz_estimate</c>
/// driver. Aggregated observations on a spot calibrate that model over
/// time.
/// </summary>
public sealed record FreedivingObservation
{
    /// <summary>Observed visibility in meters at the spot at the time
    /// of the visit. The orchestrator's <c>nearby_obs</c> driver treats
    /// recent values as a strong calibration signal.</summary>
    [JsonPropertyName("visibilityMeters")] public double? VisibilityMeters { get; init; }

    /// <summary>Water temperature (°C) at depth — colder than the air
    /// proxy the orchestrator falls back to.</summary>
    [JsonPropertyName("waterTempC")] public double? WaterTempC { get; init; }

    /// <summary>Current strength code: <c>"none"</c>, <c>"light"</c>,
    /// <c>"moderate"</c>, <c>"strong"</c>. Free-form so future divers'
    /// vocabularies plug in without a schema change.</summary>
    [JsonPropertyName("currentStrength")] public string? CurrentStrength { get; init; }

    /// <summary>Species observed during the dive (free-form codes from
    /// the kind's target-species list or new entries).</summary>
    [JsonPropertyName("speciesSeen")] public IReadOnlyList<string> SpeciesSeen { get; init; }

    [JsonPropertyName("concerns")] public string? Concerns { get; init; }

    [JsonConstructor]
    public FreedivingObservation(
        double? visibilityMeters,
        double? waterTempC,
        string? currentStrength,
        IReadOnlyList<string>? speciesSeen,
        string? concerns)
    {
        VisibilityMeters = visibilityMeters;
        WaterTempC = waterTempC;
        CurrentStrength = currentStrength;
        SpeciesSeen = speciesSeen ?? Array.Empty<string>();
        Concerns = concerns;
    }
}
