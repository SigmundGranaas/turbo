using System.Globalization;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.RateLimiting;
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
[EnableRateLimiting(PlacesModule.RateLimitPolicy)]
public class PlacesController : ControllerBase
{
    // Mainland Norway envelope (generous): reject obviously out-of-scope
    // coords/centres early instead of running spatial queries for London.
    private const double MinLat = 57.0, MaxLat = 72.5, MinLng = 4.0, MaxLng = 32.0;

    private readonly ReverseGeocodeService _reverse;
    private readonly SearchService _search;
    private readonly IPlaceStore _store;
    private readonly DatasetVersionProvider _version;
    private readonly RulesetProvider _ruleset;
    private readonly Turboapi.Places.Infrastructure.BundleBuilder _bundles;

    public PlacesController(
        ReverseGeocodeService reverse, SearchService search, IPlaceStore store,
        DatasetVersionProvider version, RulesetProvider ruleset,
        Turboapi.Places.Infrastructure.BundleBuilder bundles)
    {
        _reverse = reverse;
        _search = search;
        _store = store;
        _version = version;
        _ruleset = ruleset;
        _bundles = bundles;
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

    /// <summary>GET /api/places/ruleset/{version} — the classification ruleset
    /// the core ran for that version (bundles embed the same artifact).</summary>
    [HttpGet("ruleset/{version}")]
    public ActionResult Ruleset(string version)
    {
        var json = _ruleset.ForVersion(version);
        return json is null
            ? NotFound(new ErrorResponse("unknown_ruleset", $"No ruleset version '{version}'."))
            : Content(json, "application/json");
    }

    /// <summary>GET /api/places/bundle?bbox=minLng,minLat,maxLng,maxLat&amp;since=
    /// — an offline SQLite region bundle (R*Tree + polygon containment + the
    /// ruleset), so the on-device engine answers identically offline. 304 when
    /// the client's <c>since</c> already matches the active dataset version.</summary>
    [HttpGet("bundle")]
    public async Task<IActionResult> Bundle(
        [FromQuery] string bbox, [FromQuery] string? since, CancellationToken ct)
    {
        var parts = (bbox ?? "").Split(',');
        if (parts.Length != 4 ||
            !double.TryParse(parts[0], NumberStyles.Float, CultureInfo.InvariantCulture, out var minLng) ||
            !double.TryParse(parts[1], NumberStyles.Float, CultureInfo.InvariantCulture, out var minLat) ||
            !double.TryParse(parts[2], NumberStyles.Float, CultureInfo.InvariantCulture, out var maxLng) ||
            !double.TryParse(parts[3], NumberStyles.Float, CultureInfo.InvariantCulture, out var maxLat))
        {
            return BadRequest(new ErrorResponse("bad_bbox", "bbox must be minLng,minLat,maxLng,maxLat."));
        }
        if (!InNorway(minLat, minLng) || !InNorway(maxLat, maxLng))
            return BadRequest(new ErrorResponse("out_of_coverage", "bbox is outside Norway."));
        if (maxLat <= minLat || maxLng <= minLng)
            return BadRequest(new ErrorResponse("bad_bbox", "max must be greater than min."));
        // Cap the on-demand bundle to a region: a whole-country slice would
        // build ~1M rows into SQLite + stream it on every request (a DoS lever).
        // Larger areas are produced offline by the `bundle` ingestion job.
        if (maxLat - minLat > MaxBundleLatSpan || maxLng - minLng > MaxBundleLngSpan)
            return BadRequest(new ErrorResponse("bbox_too_large",
                $"bbox spans at most {MaxBundleLatSpan}° lat × {MaxBundleLngSpan}° lng; " +
                "use the offline bundle job for larger areas."));

        var version = await _version.GetActiveVersionAsync(ct);
        if (version is null)
            return NotFound(new ErrorResponse("no_dataset", "No active dataset published."));
        if (since == version)
            return StatusCode(StatusCodes.Status304NotModified);

        var path = Path.Combine(Path.GetTempPath(), $"places-bundle-{Guid.NewGuid():n}.sqlite");
        try
        {
            await _bundles.BuildAsync(minLng, minLat, maxLng, maxLat, _ruleset.Json, version, path, ct);
        }
        catch
        {
            if (System.IO.File.Exists(path)) System.IO.File.Delete(path);
            throw;
        }

        // Stream the file (never load it fully into memory) and have the OS
        // unlink it once the response finishes (DeleteOnClose).
        var stream = new FileStream(
            path, FileMode.Open, FileAccess.Read, FileShare.None,
            bufferSize: 64 * 1024, FileOptions.Asynchronous | FileOptions.DeleteOnClose);
        Response.Headers.ETag = $"\"{version}\"";
        return File(stream, "application/octet-stream", $"places-{version}.sqlite");
    }

    // Region cap for the on-demand bundle endpoint (≈ a large fylke). Bigger
    // areas go through the offline `bundle` job, not a request thread.
    private const double MaxBundleLatSpan = 2.5;
    private const double MaxBundleLngSpan = 6.0;

    /// <summary>Data licence/attribution surfaced on /health and in bundles.</summary>
    public const string Attribution = "© Kartverket / Miljødirektoratet (NLOD)";

    /// <summary>GET /api/places/health — dataset freshness + attribution.</summary>
    [HttpGet("health")]
    public async Task<ActionResult<PlacesHealthResponse>> Health(CancellationToken ct)
    {
        var (places, areas, version) = await _store.StatsAsync(ct);
        return Ok(new PlacesHealthResponse(places, areas, version, Attribution));
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
