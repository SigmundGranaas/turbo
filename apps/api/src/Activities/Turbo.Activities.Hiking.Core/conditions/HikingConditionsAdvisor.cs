using Turboapi.Activities.Hiking.domain;
using Turboapi.Activities.Hiking.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Hiking.conditions;

public interface IHikingConditionsAdvisor
{
    Task<HikingConditionsReport> AdviseAsync(
        HikingActivity activity, DateTimeOffset at, CancellationToken cancellationToken);
}

/// <summary>
/// Weather-only hiking advisor. Scoring leans on heat/cold extremes,
/// wind for exposed terrain, and precipitation; the activity's
/// difficulty + estimated duration make the same weather more or less
/// risky.
/// </summary>
public sealed class HikingConditionsAdvisor : IHikingConditionsAdvisor
{
    private readonly IWeatherProvider _weather;
    private readonly TimeProvider _clock;

    public HikingConditionsAdvisor(IWeatherProvider weather, TimeProvider? clock = null)
    {
        _weather = weather;
        _clock = clock ?? TimeProvider.System;
    }

    public async Task<HikingConditionsReport> AdviseAsync(
        HikingActivity activity, DateTimeOffset at, CancellationToken cancellationToken)
    {
        // Use the trail midpoint as the representative weather sample.
        var line = activity.Route;
        var mid = line.Coordinates[line.NumPoints / 2];
        var w = await _weather.GetAsync(mid.Y, mid.X, at, cancellationToken);

        var (score, rationale) = ScoreAndRationale(w, activity.Details);
        return new HikingConditionsReport(
            activity.Core.Id, w.ValidAt, _clock.GetUtcNow(), w, score, rationale);
    }

    private static (int? score, string rationale) ScoreAndRationale(WeatherSlice w, HikingDetails d)
    {
        var s = 100.0;
        var reasons = new List<string>();

        if (w.AirTemperatureCelsius > 28) { s -= 25; reasons.Add($"{w.AirTemperatureCelsius:F0}°C — heat risk on exposed sections"); }
        else if (w.AirTemperatureCelsius < -10) { s -= 20; reasons.Add($"{w.AirTemperatureCelsius:F0}°C — cold exposure"); }

        var p1h = w.PrecipitationNext1hMm ?? 0;
        if (p1h > 5) { s -= 25; reasons.Add($"heavy rain ({p1h:F1} mm/h)"); }
        else if (p1h > 1) { s -= 8; reasons.Add("light rain"); }

        // Exposed / above-treeline routes feel wind more — penalize harder
        // when the marking suggests an alpine route (cairns + scree often
        // means above tree line).
        var exposed = d.Marking == TrailMarking.Cairns
                      || d.Surface == TrailSurface.Scree
                      || d.Surface == TrailSurface.Rock
                      || d.Surface == TrailSurface.Snow
                      || d.ElevationMaxMeters >= 1000;
        var windPenalty = exposed ? 1.5 : 1.0;
        if (w.WindSpeedMs > 15) { s -= (int)(25 * windPenalty); reasons.Add($"strong wind ({w.WindSpeedMs:F0} m/s)"); }
        else if (w.WindSpeedMs > 10) { s -= (int)(10 * windPenalty); reasons.Add($"fresh wind ({w.WindSpeedMs:F0} m/s)"); }

        // Difficulty + duration: a 6h Hard trail in marginal weather is
        // more exposed than a 1h Easy one in the same weather.
        if (d.Difficulty == HikingDifficulty.Expert
            && (w.WindSpeedMs > 12 || p1h > 2 || w.AirTemperatureCelsius < 0))
        {
            s -= 15;
            reasons.Add("expert-grade route in marginal weather");
        }

        if (d.EstimatedHours is { } h && h > 4 && (p1h > 1 || w.WindSpeedMs > 12))
        {
            s -= 10;
            reasons.Add($"{h:F1}h route in deteriorating weather");
        }

        var score = (int)Math.Clamp(Math.Round(s), 0, 100);
        var rationale = reasons.Count == 0
            ? "Pleasant conditions for a hike."
            : $"Conditions: {string.Join(", ", reasons)}.";
        return (score == 100 && reasons.Count == 0 ? (int?)null : score, rationale);
    }
}
