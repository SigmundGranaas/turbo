using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.domain.queries;

public record GetTrackByIdQuery(Guid TrackId, Guid Owner);

public record GetUserTracksQuery(Guid Owner, int? Limit = null);

public record GetTracksChangedSinceQuery(Guid Owner, DateTime Since, int Limit = 500);

public record TrackData(
    Guid Id,
    Guid OwnerId,
    TrackMetadata Metadata,
    TrackGeometry Geometry,
    TrackStats Stats,
    DateTime CreatedAt,
    DateTime UpdatedAt,
    DateTime? DeletedAt,
    long Version);
