using System.Text.Json;

namespace Turboapi.Activities.data.model;

/// <summary>
/// One user-contributed observation on an activity. <see cref="KindPayload"/>
/// is a kind-specific jsonb document — freediving carries observed visibility
/// + water temp; xc ski carries track condition; backcountry carries snow
/// quality + concerns. The store stays generic; per-kind contracts type the
/// payload in their own assemblies.
/// </summary>
public class ActivityObservationEntity
{
    public required Guid Id { get; set; }
    public required Guid ActivityId { get; set; }
    public required Guid UserId { get; set; }
    public required DateTime ObservedAt { get; set; }
    public required string Kind { get; set; }
    public short? Rating { get; set; }
    public string? Comment { get; set; }
    public required JsonDocument KindPayload { get; set; }
    public short PhotoCount { get; set; }
    public DateTime CreatedAt { get; set; }

    /// <summary>Watershed correlate copied from the activity's geo context
    /// at write time. Lets orchestrators query observations by watershed
    /// without joining the geo-context table on the hot path. Nullable —
    /// activities without geo context yet write null.</summary>
    public string? WatershedHrefId { get; set; }
}

/// <summary>
/// Lightweight "I was here" log. No payload — only presence at a time.
/// Personalizes orchestrator messaging and rate-limits noisy observation
/// posting.
/// </summary>
public class ActivityVisitEntity
{
    public required Guid Id { get; set; }
    public required Guid ActivityId { get; set; }
    public required Guid UserId { get; set; }
    public required DateTime VisitedAt { get; set; }

    /// <summary><c>manual</c>, <c>gps_inferred</c>, <c>track_match</c>.</summary>
    public required string Source { get; set; }

    public DateTime CreatedAt { get; set; }
}
