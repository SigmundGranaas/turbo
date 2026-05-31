using System.Text.Json.Serialization;

namespace Turboapi.Activities.XcSki.value;

/// <summary>
/// User-contributed post-visit observation for an XC-ski trail. Lives in
/// the kind's Contracts assembly so the typed payload stays out of the
/// shared store — the persistence layer holds this as jsonb in
/// <c>activities.activity_observations.kind_payload</c>.
///
/// Read by the orchestrator's <c>nearby_obs</c> driver: recent
/// observations on the same trail boost (or undermine) the model's
/// confidence in its score with ground truth.
/// </summary>
public sealed record XcSkiObservation
{
    /// <summary>What the actual track surface felt like
    /// (<c>"frozen_granular"</c>, <c>"fast_glide"</c>, <c>"sticky"</c>,
    /// <c>"icy_ruts"</c>, <c>"deep_unbroken"</c>, …). Free-form code so
    /// the synthesizer can map to severity locally.</summary>
    [JsonPropertyName("trackCondition")] public string? TrackCondition { get; init; }

    /// <summary>Tied to the gridded snow driver — user-reported snow
    /// quality (<c>"powder"</c>, <c>"hard_pack"</c>, <c>"breakable_crust"</c>,
    /// <c>"wet"</c>).</summary>
    [JsonPropertyName("snowQuality")] public string? SnowQuality { get; init; }

    /// <summary>Whether grooming was visibly recent (track machine
    /// tracks, fresh corduroy).</summary>
    [JsonPropertyName("freshGroomingVisible")] public bool? FreshGroomingVisible { get; init; }

    [JsonPropertyName("concerns")] public string? Concerns { get; init; }

    [JsonConstructor]
    public XcSkiObservation(
        string? trackCondition,
        string? snowQuality,
        bool? freshGroomingVisible,
        string? concerns)
    {
        TrackCondition = trackCondition;
        SnowQuality = snowQuality;
        FreshGroomingVisible = freshGroomingVisible;
        Concerns = concerns;
    }
}
