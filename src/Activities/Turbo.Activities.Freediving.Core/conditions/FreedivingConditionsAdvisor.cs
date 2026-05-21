using Turboapi.Activities.Freediving.domain;
using Turboapi.Activities.Freediving.value;
using Turboapi.Activities.value;
using ITideProvider = Turboapi.Activities.value.ITideProvider;
using TideSlice = Turboapi.Activities.value.TideSlice;

namespace Turboapi.Activities.Freediving.conditions;

public interface IFreedivingConditionsAdvisor
{
    Task<FreedivingConditionsReport> AdviseAsync(
        FreedivingActivity activity, DateTimeOffset at, CancellationToken cancellationToken);
}

/// <summary>
/// Freediving advisor. Weather drives surface conditions and visibility:
/// wind chops the surface and stirs sediment; recent precipitation cuts
/// visibility on shore entries (river outflow). When the
/// <see cref="ITideProvider"/> lands the report will fill in
/// <see cref="FreedivingConditionsReport.SeaStateSummary"/>.
/// </summary>
public sealed class FreedivingConditionsAdvisor : IFreedivingConditionsAdvisor
{
    private readonly IWeatherProvider _weather;
    private readonly ITideProvider? _tide;
    private readonly TimeProvider _clock;

    public FreedivingConditionsAdvisor(
        IWeatherProvider weather,
        ITideProvider? tide = null,
        TimeProvider? clock = null)
    {
        _weather = weather;
        _tide = tide;
        _clock = clock ?? TimeProvider.System;
    }

    public async Task<FreedivingConditionsReport> AdviseAsync(
        FreedivingActivity activity, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var p = activity.Position;
        var w = await _weather.GetAsync(p.Y, p.X, at, cancellationToken);

        TideSlice? tide = null;
        if (_tide is not null && activity.Details.WaterBody == WaterBody.Sea)
        {
            try { tide = await _tide.GetAsync(p.Y, p.X, at, cancellationToken); }
            catch { /* soft */ }
        }

        var (score, rationale) = ScoreAndRationale(w, tide, activity.Details);
        return new FreedivingConditionsReport(
            activity.Core.Id, w.ValidAt, _clock.GetUtcNow(),
            w, tide?.Summary, score, rationale);
    }

    private static (int? score, string rationale) ScoreAndRationale(
        WeatherSlice w, TideSlice? tide, FreedivingDetails d)
    {
        var s = 100.0;
        var reasons = new List<string>();

        // Wind = chop = visibility + surface comfort.
        if (w.WindSpeedMs > 10) { s -= 30; reasons.Add($"choppy surface ({w.WindSpeedMs:F0} m/s)"); }
        else if (w.WindSpeedMs > 6) { s -= 15; reasons.Add("light chop"); }

        // Rain in the last hours = murky water near shore (river runoff,
        // particulates). Worse on shore entries with kelp / sediment beds.
        var p1h = w.PrecipitationNext1hMm ?? 0;
        if (p1h > 2 && d.ShoreEntry)
        {
            s -= 15;
            reasons.Add("rain — runoff cuts shore-entry visibility");
        }

        // Cold air doesn't really hit a freediver — water temp does. Lacking
        // that data, we mention it only when it's really cold.
        if (w.AirTemperatureCelsius < -5)
        {
            reasons.Add("very cold air — wetsuit warmup matters");
        }

        if (tide is not null && tide.Summary is { Length: > 0 })
        {
            reasons.Add($"sea state: {tide.Summary}");
        }
        else if (d.WaterBody == WaterBody.Sea)
        {
            reasons.Add("tide data not yet available — check Sehavnivå before going");
        }

        var score = (int)Math.Clamp(Math.Round(s), 0, 100);
        var rationale = reasons.Count == 0
            ? "Calm surface — good visibility likely."
            : $"Conditions: {string.Join(", ", reasons)}.";
        return (score == 100 && reasons.Count == 0 ? (int?)null : score, rationale);
    }
}
