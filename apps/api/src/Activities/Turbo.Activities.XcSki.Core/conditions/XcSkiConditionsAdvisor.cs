using Turboapi.Activities.value;
using Turboapi.Activities.XcSki.domain;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.conditions;

public interface IXcSkiConditionsAdvisor
{
    Task<XcSkiConditionsReport> AdviseAsync(
        XcSkiActivity activity, DateTimeOffset at, CancellationToken cancellationToken);
}

/// <summary>
/// XC ski advisor. Cold + dry + recently groomed is the sweet spot.
/// Composes weather + (optional) live grooming feed: when the
/// IGroomingProvider returns fresh data for the activity's stored
/// feed key, the live hours-ago count overrides the stored
/// GroomingStatus enum for scoring.
/// </summary>
public sealed class XcSkiConditionsAdvisor : IXcSkiConditionsAdvisor
{
    private readonly IWeatherProvider _weather;
    private readonly IGroomingProvider? _grooming;
    private readonly TimeProvider _clock;

    public XcSkiConditionsAdvisor(
        IWeatherProvider weather,
        IGroomingProvider? grooming = null,
        TimeProvider? clock = null)
    {
        _weather = weather;
        _grooming = grooming;
        _clock = clock ?? TimeProvider.System;
    }

    public async Task<XcSkiConditionsReport> AdviseAsync(
        XcSkiActivity activity, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var line = activity.Route;
        var mid = line.Coordinates[line.NumPoints / 2];
        var w = await _weather.GetAsync(mid.Y, mid.X, at, cancellationToken);

        GroomingSlice? live = null;
        if (_grooming is not null && activity.Details.GroomingFeedKey is { } key)
        {
            try { live = await _grooming.GetAsync(key, at, cancellationToken); }
            catch { /* soft */ }
        }

        var (score, rationale) = ScoreAndRationale(w, live, activity.Details);
        return new XcSkiConditionsReport(
            activity.Core.Id, w.ValidAt, _clock.GetUtcNow(),
            w, liveGroomingHoursAgo: live?.HoursAgo, score, rationale);
    }

    private static (int? score, string rationale) ScoreAndRationale(
        WeatherSlice w, GroomingSlice? live, XcSkiDetails d)
    {
        var s = 100.0;
        var reasons = new List<string>();

        if (w.AirTemperatureCelsius > 3)
        { s -= 40; reasons.Add($"{w.AirTemperatureCelsius:F0}°C — track is melting / icy"); }
        else if (w.AirTemperatureCelsius > 0)
        { s -= 15; reasons.Add($"{w.AirTemperatureCelsius:F0}°C — sticky snow likely"); }
        else if (w.AirTemperatureCelsius < -15)
        { s -= 10; reasons.Add($"{w.AirTemperatureCelsius:F0}°C — bring serious wax"); }

        if (w.WindSpeedMs > 12) { s -= 15; reasons.Add($"strong wind ({w.WindSpeedMs:F0} m/s)"); }

        var p1h = w.PrecipitationNext1hMm ?? 0;
        if (p1h > 2 && w.AirTemperatureCelsius > 0)
        { s -= 20; reasons.Add("rain on track — slushy"); }

        // Live grooming overrides the stored estimate when available.
        if (live is not null)
        {
            if (live.HoursAgo > 72) { s -= 25; reasons.Add($"last groomed {live.HoursAgo / 24}d ago"); }
            else if (live.HoursAgo > 36) { s -= 10; reasons.Add($"last groomed {live.HoursAgo}h ago"); }
            else if (live.HoursAgo < 6) reasons.Add("freshly groomed");
        }
        else
        {
            switch (d.GroomingStatus)
            {
                case GroomingStatus.OlderThanTwoDays:
                    s -= 25; reasons.Add("last groomed > 2 days ago (stored estimate)"); break;
                case GroomingStatus.NeverGroomed:
                    s -= 10; reasons.Add("backcountry / never groomed track"); break;
                case GroomingStatus.Yesterday:
                    s -= 5; break;
                default: break;
            }
        }

        var score = (int)Math.Clamp(Math.Round(s), 0, 100);
        var rationale = reasons.Count == 0
            ? "Cold and dry — good track conditions."
            : $"Conditions: {string.Join(", ", reasons)}.";
        return (score == 100 && reasons.Count == 0 ? (int?)null : score, rationale);
    }
}
