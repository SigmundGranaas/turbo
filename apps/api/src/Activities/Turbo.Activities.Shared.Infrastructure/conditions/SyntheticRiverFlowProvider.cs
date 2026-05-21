using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic river-flow generator used when no NVE provider is
/// configured. Produces flow varying by station code (so different
/// rivers look different) and time-of-year (seasonal melt curve in
/// May–June, low in winter). Trend is derived from the last 24h
/// synthesis.
/// </summary>
public sealed class SyntheticRiverFlowProvider : IRiverFlowProvider
{
    public string Key => "synthetic_river_flow";

    public Task<RiverFlowSlice> GetAsync(
        string nveStationCode, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var current = SynthesizeFlowAt(nveStationCode, at);
        var yesterday = SynthesizeFlowAt(nveStationCode, at.AddDays(-1));
        var ratio = (current - yesterday) / Math.Max(yesterday, 0.1f);
        var trend = ratio switch
        {
            > 0.1f => "rising",
            < -0.1f => "falling",
            _ => "stable",
        };
        return Task.FromResult(new RiverFlowSlice(
            stationCode: nveStationCode,
            validAt: at,
            currentCumecs: current,
            trend: trend));
    }

    private static float SynthesizeFlowAt(string nveStationCode, DateTimeOffset at)
    {
        var stationSeed = nveStationCode.GetHashCode(StringComparison.Ordinal);
        var baseSize = 5.0f + Math.Abs(stationSeed % 80); // station-specific baseline 5–85 m³/s
        var dayOfYear = at.DayOfYear;
        // Northern-hemisphere seasonal melt: low in winter, peak May–June, declining through autumn.
        var seasonal = (float)(0.4 + 1.4 * Math.Exp(-Math.Pow((dayOfYear - 152) / 60.0, 2)));
        // Diurnal melt cycle peaks ~16:00 local; small effect.
        var diurnal = 1.0f + 0.05f * (float)Math.Sin((at.Hour - 10) * Math.PI / 12.0);
        var rng = new Random(HashCode.Combine(stationSeed, at.DayOfYear));
        var noise = 1.0f + (float)(rng.NextDouble() - 0.5) * 0.15f;
        return Math.Max(0.5f, baseSize * seasonal * diurnal * noise);
    }
}
