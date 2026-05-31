using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic regObs-style snowpack observation aggregator. Seeded by
/// (rounded lat/lon, day) so dev/test runs are reproducible. Real impl
/// (HTTP regObs) lands later — interface is here so kind orchestrators
/// can be written against it now.
/// </summary>
public sealed class SyntheticSnowpackProvider : ISnowpackProvider
{
    public string Key => "synthetic_snowpack";

    public Task<SnowpackSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, int lookbackDays,
        CancellationToken cancellationToken)
    {
        var cell = $"{Math.Round(latitude, 1):F1}_{Math.Round(longitude, 1):F1}";
        var day = new DateTimeOffset(at.Year, at.Month, at.Day, 0, 0, 0, TimeSpan.Zero);
        var seed = HashCode.Combine(cell, day.ToUnixTimeSeconds(), lookbackDays);
        var rng = new Random(seed);

        var obsCount = rng.Next(0, 18);
        var slides = obsCount == 0 ? 0 : rng.Next(0, Math.Max(1, obsCount / 4));
        var weakPool = new[] { "persistent_slab", "buried_surface_hoar", "wind_slab", "wet_loose" };
        var weak = obsCount switch
        {
            0 => Array.Empty<string>(),
            < 4 => new[] { weakPool[rng.Next(weakPool.Length)] },
            _ => Enumerable.Range(0, rng.Next(1, 3))
                .Select(_ => weakPool[rng.Next(weakPool.Length)])
                .Distinct().ToArray(),
        };
        var tests = obsCount > 2 && rng.NextDouble() < 0.4
            ? new[] { rng.Next(2) == 0 ? "ECTP12" : "RB3" }
            : Array.Empty<string>();

        return Task.FromResult(new SnowpackSlice(
            validAt: day,
            weakLayers: weak,
            recentSlideActivity: slides,
            stabilityTests: tests,
            observationCount: obsCount));
    }
}
