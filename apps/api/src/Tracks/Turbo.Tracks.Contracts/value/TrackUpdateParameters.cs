using System.Text.Json.Serialization;

namespace Turboapi.Tracks.domain.value;

/// <summary>
/// The full set of optional updates a PATCH/PUT can carry. Each field is
/// null-or-absent for "no change"; the aggregate compares against the
/// current state before emitting an update event.
/// </summary>
public record TrackUpdateParameters
{
    /// <summary>If not null, replaces the geometry (and the stored elevations).</summary>
    [JsonPropertyName("geometry")]
    public TrackGeometry? Geometry { get; init; }

    /// <summary>If not null, applies a (possibly partial) change to the display metadata.</summary>
    [JsonPropertyName("metadata")]
    public TrackMetadataUpdate? Metadata { get; init; }

    /// <summary>If not null, replaces the stats. Stats are client-attested so partial
    /// updates aren't supported here — the client either resends the full block or
    /// omits it entirely.</summary>
    [JsonPropertyName("stats")]
    public TrackStats? Stats { get; init; }

    [JsonConstructor]
    public TrackUpdateParameters(
        TrackGeometry? geometry = null,
        TrackMetadataUpdate? metadata = null,
        TrackStats? stats = null)
    {
        Geometry = geometry;
        Metadata = metadata;
        Stats = stats;
    }

    [JsonIgnore]
    public bool HasAnyChange =>
        Geometry is not null
        || (Metadata is not null && Metadata.HasAnyChange)
        || Stats is not null;
}
