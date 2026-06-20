using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.RateLimiting;
using Turboapi.Places.controller.response;
using Turboapi.Places.Core;

namespace Turboapi.Places.controller;

/// <summary>
/// Read-only proxy to the open Nasjonal Turbase API (ut.no / DNT). Injects the
/// secret <c>api_key</c> server-side so mobile clients never hold it, and
/// returns normalised DTOs. Mounted under <c>/api/places/ntb</c> so it rides the
/// existing Places gateway route — no extra cluster/route wiring. Anonymous +
/// rate-limited like the rest of Places (public reference data, no per-user
/// state).
/// </summary>
[ApiController]
[Route("api/places/ntb")]
[AllowAnonymous]
[EnableRateLimiting(PlacesModule.RateLimitPolicy)]
public class TurbasenController : ControllerBase
{
    // Mainland Norway envelope — reject obviously out-of-scope viewports early.
    private const double MinLat = 57.0, MaxLat = 72.5, MinLng = 4.0, MaxLng = 32.0;

    // Cap the viewport span so a zoomed-out request can't ask for the country.
    private const double MaxLatSpan = 3.0, MaxLngSpan = 6.0;

    private readonly NasjonalTurbaseProxyClient _client;

    public TurbasenController(NasjonalTurbaseProxyClient client) => _client = client;

    /// <summary>GET /api/places/ntb/pois?minLat=&amp;minLon=&amp;maxLat=&amp;maxLon=
    /// — cabins, places and trip markers within the viewport bbox.</summary>
    [HttpGet("pois")]
    public async Task<ActionResult<NtbPoisResponse>> Pois(
        [FromQuery] double minLat, [FromQuery] double minLon,
        [FromQuery] double maxLat, [FromQuery] double maxLon,
        CancellationToken ct)
    {
        if (maxLat <= minLat || maxLon <= minLon)
            return BadRequest(new ErrorResponse("bad_bbox", "max must be greater than min."));
        if (!InNorway(minLat, minLon) || !InNorway(maxLat, maxLon))
            return BadRequest(new ErrorResponse("out_of_coverage", "bbox is outside Norway."));
        if (maxLat - minLat > MaxLatSpan || maxLon - minLon > MaxLngSpan)
            return BadRequest(new ErrorResponse("bbox_too_large",
                $"bbox spans at most {MaxLatSpan}° lat × {MaxLngSpan}° lng."));

        var pois = await _client.FetchPoisAsync(minLat, minLon, maxLat, maxLon, ct: ct);
        return Ok(new NtbPoisResponse(pois));
    }

    /// <summary>GET /api/places/ntb/route/{id} — one trip's route polyline +
    /// metadata, for the animated reveal.</summary>
    [HttpGet("route/{id}")]
    public async Task<ActionResult<NtbRoute>> Route(string id, CancellationToken ct)
    {
        if (string.IsNullOrWhiteSpace(id))
            return BadRequest(new ErrorResponse("bad_id", "id must be non-empty."));
        var route = await _client.FetchRouteAsync(id, ct);
        return route is null
            ? NotFound(new ErrorResponse("no_route", "No route for that id."))
            : Ok(route);
    }

    private static bool InNorway(double lat, double lng) =>
        lat is >= MinLat and <= MaxLat && lng is >= MinLng and <= MaxLng;
}
