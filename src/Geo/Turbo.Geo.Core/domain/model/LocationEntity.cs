
using NetTopologySuite.Geometries;

namespace Turboapi.Geo.data.model;

public class LocationEntity
{
    public required Guid Id { get; set; }
    public required Guid OwnerId { get; set; }
    public required Point Geometry { get; set; }
    public required string Name { get; set; }
    public string Description { get; set; } = string.Empty;
    public string Icon { get; set; } = string.Empty;
    public DateTime CreatedAt { get; set; }

    /// <summary>
    /// Server-stamped on every projection write. Drives the
    /// <c>?since=</c> delta endpoint and the <c>ETag</c>/<c>If-Match</c>
    /// optimistic-concurrency contract.
    /// </summary>
    public DateTime UpdatedAt { get; set; }

    /// <summary>
    /// Tombstone for soft-delete. The read model retains the row so a
    /// client that synced before the delete can learn about it on its
    /// next pull.
    /// </summary>
    public DateTime? DeletedAt { get; set; }

    /// <summary>
    /// Monotonic per-row version. Emitted on reads as <c>ETag</c>; expected
    /// by writes via <c>If-Match</c>.
    /// </summary>
    public long Version { get; set; }
}
