using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic dev fallback for <see cref="ITurbidityProvider"/>.
/// Seeded by (rounded lat/lon, day) so reruns are reproducible.
/// Mid-summer / coastal-river-mouth combinations produce higher
/// turbidity to match the seasonal envelope a real Sentinel-2 series
/// would show — gives the freediving orchestrator a non-trivial signal
/// to work with in dev.
/// </summary>
public sealed class SyntheticTurbidityProvider : ITurbidityProvider
{
    public string Key => "synthetic_turbidity";

    public Task<TurbiditySlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at,
        CancellationToken cancellationToken)
    {
        var cell = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        var day = new DateTimeOffset(at.Year, at.Month, at.Day, 0, 0, 0, TimeSpan.Zero);
        var seed = HashCode.Combine(cell, day.ToUnixTimeSeconds());
        var rng = new Random(seed);

        var doy = day.UtcDateTime.DayOfYear;
        // Summer bloom envelope. Peak turbidity around late-June to
        // mid-August (DOY 175–230); winter base is clear.
        var seasonal = Math.Max(0.0,
            Math.Cos((doy - 200) / 365.0 * 2 * Math.PI) * 0.5 + 0.5);
        var baseNtu = 1.0 + 6.0 * seasonal;
        var jitter = (rng.NextDouble() - 0.5) * 2.5;
        var turbidity = Math.Max(0.2, baseNtu + jitter);

        var cloud = rng.NextDouble() * 80;        // Sentinel-2 pixels often cloudy in NO
        var ageH = (int)Math.Floor(rng.NextDouble() * 96);

        return Task.FromResult(new TurbiditySlice(
            validAt: day,
            turbidityNtu: Math.Round(turbidity, 2),
            cloudCoveragePct: Math.Round(cloud, 0),
            ageHours: ageH));
    }
}
