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

    public sealed record ConditionsResponse(WeatherSlice Now, IReadOnlyList<WeatherSlice> Hourly, TideSlice? Tide);

    /// <summary>Weather now + a 24h outlook (every 3h) at <paramref name="lat"/>/<paramref name="lon"/>.</summary>
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
            var nowSlice = await _weather.GetAsync(lat, lon, now, ct);

            var hourly = new List<WeatherSlice>();
            for (var h = 3; h <= 24; h += 3)
                hourly.Add(await _weather.GetAsync(lat, lon, now.AddHours(h), ct));

            // Tide / sea-state is coastal-only — null inland, which is fine.
            TideSlice? tide = null;
            try { tide = await _tide.GetAsync(lat, lon, now, ct); }
            catch (Exception ex) { _logger.LogDebug(ex, "No tide for {Lat},{Lon}", lat, lon); }

            return Ok(new ConditionsResponse(nowSlice, hourly, tide));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Conditions fetch failed for {Lat},{Lon}", lat, lon);
            return StatusCode(StatusCodes.Status502BadGateway, new { error = "Conditions unavailable" });
        }
    }
}
