using System.Text.Json.Serialization;

namespace Turboapi.Tracks.domain.value;

/// <summary>
/// Derived totals the client computes while recording. The server stores
/// these as-is — the projection does not recompute. Treating them as
/// client-attested keeps the API simple and avoids recomputing distance,
/// ascent, etc. server-side on every projection.
/// </summary>
public record TrackStats
{
    [JsonPropertyName("distanceMeters")]
    public double DistanceMeters { get; init; }

    [JsonPropertyName("ascentMeters")]
    public double? AscentMeters { get; init; }

    [JsonPropertyName("descentMeters")]
    public double? DescentMeters { get; init; }

    [JsonPropertyName("movingTimeSeconds")]
    public int? MovingTimeSeconds { get; init; }

    [JsonPropertyName("recordedAt")]
    public DateTime? RecordedAt { get; init; }

    [JsonConstructor]
    public TrackStats(
        double distanceMeters,
        double? ascentMeters = null,
        double? descentMeters = null,
        int? movingTimeSeconds = null,
        DateTime? recordedAt = null)
    {
        DistanceMeters = distanceMeters;
        AscentMeters = ascentMeters;
        DescentMeters = descentMeters;
        MovingTimeSeconds = movingTimeSeconds;
        RecordedAt = recordedAt;
    }

    public TrackStats() { }
}
