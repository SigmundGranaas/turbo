using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Elevation samples along a geometry (point: one sample; line:
/// regularly-spaced samples along the linestring). Sourced from Kartverket
/// Høydedata WCS in production; deterministic ramp in dev.
/// <see cref="Samples"/> is ordered by <see cref="ElevationSample.DistanceM"/>
/// from the start of the geometry. Distances on a point are always 0.
/// </summary>
public sealed record ElevationSlice
{
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("samples")] public IReadOnlyList<ElevationSample> Samples { get; init; }

    [JsonConstructor]
    public ElevationSlice(DateTimeOffset validAt, IReadOnlyList<ElevationSample> samples)
    {
        ValidAt = validAt;
        Samples = samples ?? Array.Empty<ElevationSample>();
    }
}

public sealed record ElevationSample
{
    [JsonPropertyName("distanceM")] public double DistanceM { get; init; }
    [JsonPropertyName("elevationM")] public double ElevationM { get; init; }

    [JsonConstructor]
    public ElevationSample(double distanceM, double elevationM)
    {
        DistanceM = distanceM;
        ElevationM = elevationM;
    }
}

/// <summary>
/// DEM provider. Implementations sample elevation along a geometry at
/// roughly fixed spacing (~25m by default). Used by the geo-context
/// service to derive ascent / descent / aspect histograms at activity
/// create/update time. Synthetic returns a deterministic linear ramp so
/// dev/test runs don't need network access.
/// </summary>
public interface IElevationProvider
{
    string Key { get; }

    Task<ElevationSlice> GetAsync(
        IReadOnlyList<(double Latitude, double Longitude)> path,
        double sampleSpacingM,
        CancellationToken cancellationToken);
}
