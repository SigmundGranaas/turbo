using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic tide generator. Sinusoidal cycle with a 12h25min
/// period (close to the real M2 lunar semidiurnal period), amplitude
/// scaled mildly by latitude (higher tides at higher latitudes in
/// Norway). Produces a slack / flood / ebb summary based on derivative.
/// </summary>
public sealed class SyntheticTideProvider : ITideProvider
{
    public string Key => "synthetic_tide";

    public Task<TideSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        const double periodSec = 12.421 * 3600.0; // M2 lunar semidiurnal period.
        var latInt = (int)Math.Round(latitude * 100, MidpointRounding.ToEven);
        var lonInt = (int)Math.Round(longitude * 100, MidpointRounding.ToEven);
        var phaseSeed = HashCode.Combine(latInt, lonInt);
        // Stable phase offset per (lat, lon) cell so neighbours don't see
        // wildly different tide states.
        var phase = (phaseSeed % 1000) / 1000.0 * 2 * Math.PI;

        var t = at.ToUnixTimeSeconds();
        var omega = 2 * Math.PI / periodSec;
        var amplitude = 0.6 + 0.4 * Math.Cos(latitude * Math.PI / 180.0); // 0.6–1.0 m typical Norway range
        var height = amplitude * Math.Sin(omega * t + phase);

        // Derivative tells us flood (rising) vs ebb (falling) vs slack
        // (near zero). Compute by sampling 5 minutes ahead.
        var heightAhead = amplitude * Math.Sin(omega * (t + 300) + phase);
        var dh = heightAhead - height;
        var summary = Math.Abs(dh) < amplitude * 0.001
            ? "slack"
            : dh > 0 ? "rising tide" : "falling tide";

        return Task.FromResult(new TideSlice(
            validAt: at,
            currentHeightMeters: (float)height,
            summary: summary));
    }
}
