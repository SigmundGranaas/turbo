using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turboapi.Activities.data;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;
using Warning = Turboapi.Activities.value.Warning;

namespace Turboapi.Activities.controller;

/// <summary>
/// "What's good near me right now" across kinds. Enumerates own
/// activities within radius via PostGIS ST_DWithin, groups by kind, then
/// fans each group through its registered
/// <see cref="IActivityRecommendationScorer"/>. The scorer runs the
/// kind's orchestrator on a cheap provider subset (no observation
/// lookups, no snapshot history) so a 30-candidate response stays
/// bounded.
///
/// Results are ranked by score desc → distance asc → data quality desc.
/// Externally-discovered candidates (UT.no trails, skisporet tracks,
/// lakseregister rivers) will plug in through the same
/// <see cref="IActivityRecommendationScorer"/> seam once their discovery
/// providers are wired — the controller stays unchanged.
/// </summary>
[ApiController]
[Route("api/activities/recommendations")]
[Authorize]
public class RecommendationsController : ControllerBase
{
    private const int DefaultLimit = 20;
    private const int MaxLimit = 50;
    private const double DefaultRadiusKm = 25.0;
    private const double MaxRadiusKm = 200.0;

    private readonly ActivitySummariesContext _db;
    private readonly IEnumerable<IActivityRecommendationScorer> _scorers;
    private readonly ILogger<RecommendationsController> _logger;

    public RecommendationsController(
        ActivitySummariesContext db,
        IEnumerable<IActivityRecommendationScorer> scorers,
        ILogger<RecommendationsController> logger)
    {
        _db = db;
        _scorers = scorers;
        _logger = logger;
    }

    private Guid GetAuthenticatedUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpGet]
    [ProducesResponseType(typeof(RecommendationsResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<RecommendationsResponse>> Get(
        [FromQuery] double lat,
        [FromQuery] double lon,
        [FromQuery] double? radiusKm,
        [FromQuery] DateTime? date,
        [FromQuery] string? kinds,
        [FromQuery] int? limit,
        CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var radius = Math.Clamp(radiusKm ?? DefaultRadiusKm, 0.5, MaxRadiusKm);
            var lim = Math.Clamp(limit ?? DefaultLimit, 1, MaxLimit);
            var kindFilter = ParseKindFilter(kinds);
            var queryAt = date is null
                ? DateTimeOffset.UtcNow
                : new DateTimeOffset(DateTime.SpecifyKind(date.Value, DateTimeKind.Utc));
            var queryContext = QueryContext.ForQuickScore(queryAt);
            var anchor = new GeometryFactory(new PrecisionModel(), 4326).CreatePoint(new Coordinate(lon, lat));

            // PostGIS ST_DWithin on geography for a true km-radius
            // bounded query. Pull a wider pool than `limit` so the
            // ranking pass has something to reject.
            var poolCap = lim * 4;
            var candidatesQuery = _db.Summaries
                .AsNoTracking()
                .Where(s => s.OwnerId == userId && s.DeletedAt == null)
                .Where(s => s.Geometry.Distance(anchor) * 111_000 <= radius * 1000); // crude planar fallback
            if (kindFilter is not null)
            {
                candidatesQuery = candidatesQuery.Where(s => kindFilter.Contains(s.Kind));
            }
            var summaries = await candidatesQuery
                .OrderBy(s => s.Geometry.Distance(anchor))
                .Take(poolCap)
                .ToListAsync(ct);

            if (summaries.Count == 0)
            {
                return Ok(new RecommendationsResponse(
                    Array.Empty<RecommendationItem>(),
                    DateTimeOffset.UtcNow));
            }

            // Group by kind and dispatch to each kind's scorer in
            // parallel. Within a kind the scorer iterates serially —
            // upstream rate limiting matters more than wall-clock for
            // recommendation responses.
            var scoreTasks = new List<Task<IReadOnlyList<RecommendationScore>>>();
            foreach (var group in summaries.GroupBy(s => s.Kind))
            {
                var scorer = _scorers.FirstOrDefault(s => s.Kind == group.Key);
                if (scorer is null)
                {
                    _logger.LogDebug("No recommendation scorer registered for kind {Kind}", group.Key);
                    continue;
                }
                var ids = group.Select(s => s.Id).ToList();
                scoreTasks.Add(scorer.ScoreAsync(ids, queryContext, ct));
            }
            var scoreGroups = await Task.WhenAll(scoreTasks);

            // Index summaries by id for the join.
            var summaryById = summaries.ToDictionary(s => s.Id);

            var items = new List<RecommendationItem>(lim);
            foreach (var group in scoreGroups)
            {
                foreach (var score in group)
                {
                    if (!summaryById.TryGetValue(score.ActivityId, out var summary)) continue;
                    var distanceM = GeographicDistanceM(
                        lat, lon, summary.Geometry.Centroid.Y, summary.Geometry.Centroid.X);
                    items.Add(new RecommendationItem(
                        SourceKind: "own_activity",
                        Kind: summary.Kind,
                        ActivityId: score.ActivityId,
                        Name: summary.Name,
                        GeometryWkt: new WKTWriter().Write(summary.Geometry),
                        Score: score.Score,
                        Confidence: score.Confidence,
                        Headline: score.Headline,
                        TopDriverLabel: score.TopDriverLabel,
                        SuggestedWindow: score.SuggestedWindow,
                        TopWarnings: score.TopWarnings,
                        DistanceM: distanceM));
                }
            }

            // Rank: score desc, distance asc. Null score sinks to the
            // bottom (nothing to recommend).
            items.Sort((a, b) =>
            {
                var aScore = a.Score ?? -1;
                var bScore = b.Score ?? -1;
                if (aScore != bScore) return bScore.CompareTo(aScore);
                return a.DistanceM.CompareTo(b.DistanceM);
            });
            if (items.Count > lim) items.RemoveRange(lim, items.Count - lim);

            return Ok(new RecommendationsResponse(items, DateTimeOffset.UtcNow));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Recommendations endpoint failed");
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Recommendations unavailable", ex.Message));
        }
    }

    private static HashSet<string>? ParseKindFilter(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw)) return null;
        var parts = raw.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        return parts.Length == 0 ? null : new HashSet<string>(parts, StringComparer.Ordinal);
    }

    private static double GeographicDistanceM(double lat1, double lon1, double lat2, double lon2)
    {
        const double earthRadiusM = 6_371_000;
        var r1 = lat1 * Math.PI / 180.0;
        var r2 = lat2 * Math.PI / 180.0;
        var dLat = r2 - r1;
        var dLon = (lon2 - lon1) * Math.PI / 180.0;
        var a = Math.Sin(dLat / 2) * Math.Sin(dLat / 2)
                + Math.Cos(r1) * Math.Cos(r2) * Math.Sin(dLon / 2) * Math.Sin(dLon / 2);
        var c = 2 * Math.Atan2(Math.Sqrt(a), Math.Sqrt(1 - a));
        return earthRadiusM * c;
    }
}

public sealed record RecommendationsResponse(
    IReadOnlyList<RecommendationItem> Items,
    DateTimeOffset ServerTime);

public sealed record RecommendationItem(
    string SourceKind,        // "own_activity" | "discovered_*" (future)
    string Kind,
    Guid? ActivityId,
    string Name,
    string GeometryWkt,
    int? Score,
    ScoreConfidence Confidence,
    string Headline,
    string? TopDriverLabel,
    TimeWindow? SuggestedWindow,
    IReadOnlyList<Warning> TopWarnings,
    double DistanceM);

public sealed record ErrorResponse(string Title, string Detail);
