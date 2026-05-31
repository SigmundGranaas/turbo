namespace Turboapi.Activities.data.model;

/// <summary>
/// Persistent time-series of conditions slice payloads. Unlike the TTL
/// <c>conditions_cache</c> table — which holds at most one row per
/// (provider, grid, time_bucket) and expires on schedule — this table
/// appends a row per successful upstream fetch. The snapshotting decorator
/// writes a row alongside every cache put; orchestrators query the table
/// for percentile and trend signals no upstream API exposes.
/// </summary>
public class ConditionsSnapshotEntity
{
    public long Id { get; set; }
    public required string ProviderKey { get; set; }
    public required string GridCell { get; set; }

    /// <summary>The slice's <c>ValidAt</c>/observation time — not the
    /// time the snapshot was fetched. Trend and percentile queries use
    /// this column.</summary>
    public required DateTime ObservedAt { get; set; }

    public required DateTime FetchedAt { get; set; }
    public required byte[] Payload { get; set; }
    public required short PayloadSchemaVersion { get; set; }
}
