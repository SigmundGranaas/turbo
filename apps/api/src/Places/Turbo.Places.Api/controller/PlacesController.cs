using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Places.controller.response;
using Turboapi.Places.Core;

namespace Turboapi.Places.controller;

/// <summary>
/// Search + reverse-geocoding over the owned reference datasets. Anonymous by
/// design — this is public reference data with no per-user state; protect at
/// the gateway (app token / rate limit), not per user (plan §7).
/// </summary>
[ApiController]
[Route("api/places")]
[AllowAnonymous]
public class PlacesController : ControllerBase
{
    // Mainland Norway envelope (generous): reject obviously out-of-scope
    // coords/centres early instead of running spatial queries for London.
    private const double MinLat = 57.0, MaxLat = 72.5, MinLng = 4.0, MaxLng = 32.0;

    private readonly ReverseGeocodeService _reverse;
    private readonly SearchService _search;
    private readonly IPlaceStore _store;
    private readonly DatasetVersionProvider _version;

    public PlacesController(
        ReverseGeocodeService reverse, SearchService search, IPlaceStore store,
        DatasetVersionProvider version)
    {
        _reverse = reverse;
        _search = search;
        _store = store;
        _version = version;
    }

    /// <summary>GET /api/places/reverse?lat=&amp;lon= — describe a coordinate.
    /// Responses are deterministic per dataset version, so they carry an ETag
    /// (= dataset version) and honour If-None-Match with 304 — cacheable at
    /// the gateway/CDN until the next ingestion run.</summary>
    [HttpGet("reverse")]
    public async Task<ActionResult<ReverseResponse>> Reverse(
        [FromQuery] double lat, [FromQuery] double lon, CancellationToken ct)
    {
        if (!InNorway(lat, lon))
            return BadRequest(new ErrorResponse("out_of_coverage", "Coordinate is outside Norway."));

        if (await NotModifiedAsync(ct)) return StatusCode(StatusCodes.Status304NotModified);

        var d = await _reverse.DescribeAsync(lat, lon, ct);
        if (d is null)
            return NotFound(new ErrorResponse("no_description", "No source produced a usable label."));

        return Ok(new ReverseResponse(
            d.Title, d.Qualifier, d.Secondary, d.Kommune, d.Fylke, d.DistanceM, d.ElevationM));
    }

    /// <summary>GET /api/places/search?q=&amp;lat=&amp;lon=&amp;limit= — ranked
    /// forward search, proximity-biased when a map centre is given.</summary>
    [HttpGet("search")]
    public async Task<ActionResult<SearchResponse>> Search(
        [FromQuery] string q,
        [FromQuery] double? lat,
        [FromQuery] double? lon,
        [FromQuery] int limit = 10,
        CancellationToken ct = default)
    {
        if (string.IsNullOrWhiteSpace(q))
            return BadRequest(new ErrorResponse("empty_query", "q must be non-empty."));
        if (lat.HasValue != lon.HasValue)
            return BadRequest(new ErrorResponse("partial_centre", "Provide both lat and lon, or neither."));
        if (lat.HasValue && !InNorway(lat.Value, lon!.Value))
            return BadRequest(new ErrorResponse("out_of_coverage", "Centre is outside Norway."));
        limit = Math.Clamp(limit, 1, 50);

        if (await NotModifiedAsync(ct)) return StatusCode(StatusCodes.Status304NotModified);

        var results = await _search.SearchAsync(q, lat, lon, limit, ct);
        return Ok(new SearchResponse(
            results.Select(r => new SearchHitResponse(
                r.Title, r.Description, r.Icon, r.Lat, r.Lng)).ToList()));
    }

    /// <summary>GET /api/places/health — dataset freshness for ops + clients.</summary>
    [HttpGet("health")]
    public async Task<ActionResult<PlacesHealthResponse>> Health(CancellationToken ct)
    {
        var (places, areas, version) = await _store.StatsAsync(ct);
        return Ok(new PlacesHealthResponse(places, areas, version));
    }

    private static bool InNorway(double lat, double lng) =>
        lat is >= MinLat and <= MaxLat && lng is >= MinLng and <= MaxLng;

    /// <summary>Stamps the dataset-version ETag on the response and answers
    /// whether the client's If-None-Match already matches it.</summary>
    private async Task<bool> NotModifiedAsync(CancellationToken ct)
    {
        var version = await _version.GetActiveVersionAsync(ct);
        if (version is null) return false;
        var etag = $"\"{version}\"";
        Response.Headers.ETag = etag;
        return Request.Headers.IfNoneMatch.Any(v => v == etag);
    }
}
