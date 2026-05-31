using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic seNorge-style snow-state generator. Seeded by
/// (rounded lat/lon, day) so dev/test runs are reproducible. Snow depth
/// scales loosely with latitude + season (more snow further north, peak
/// in February).
/// </summary>
public sealed class SyntheticGriddedSnowProvider : IGriddedSnowProvider
{
    public string Key => "synthetic_gridded_snow";

    public Task<GriddedSnowSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var cell = $"{Math.Round(latitude, 1):F1}_{Math.Round(longitude, 1):F1}";
        var day = new DateTimeOffset(at.Year, at.Month, at.Day, 0, 0, 0, TimeSpan.Zero);
        var seed = HashCode.Combine(cell, day.ToUnixTimeSeconds());
        var rng = new Random(seed);

        // Seasonal envelope: peak around mid-Feb (DOY ~46), drops to zero
        // mid-Apr / Oct.
        var doy = day.UtcDateTime.DayOfYear;
        var seasonal = Math.Max(0.0, Math.Cos((doy - 46) / 365.0 * 2 * Math.PI) * 0.5 + 0.5);
        var latitudeBoost = Math.Max(0, (latitude - 58.0) / 14.0);
        var baseCm = (40 + 80 * latitudeBoost) * seasonal;
        var snowDepthCm = Math.Round(baseCm + (rng.NextDouble() - 0.5) * 20, 1);
        if (snowDepthCm < 0) snowDepthCm = 0;

        // SWE roughly 30% of depth.
        var sweMm = Math.Round(snowDepthCm * 3 * (0.9 + rng.NextDouble() * 0.2), 1);

        // Fresh snow last 24h: about 15% chance of a meaningful dump.
        var freshCm = rng.NextDouble() < 0.15
            ? Math.Round(rng.NextDouble() * 12 * seasonal, 1)
            : 0.0;

        var freezeThaw = (int)Math.Round(rng.NextDouble() * 4 * (1 - seasonal));

        return Task.FromResult(new GriddedSnowSlice(
            validAt: day,
            sweMm: sweMm,
            snowDepthCm: snowDepthCm,
            freshSnowLast24hCm: freshCm,
            freezeThawLast7d: freezeThaw));
    }
}
