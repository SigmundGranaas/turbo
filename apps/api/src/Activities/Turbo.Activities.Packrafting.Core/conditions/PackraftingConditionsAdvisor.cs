using Turboapi.Activities.Packrafting.domain;
using Turboapi.Activities.Packrafting.value;
using Turboapi.Activities.value;
using IRiverFlowProvider = Turboapi.Activities.value.IRiverFlowProvider;
using RiverFlowSlice = Turboapi.Activities.value.RiverFlowSlice;

namespace Turboapi.Activities.Packrafting.conditions;

public interface IPackraftingConditionsAdvisor
{
    Task<PackraftingConditionsReport> AdviseAsync(
        PackraftingActivity activity, DateTimeOffset at, CancellationToken cancellationToken);
}

/// <summary>
/// Packrafting advisor. Weather-only today. Once
/// <see cref="IRiverFlowProvider"/> ships and is wired in, this advisor
/// will compose flow + weather: high flow + cold air = serious hypothermia
/// risk on a swim, flow outside the activity's
/// <see cref="PackraftingDetails.MinFlowCumecs"/>/<c>MaxFlowCumecs</c>
/// window penalizes the score, etc.
///
/// The <c>currentFlowCumecs</c> + <c>flowTrend</c> fields on the report
/// are already part of the wire shape so the client can render them
/// without a coordinated release.
/// </summary>
public sealed class PackraftingConditionsAdvisor : IPackraftingConditionsAdvisor
{
    private readonly IWeatherProvider _weather;
    private readonly IRiverFlowProvider? _flow;
    private readonly TimeProvider _clock;

    public PackraftingConditionsAdvisor(
        IWeatherProvider weather,
        IRiverFlowProvider? flow = null,
        TimeProvider? clock = null)
    {
        _weather = weather;
        _flow = flow;
        _clock = clock ?? TimeProvider.System;
    }

    public async Task<PackraftingConditionsReport> AdviseAsync(
        PackraftingActivity activity, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var line = activity.Route;
        var mid = line.Coordinates[line.NumPoints / 2];
        var w = await _weather.GetAsync(mid.Y, mid.X, at, cancellationToken);

        RiverFlowSlice? flow = null;
        if (_flow is not null && activity.Details.NveStationCode is { } code)
        {
            try { flow = await _flow.GetAsync(code, at, cancellationToken); }
            catch { /* soft: a missing flow shouldn't fail the report */ }
        }

        var (score, rationale) = ScoreAndRationale(w, flow, activity.Details);
        return new PackraftingConditionsReport(
            activity.Core.Id, w.ValidAt, _clock.GetUtcNow(),
            w, flow?.CurrentCumecs, flow?.Trend, score, rationale);
    }

    private static (int? score, string rationale) ScoreAndRationale(
        WeatherSlice w, RiverFlowSlice? flow, PackraftingDetails d)
    {
        var s = 100.0;
        var reasons = new List<string>();

        // Cold + wet is the hypothermia risk on a swim.
        if (w.AirTemperatureCelsius < 5 && (w.PrecipitationNext1hMm ?? 0) > 1)
        {
            s -= 25;
            reasons.Add($"cold ({w.AirTemperatureCelsius:F0}°C) + wet — swim consequences");
        }
        else if (w.AirTemperatureCelsius < 0)
        {
            s -= 15;
            reasons.Add($"sub-zero ({w.AirTemperatureCelsius:F0}°C) — bring drysuit");
        }

        if (w.WindSpeedMs > 12) { s -= 15; reasons.Add("strong wind — surface chop"); }

        // Flow window: outside the user's safe range = penalty.
        if (flow is not null)
        {
            if (d.MinFlowCumecs is { } min && flow.CurrentCumecs < min)
            {
                s -= 25;
                reasons.Add($"flow {flow.CurrentCumecs:F1} m³/s below your minimum ({min:F0})");
            }
            else if (d.MaxFlowCumecs is { } max && flow.CurrentCumecs > max)
            {
                s -= 30;
                reasons.Add($"flow {flow.CurrentCumecs:F1} m³/s above your maximum ({max:F0})");
            }
            if (flow.Trend == "rising" && (w.PrecipitationNext6hMm ?? 0) > 10)
            {
                s -= 10;
                reasons.Add("flow rising + rain forecast — levels will climb");
            }
        }
        else if (d.NveStationCode is not null)
        {
            reasons.Add("river flow data not yet available — check NVE before launching");
        }

        var score = (int)Math.Clamp(Math.Round(s), 0, 100);
        var rationale = reasons.Count == 0
            ? "Conditions look reasonable; double-check the put-in before committing."
            : $"Conditions: {string.Join(", ", reasons)}.";
        return (score == 100 && reasons.Count == 0 ? (int?)null : score, rationale);
    }
}
