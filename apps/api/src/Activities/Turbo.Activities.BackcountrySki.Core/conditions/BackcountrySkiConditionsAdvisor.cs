using Turboapi.Activities.BackcountrySki.domain;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.BackcountrySki.conditions;

/// <summary>
/// Default backcountry ski advisor. Composes weather + (optional)
/// avalanche provider. When the avalanche provider is available
/// (Varsom in production, synthetic otherwise) the report's
/// AvalancheLevel + AvalancheSummary are populated and the scoring
/// penalizes ATES Complex routes on level ≥ 3 days; absent the
/// provider, the rationale falls back to "check Varsom before going".
/// </summary>
public sealed class BackcountrySkiConditionsAdvisor : IBackcountrySkiConditionsAdvisor
{
    private readonly IWeatherProvider _weather;
    private readonly IAvalancheProvider? _avalanche;
    private readonly TimeProvider _clock;

    public BackcountrySkiConditionsAdvisor(
        IWeatherProvider weather,
        IAvalancheProvider? avalanche = null,
        TimeProvider? clock = null)
    {
        _weather = weather;
        _avalanche = avalanche;
        _clock = clock ?? TimeProvider.System;
    }

    public async Task<BackcountrySkiConditionsReport> AdviseAsync(
        BackcountrySkiActivity activity, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var line = activity.Route;
        var midpoint = line.Coordinates[line.NumPoints / 2];
        var weather = await _weather.GetAsync(midpoint.Y, midpoint.X, at, cancellationToken);

        AvalancheSlice? avalanche = null;
        if (_avalanche is not null && activity.Details.VarsomRegionId is { } region)
        {
            try { avalanche = await _avalanche.GetAsync(region, at, cancellationToken); }
            catch { /* soft: a missing bulletin shouldn't fail the report */ }
        }

        var (score, rationale) = ScoreAndRationale(weather, avalanche, activity.Details);

        return new BackcountrySkiConditionsReport(
            activityId: activity.Core.Id,
            validAt: weather.ValidAt,
            fetchedAt: _clock.GetUtcNow(),
            weather: weather,
            avalancheLevel: avalanche?.DangerLevel,
            avalancheSummary: avalanche?.Summary,
            score: score,
            rationale: rationale);
    }

    private static (int? score, string rationale) ScoreAndRationale(
        WeatherSlice w, AvalancheSlice? a, BackcountrySkiDetails d)
    {
        var s = 100.0;
        var reasons = new List<string>();

        // Avalanche level dominates when present.
        if (a is not null)
        {
            switch (a.DangerLevel)
            {
                case 5: s -= 80; reasons.Add("avalanche level 5 — avoid all avalanche terrain"); break;
                case 4: s -= 50; reasons.Add("avalanche level 4 — very dangerous"); break;
                case 3:
                    s -= 25;
                    reasons.Add(d.AtesRating == AtesRating.Complex
                        ? "level 3 + complex ATES — significant exposure"
                        : "avalanche level 3");
                    break;
                case 2: s -= 10; reasons.Add("avalanche level 2"); break;
                default: break;
            }
            if (d.PreferredAvalancheMaxLevel is { } maxLevel && a.DangerLevel > maxLevel)
            {
                s -= 15;
                reasons.Add($"forecast level {a.DangerLevel} exceeds your max ({maxLevel})");
            }
        }
        else if (d.PreferredAvalancheMaxLevel is { } maxLevel && maxLevel <= 2 && d.AtesRating == AtesRating.Complex)
        {
            reasons.Add("complex terrain; avalanche data unavailable — verify Varsom before going");
        }

        // Weather contributions are independent.
        var precip6h = w.PrecipitationNext6hMm ?? 0;
        if (precip6h > 10 && w.WindSpeedMs > 12)
        { s -= 35; reasons.Add($"{precip6h:F0}mm/6h + strong wind: wind loading on lee aspects"); }
        else if (precip6h > 10) { reasons.Add($"fresh snow ({precip6h:F0}mm/6h)"); }

        if (w.WindSpeedMs > 18) { s -= 30; reasons.Add($"ridge wind {w.WindSpeedMs:F0} m/s — exposed ridges dangerous"); }
        else if (w.WindSpeedMs > 12) { s -= 15; reasons.Add($"fresh wind ({w.WindSpeedMs:F0} m/s)"); }

        if (w.AirTemperatureCelsius > 3) { s -= 15; reasons.Add($"{w.AirTemperatureCelsius:F0}°C — wet snow / glide"); }
        if (w.AirPressureHpa < 990) { s -= 10; reasons.Add("very low pressure — system moving through"); }

        var score = (int)Math.Clamp(Math.Round(s), 0, 100);
        var rationale = reasons.Count == 0
            ? a is not null
                ? $"Stable weather; avalanche level {a.DangerLevel} ({a.Summary})."
                : "Stable weather; check Varsom for the avalanche bulletin before heading out."
            : a is not null
                ? $"{string.Join(". ", reasons)}."
                : $"{string.Join(". ", reasons)}. Check Varsom for the avalanche bulletin before heading out.";

        return (score == 100 && reasons.Count == 0 ? (int?)null : score, rationale);
    }
}
