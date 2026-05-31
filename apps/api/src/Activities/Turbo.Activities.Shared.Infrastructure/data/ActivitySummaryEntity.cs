using NetTopologySuite.Geometries;

namespace Turboapi.Activities.data.model;

/// <summary>
/// Cross-kind denormalized projection used by map-viewport and delta-sync
/// queries. The source of truth for a given activity lives in that kind's
/// own typed table (e.g. <c>fishing.activities</c>); this row is rebuilt by
/// a subscriber to <see cref="events.ActivitySummaryUpserted"/>.
/// </summary>
public class ActivitySummaryEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }
    public required string Kind { get; set; }
    public required string Name { get; set; }
    public required Geometry Geometry { get; set; }
    public required string IconKey { get; set; }
    public string? ColorHex { get; set; }

    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public DateTime? DeletedAt { get; set; }
    public long Version { get; set; }

    /// <summary>0–100 composite score from the orchestrator's last
    /// fetched analysis. Written back opportunistically (every analysis
    /// fetch + the snapshotter's rotation); <c>null</c> means no recent
    /// score is known.</summary>
    public int? SummaryScore { get; set; }

    /// <summary>When [SummaryScore] was last written. Clients use this
    /// to decide whether to render a score halo at all (e.g. ignore
    /// anything older than ~3h).</summary>
    public DateTime? SummaryScoreAt { get; set; }

    /// <summary>Localized label of the highest-weight driver from the
    /// last analysis (e.g. "Avalanche danger considerable",
    /// "Fresh groomed"). Used for the recommendation card's headline.
    /// Truncated to 64 chars upstream.</summary>
    public string? TopDriverLabel { get; set; }
}
