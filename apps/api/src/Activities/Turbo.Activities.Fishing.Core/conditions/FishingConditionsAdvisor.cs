using Turboapi.Activities.Fishing.domain;
using Turboapi.Activities.Fishing.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Fishing.conditions;

/// <summary>
/// Default fishing conditions advisor. Pulls weather for the spot at
/// the requested instant, scores against the user's
/// <see cref="PreferredConditions"/> if set, and produces a short
/// rationale. Tides + river flow advisors will compose alongside this
/// in a follow-up; the typed report has named fields for them.
/// </summary>
public sealed class FishingConditionsAdvisor : IFishingConditionsAdvisor
{
    private readonly IWeatherProvider _weather;
    private readonly TimeProvider _clock;

    public FishingConditionsAdvisor(IWeatherProvider weather, TimeProvider? clock = null)
    {
        _weather = weather;
        _clock = clock ?? TimeProvider.System;
    }

    public async Task<FishingConditionsReport> AdviseAsync(
        FishingActivity activity, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var point = activity.Position;
        var weather = await _weather.GetAsync(
            latitude: point.Y, longitude: point.X, at, cancellationToken);

        var preferred = activity.Details.Preferred;
        var (score, rationale) = ScoreAndRationale(weather, preferred);

        return new FishingConditionsReport(
            activityId: activity.Core.Id,
            validAt: weather.ValidAt,
            fetchedAt: _clock.GetUtcNow(),
            weather: weather,
            score: score,
            rationale: rationale);
    }

    private static (int? score, string rationale) ScoreAndRationale(
        WeatherSlice w, PreferredConditions? preferred)
    {
        // Base climate score from rules-of-thumb: penalize high wind,
        // heavy precipitation, and pressure swings.
        var s = 100.0;
        var reasons = new List<string>();

        if (w.WindSpeedMs > 12) { s -= 40; reasons.Add($"strong wind ({w.WindSpeedMs:F0} m/s)"); }
        else if (w.WindSpeedMs > 8) { s -= 20; reasons.Add($"fresh wind ({w.WindSpeedMs:F0} m/s)"); }

        var p1h = w.PrecipitationNext1hMm ?? 0;
        if (p1h > 5) { s -= 30; reasons.Add($"heavy rain ({p1h:F1} mm/h)"); }
        else if (p1h > 1) { s -= 10; reasons.Add($"light rain"); }

        if (w.AirPressureHpa < 1000) { s -= 10; reasons.Add("low pressure"); }
        else if (w.AirPressureHpa > 1025) { s -= 5; }

        // Apply user-attested preferences when present.
        if (preferred is not null)
        {
            if (preferred.PressureMinHpa is { } minP && w.AirPressureHpa < minP)
            {
                s -= 15;
                reasons.Add($"pressure below your preferred minimum ({minP} hPa)");
            }
            if (preferred.PressureMaxHpa is { } maxP && w.AirPressureHpa > maxP)
            {
                s -= 15;
                reasons.Add($"pressure above your preferred maximum ({maxP} hPa)");
            }
            if (preferred.WindMaxMs is { } maxW && w.WindSpeedMs > maxW)
            {
                s -= 20;
                reasons.Add($"wind exceeds your preferred maximum ({maxW:F1} m/s)");
            }
        }

        var score = (int)Math.Clamp(Math.Round(s), 0, 100);
        var rationale = reasons.Count == 0
            ? "Calm and stable — good fishing conditions."
            : $"Conditions: {string.Join(", ", reasons)}.";

        // Score is null only when both no preferences are set AND the
        // base climate scoring made no adjustments — i.e. perfectly
        // neutral conditions. Otherwise we return the computed value.
        return (score == 100 && preferred is null && reasons.Count == 0 ? (int?)null : score, rationale);
    }
}
