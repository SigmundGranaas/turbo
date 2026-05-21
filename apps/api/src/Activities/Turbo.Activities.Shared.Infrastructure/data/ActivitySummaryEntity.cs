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
}
