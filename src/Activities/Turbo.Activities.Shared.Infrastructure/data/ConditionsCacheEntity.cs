namespace Turboapi.Activities.data.model;

/// <summary>
/// EF-mapped cache row. Lives in the same <c>activities</c> database as
/// the cross-kind summaries so per-kind advisors can share one
/// pre-warmed cache. Composite PK on (provider_key, grid_cell,
/// time_bucket) means every (where, when, what) tuple has at most one
/// row; the writer side uses an UPSERT.
/// </summary>
public class ConditionsCacheEntity
{
    public required string ProviderKey { get; set; }
    public required string GridCell { get; set; }
    public required DateTime TimeBucket { get; set; }
    public required byte[] Payload { get; set; }
    public required DateTime FetchedAt { get; set; }
    public required DateTime ExpiresAt { get; set; }
}
