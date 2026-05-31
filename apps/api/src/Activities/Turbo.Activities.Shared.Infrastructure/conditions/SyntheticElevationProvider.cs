using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic dev fallback for <see cref="IElevationProvider"/>. Pretends
/// the terrain is a linear ramp from sea level at the first point to
/// (start.Latitude × 50)m at the last point — enough variation to exercise
/// the geo-context aspect/slope derivation in tests without hitting a DEM
/// service. Replace with a real Kartverket Høydedata implementation in
/// Phase 2 (pilot).
/// </summary>
public sealed class SyntheticElevationProvider : IElevationProvider
{
    public string Key => "elevation.synthetic";

    public Task<ElevationSlice> GetAsync(
        IReadOnlyList<(double Latitude, double Longitude)> path,
        double sampleSpacingM,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(path);
        if (path.Count == 0)
        {
            return Task.FromResult(new ElevationSlice(DateTimeOffset.UtcNow, Array.Empty<ElevationSample>()));
        }

        var seedElevation = Math.Abs(path[0].Latitude % 1) * 1000;
        var samples = new ElevationSample[path.Count];
        for (var i = 0; i < path.Count; i++)
        {
            var bumps = Math.Sin(i * 0.7) * 25;
            samples[i] = new ElevationSample(
                distanceM: i * sampleSpacingM,
                elevationM: seedElevation + i * 5 + bumps);
        }
        return Task.FromResult(new ElevationSlice(DateTimeOffset.UtcNow, samples));
    }
}
