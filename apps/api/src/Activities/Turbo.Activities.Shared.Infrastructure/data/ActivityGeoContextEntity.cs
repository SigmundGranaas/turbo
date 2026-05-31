using System.Text.Json;

namespace Turboapi.Activities.data.model;

/// <summary>
/// EF row for an activity's geometry-derived attributes. Mirrors
/// <see cref="domain.services.ActivityGeoContext"/>. The full record lives
/// in <see cref="Payload"/> (jsonb) so the schema can evolve without
/// migrations on per-field additions; the unindexed columns up top
/// (geometry hash, version, computed_at) cover the recompute-only-on-change
/// path.
/// </summary>
public class ActivityGeoContextEntity
{
    public required Guid ActivityId { get; set; }
    public required int Version { get; set; }

    /// <summary>SHA-256 of the activity geometry's WKB. The compute path
    /// short-circuits when the existing row's hash matches the new
    /// geometry's hash.</summary>
    public required string GeomHash { get; set; }

    public required JsonDocument Payload { get; set; }
    public required DateTime ComputedAt { get; set; }
}
