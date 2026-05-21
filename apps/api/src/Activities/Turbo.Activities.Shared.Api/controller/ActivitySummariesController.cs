using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turboapi.Activities.data;

namespace Turboapi.Activities.controller;

[ApiController]
[Route("api/activities/summaries")]
[Authorize]
public class ActivitySummariesController : ControllerBase
{
    private const int MaxDeltaLimit = 500;
    private const int MaxBboxLimit = 1000;

    private readonly ActivitySummariesContext _db;
    private readonly ILogger<ActivitySummariesController> _logger;

    public ActivitySummariesController(ActivitySummariesContext db, ILogger<ActivitySummariesController> logger)
    {
        _db = db;
        _logger = logger;
    }

    private Guid GetAuthenticatedUserId()
    {
        var userId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not found in token");
        return Guid.Parse(userId);
    }

    /// <summary>
    /// Map-viewport query. Returns non-deleted summaries owned by the caller
    /// whose geometry intersects the supplied bbox. Optional
    /// <paramref name="kinds"/> filter is a comma-separated kind key list.
    /// </summary>
    [HttpGet("bbox")]
    [ProducesResponseType(typeof(ActivitySummariesResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<ActivitySummariesResponse>> GetByBoundingBox(
        [FromQuery] double minLon,
        [FromQuery] double minLat,
        [FromQuery] double maxLon,
        [FromQuery] double maxLat,
        [FromQuery] string? kinds,
        [FromQuery] string? cursor,
        [FromQuery] int? limit,
        CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var kindFilter = ParseKindFilter(kinds);
            var lim = limit is null ? MaxBboxLimit : Math.Clamp(limit.Value, 1, MaxBboxLimit);
            var cursorPos = BboxCursor.TryParse(cursor);

            var envelope = MakeBoundingBox(minLon, minLat, maxLon, maxLat);

            IQueryable<data.model.ActivitySummaryEntity> q = _db.Summaries
                .Where(s => s.OwnerId == userId && s.DeletedAt == null && s.Geometry.Intersects(envelope));

            if (kindFilter is { Count: > 0 })
                q = q.Where(s => kindFilter.Contains(s.Kind));

            if (cursorPos is { } cur)
            {
                // Stable keyset pagination on (UpdatedAt, Id). Strict
                // ordering — same UpdatedAt resolved by Id — avoids
                // re-emitting / skipping rows when ties occur.
                q = q.Where(s => s.UpdatedAt > cur.UpdatedAt
                              || (s.UpdatedAt == cur.UpdatedAt && s.Id.CompareTo(cur.Id) > 0));
            }

            // Fetch lim+1 to detect truncation without doing a second count.
            var rows = await q.OrderBy(s => s.UpdatedAt).ThenBy(s => s.Id).Take(lim + 1).ToListAsync(ct);

            var truncated = rows.Count > lim;
            if (truncated) rows.RemoveAt(rows.Count - 1);

            string? nextCursor = null;
            if (truncated && rows.Count > 0)
            {
                var last = rows[^1];
                nextCursor = new BboxCursor(last.UpdatedAt, last.Id).Encode();
            }

            return Ok(new ActivitySummariesResponse
            {
                Items = rows.Select(ActivitySummaryItem.From).ToList(),
                ServerTime = DateTime.UtcNow,
                Truncated = truncated,
                NextCursor = nextCursor,
            });
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    /// <summary>
    /// Delta sync. Same shape as Tracks' delta endpoint — items changed
    /// strictly after <paramref name="since"/> plus tombstones.
    /// </summary>
    [HttpGet("changes")]
    [ProducesResponseType(typeof(ActivitySummariesDeltaResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<ActivitySummariesDeltaResponse>> GetChanged(
        [FromQuery] DateTime? since,
        [FromQuery] int? limit,
        CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var cutoff = since?.ToUniversalTime() ?? DateTime.MinValue.ToUniversalTime();
            var lim = limit is null ? MaxDeltaLimit : Math.Clamp(limit.Value, 1, MaxDeltaLimit);

            var rows = await _db.Summaries
                .Where(s => s.OwnerId == userId && s.UpdatedAt > cutoff)
                .OrderBy(s => s.UpdatedAt)
                .Take(lim)
                .ToListAsync(ct);

            return Ok(new ActivitySummariesDeltaResponse
            {
                Items = rows.Where(r => r.DeletedAt == null).Select(ActivitySummaryItem.From).ToList(),
                Deleted = rows.Where(r => r.DeletedAt != null)
                    .Select(r => new TombstoneItem(r.Id, r.Kind, r.DeletedAt!.Value, r.Version))
                    .ToList(),
                ServerTime = DateTime.UtcNow,
            });
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    private static IReadOnlyList<string>? ParseKindFilter(string? kinds)
    {
        if (string.IsNullOrWhiteSpace(kinds)) return null;
        return kinds.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(s => s.Length > 0).ToList();
    }

    private static Polygon MakeBoundingBox(double minLon, double minLat, double maxLon, double maxLat)
    {
        var factory = new GeometryFactory(new PrecisionModel(), 4326);
        var ring = factory.CreateLinearRing(new[]
        {
            new Coordinate(minLon, minLat),
            new Coordinate(maxLon, minLat),
            new Coordinate(maxLon, maxLat),
            new Coordinate(minLon, maxLat),
            new Coordinate(minLon, minLat),
        });
        return factory.CreatePolygon(ring);
    }
}

public sealed record ActivitySummariesResponse
{
    public List<ActivitySummaryItem> Items { get; init; } = new();
    public DateTime ServerTime { get; init; }
    /// <summary>True when the result was capped at the page limit; pass NextCursor back to fetch the rest.</summary>
    public bool Truncated { get; init; }
    /// <summary>Opaque cursor for the next page when Truncated is true; null otherwise.</summary>
    public string? NextCursor { get; init; }
}

public readonly record struct BboxCursor(DateTime UpdatedAt, Guid Id)
{
    public string Encode()
    {
        var raw = $"{UpdatedAt.Ticks:D}:{Id:N}";
        return Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes(raw));
    }

    public static BboxCursor? TryParse(string? cursor)
    {
        if (string.IsNullOrWhiteSpace(cursor)) return null;
        try
        {
            var raw = System.Text.Encoding.UTF8.GetString(Convert.FromBase64String(cursor));
            var parts = raw.Split(':', 2);
            if (parts.Length != 2) return null;
            if (!long.TryParse(parts[0], out var ticks)) return null;
            if (!Guid.TryParse(parts[1], out var id)) return null;
            return new BboxCursor(new DateTime(ticks, DateTimeKind.Utc), id);
        }
        catch (FormatException) { return null; }
    }
}

public sealed record ActivitySummariesDeltaResponse
{
    public List<ActivitySummaryItem> Items { get; init; } = new();
    public List<TombstoneItem> Deleted { get; init; } = new();
    public DateTime ServerTime { get; init; }
}

public sealed record ActivitySummaryItem
{
    public Guid Id { get; init; }
    public string Kind { get; init; } = string.Empty;
    public string Name { get; init; } = string.Empty;
    public string GeometryWkt { get; init; } = string.Empty;
    public string GeometryKind { get; init; } = string.Empty;
    public string IconKey { get; init; } = string.Empty;
    public string? ColorHex { get; init; }
    public DateTime UpdatedAt { get; init; }
    public long Version { get; init; }

    public static ActivitySummaryItem From(data.model.ActivitySummaryEntity e)
    {
        var writer = new WKTWriter();
        return new ActivitySummaryItem
        {
            Id = e.Id,
            Kind = e.Kind,
            Name = e.Name,
            GeometryWkt = writer.Write(e.Geometry),
            GeometryKind = e.Geometry.GeometryType,
            IconKey = e.IconKey,
            ColorHex = e.ColorHex,
            UpdatedAt = e.UpdatedAt,
            Version = e.Version,
        };
    }
}

public sealed record TombstoneItem(Guid Id, string Kind, DateTime DeletedAt, long Version);
