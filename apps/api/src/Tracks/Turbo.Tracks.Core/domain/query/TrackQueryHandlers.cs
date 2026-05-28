using Turboapi.Sharing;
using Turboapi.Tracks.data.model;
using Turboapi.Tracks.domain.queries;
using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.domain.query;

public class GetTrackByIdHandler
{
    private readonly ITrackReadRepository _read;
    private readonly IAccessControl _access;

    public GetTrackByIdHandler(ITrackReadRepository read, IAccessControl access)
    {
        _read = read;
        _access = access;
    }

    public async Task<TrackData?> Handle(GetTrackByIdQuery query)
    {
        var entity = await _read.GetEntityById(query.TrackId);
        if (entity is null) return null;
        if (entity.DeletedAt is not null) return null;
        if (!await _access.CanReadAsync(query.Owner, query.TrackId)) return null;
        return entity.ToData();
    }
}

public class GetUserTracksHandler
{
    private readonly ITrackReadRepository _read;
    public GetUserTracksHandler(ITrackReadRepository read) => _read = read;

    public async Task<IEnumerable<TrackData>> Handle(GetUserTracksQuery query)
    {
        // GetUserTracks already filters out tombstoned rows at the repository
        // layer; ToData here is purely a shape conversion.
        var tracks = await _read.GetUserTracks(query.Owner, query.Limit);
        // Reconstitute Track -> read-model TrackData via a side path:
        // GetUserTracks returns aggregates without sync fields, so we use
        // the entity-based path instead by re-querying via GetChangedSince
        // with since=DateTime.MinValue is overkill; for now, expose only the
        // aggregate shape and let the controller use the delta endpoint for
        // sync fields. The list endpoint is for "what do I have?", not
        // "what's changed since X?".
        return tracks.Select(t => new TrackData(
            t.Id, t.OwnerId, t.Metadata, t.Geometry, t.Stats,
            CreatedAt: default, UpdatedAt: default, DeletedAt: null, Version: 0));
    }
}

public class GetTracksChangedSinceHandler
{
    private readonly ITrackReadRepository _read;
    public GetTracksChangedSinceHandler(ITrackReadRepository read) => _read = read;

    public async Task<DeltaResult> Handle(GetTracksChangedSinceQuery query)
    {
        var rows = (await _read.GetChangedSince(query.Owner, query.Since, query.Limit)).ToList();
        var items = rows
            .Where(r => r.DeletedAt is null)
            .Select(r => r.ToData())
            .ToList();
        var deleted = rows
            .Where(r => r.DeletedAt is not null)
            .Select(r => new TombstoneData(r.Id, r.DeletedAt!.Value, r.Version))
            .ToList();
        var serverTime = await _read.GetCurrentServerTime() ?? DateTime.UtcNow;
        return new DeltaResult(items, deleted, serverTime);
    }
}

public record DeltaResult(
    IReadOnlyList<TrackData> Items,
    IReadOnlyList<TombstoneData> Deleted,
    DateTime ServerTime);

public record TombstoneData(Guid Id, DateTime DeletedAt, long Version);

internal static class TrackEntityMapper
{
    public static TrackData ToData(this TrackEntity e)
    {
        var points = TrackGeometry.FromLineString(e.Geometry,
            e.Elevations is null ? null : (IReadOnlyList<double>)e.Elevations);
        var metadata = new TrackMetadata(
            e.Name, e.Description, e.ColorHex, e.IconKey, e.LineStyleKey, e.Smoothing);
        var stats = new TrackStats(
            e.DistanceMeters, e.AscentMeters, e.DescentMeters, e.MovingTimeSeconds, e.RecordedAt);
        return new TrackData(
            e.Id, e.OwnerId, metadata, points, stats,
            e.CreatedAt, e.UpdatedAt, e.DeletedAt, e.Version);
    }
}
