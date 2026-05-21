using Turboapi.Geo.data.model;
using Turboapi.Geo.domain.model;

namespace Turboapi.Geo.domain.query;

public interface ILocationReadRepository
{
    Task<Location?> GetById(Guid id);

    /// <summary>
    /// Returns the raw read-model row, including the sync fields. The
    /// controller uses this to set <c>ETag</c> headers and to run the
    /// <c>If-Match</c> optimistic-concurrency check; aggregate
    /// reconstitution would discard <c>UpdatedAt</c> / <c>DeletedAt</c> /
    /// <c>Version</c>.
    /// </summary>
    Task<LocationEntity?> GetEntityById(Guid id);

    Task<IEnumerable<Location>> GetLocationsInExtent(
        Guid ownerId,
        double minLongitude,
        double minLatitude,
        double maxLongitude,
        double maxLatitude
    );

    /// <summary>
    /// Returns the read-model rows for <paramref name="ownerId"/> whose
    /// <c>UpdatedAt</c> is strictly greater than <paramref name="since"/>.
    /// Includes tombstoned rows (DeletedAt != null) so the client can learn
    /// about deletions.
    /// </summary>
    Task<IEnumerable<LocationEntity>> GetChangedSince(Guid ownerId, DateTime since, int limit);

    Task<DateTime?> GetCurrentServerTime();
}
