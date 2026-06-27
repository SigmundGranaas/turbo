using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.value;

namespace Turboapi.Activities.controller;

/// <summary>
/// Public point-conditions endpoint. Reuses the shared <see cref="IWeatherProvider"/>
/// (met.no-backed, Postgres-cached) so browser clients — which cannot call
/// api.met.no directly (User-Agent + CORS) — can show weather at a marker /
/// point. Anonymous: weather at a coordinate isn't user-scoped.
/// </summary>
[ApiController]
[Route("api/activities/conditions")]
[AllowAnonymous]
public class ConditionsController : ControllerBase
{
    private readonly IWeatherProvider _weather;
    private readonly ITideProvider _tide;
    private readonly ILogger<ConditionsController> _logger;

    public ConditionsController(IWeatherProvider weather, ITideProvider tide, ILogger<ConditionsController> logger)
    {
        _weather = weather;
        _tide = tide;
        _logger = logger;
    }

    public sealed record ConditionsResponse(
        WeatherSlice Now,
        IReadOnlyList<WeatherSlice> Hourly,
        IReadOnlyList<DaySlice> Daily,
        TideSlice? Tide);

    /// <summary>One calendar day's outlook, rolled up from the hourly/6-hourly
    /// timeseries. Dates are UTC (the client renders them in its own locale).</summary>
    public sealed record DaySlice(DateOnly Date, float HighC, float LowC, float? PrecipMm, string? SymbolCode);

    /// <summary>Weather now + a 24h hourly outlook (every 3h) + a ~7-day daily
    /// outlook at <paramref name="lat"/>/<paramref name="lon"/>. The whole
    /// forecast timeseries is fetched once upstream and sliced here.</summary>
    [HttpGet]
    [ProducesResponseType(typeof(ConditionsResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<ConditionsResponse>> Get(
        [FromQuery] double lat, [FromQuery] double lon, CancellationToken ct)
    {
        if (lat is < -90 or > 90 || lon is < -180 or > 180)
            return BadRequest(new { error = "lat/lon out of range" });

        try
        {
            var now = DateTimeOffset.UtcNow;
            var series = await _weather.GetForecastAsync(lat, lon, ct);
            if (series.Count == 0)
                return StatusCode(StatusCodes.Status502BadGateway, new { error = "Conditions unavailable" });

            var ordered = series.OrderBy(s => s.ValidAt).ToList();
            var nowSlice = Nearest(ordered, now);

            var hourly = new List<WeatherSlice>();
            for (var h = 3; h <= 24; h += 3)
                hourly.Add(Nearest(ordered, now.AddHours(h)));

            var daily = RollUpDaily(ordered, now);

            // Tide / sea-state is coastal-only — null inland, which is fine.
            TideSlice? tide = null;
            try { tide = await _tide.GetAsync(lat, lon, now, ct); }
            catch (Exception ex) { _logger.LogDebug(ex, "No tide for {Lat},{Lon}", lat, lon); }

            return Ok(new ConditionsResponse(nowSlice, hourly, daily, tide));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Conditions fetch failed for {Lat},{Lon}", lat, lon);
            return StatusCode(StatusCodes.Status502BadGateway, new { error = "Conditions unavailable" });
        }
    }

    private static WeatherSlice Nearest(IReadOnlyList<WeatherSlice> ordered, DateTimeOffset at)
    {
        var best = ordered[0];
        var bestDelta = (best.ValidAt - at).Duration();
        foreach (var s in ordered)
        {
            var delta = (s.ValidAt - at).Duration();
            if (delta < bestDelta) { best = s; bestDelta = delta; }
        }
        return best;
    }

    private static IReadOnlyList<DaySlice> RollUpDaily(IReadOnlyList<WeatherSlice> ordered, DateTimeOffset now)
    {
        return ordered
            .GroupBy(s => DateOnly.FromDateTime(s.ValidAt.UtcDateTime))
            .Where(g => g.Key >= DateOnly.FromDateTime(now.UtcDateTime))
            .OrderBy(g => g.Key)
            .Take(7)
            .Select(g =>
            {
                var high = g.Max(s => s.AirTemperatureCelsius);
                var low = g.Min(s => s.AirTemperatureCelsius);

                // Sum precipitation over the canonical, non-overlapping 6h synoptic
                // blocks (00/06/12/18 UTC) so we don't double-count overlapping
                // 1h/6h windows. Fall back to summing 1h windows; null if no data.
                float? precip;
                var sixHour = g.Where(s => s.PrecipitationNext6hMm.HasValue && s.ValidAt.UtcDateTime.Hour % 6 == 0)
                               .Select(s => s.PrecipitationNext6hMm!.Value).ToList();
                if (sixHour.Count > 0)
                    precip = sixHour.Sum();
                else if (g.Any(s => s.PrecipitationNext1hMm.HasValue))
                    precip = g.Sum(s => s.PrecipitationNext1hMm ?? 0f);
                else
                    precip = null;

                // Daytime symbol: the entry nearest local-ish noon (12:00 UTC).
                var noonTarget = new DateTimeOffset(g.Key.ToDateTime(TimeOnly.MinValue), TimeSpan.Zero).AddHours(12);
                var noon = Nearest(g.ToList(), noonTarget);

                return new DaySlice(g.Key, high, low, precip, noon.SymbolCode);
            })
            .ToList();
    }
}
