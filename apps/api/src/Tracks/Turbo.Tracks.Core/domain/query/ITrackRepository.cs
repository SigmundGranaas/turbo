using Turboapi.Tracks.data.model;
using Turboapi.Tracks.domain.model;

namespace Turboapi.Tracks.domain.query;

public interface ITrackReadRepository
{
    Task<Track?> GetById(Guid id);

    /// <summary>
    /// Fetches the raw read-model row (including sync fields). The
    /// controller needs the full row to set ETag headers and to drive
    /// the delta endpoint; aggregate reconstitution would discard those.
    /// </summary>
    Task<TrackEntity?> GetEntityById(Guid id);

    Task<IEnumerable<Track>> GetUserTracks(Guid ownerId, int? limit = null);

    /// <summary>
    /// Returns rows whose <c>UpdatedAt</c> is greater than <paramref name="since"/>.
    /// Includes tombstones (rows with <c>DeletedAt</c> set). Used to power
    /// the delta-sync endpoint.
    /// </summary>
    Task<IEnumerable<TrackEntity>> GetChangedSince(Guid ownerId, DateTime since, int limit);

    Task<DateTime?> GetCurrentServerTime();
}

public interface ITrackWriteRepository
{
    Task<TrackEntity?> GetById(Guid id);
    Task Add(TrackEntity entity);
    Task Update(TrackEntity entity);
    Task SoftDelete(Guid id, DateTime deletedAt);
}
