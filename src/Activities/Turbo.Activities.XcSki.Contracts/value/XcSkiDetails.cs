using System.Text.Json.Serialization;

namespace Turboapi.Activities.XcSki.value;

public sealed record XcSkiDetails
{
    [JsonPropertyName("distanceMeters")] public int DistanceMeters { get; init; }
    [JsonPropertyName("ascentMeters")] public int AscentMeters { get; init; }
    [JsonPropertyName("descentMeters")] public int DescentMeters { get; init; }
    [JsonPropertyName("technique")] public XcSkiTechnique Technique { get; init; }
    [JsonPropertyName("groomingStatus")] public GroomingStatus GroomingStatus { get; init; }
    [JsonPropertyName("isLit")] public bool IsLit { get; init; }
    [JsonPropertyName("requiresSeasonPass")] public bool RequiresSeasonPass { get; init; }
    [JsonPropertyName("groomingFeedKey")] public string? GroomingFeedKey { get; init; }

    [JsonConstructor]
    public XcSkiDetails(
        int distanceMeters, int ascentMeters, int descentMeters,
        XcSkiTechnique technique, GroomingStatus groomingStatus,
        bool isLit, bool requiresSeasonPass, string? groomingFeedKey)
    {
        DistanceMeters = distanceMeters;
        AscentMeters = ascentMeters;
        DescentMeters = descentMeters;
        Technique = technique;
        GroomingStatus = groomingStatus;
        IsLit = isLit;
        RequiresSeasonPass = requiresSeasonPass;
        GroomingFeedKey = groomingFeedKey;
    }
}
