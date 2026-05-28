using Medo;
using Turbo.Messaging;
using Turboapi.Tracks.domain.events;
using Turboapi.Tracks.domain.exception;
using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.domain.model;

/// <summary>
/// Track aggregate root: a polyline (with optional elevations) plus
/// display metadata and client-attested stats. Owned by a single user;
/// only the owner can update or delete it.
/// </summary>
public class Track
{
    public Guid Id { get; private set; }
    public Guid OwnerId { get; private set; }
    public TrackMetadata Metadata { get; private set; } = TrackMetadata.Empty;
    public TrackGeometry Geometry { get; private set; } = new();
    public TrackStats Stats { get; private set; } = new();

    private readonly List<DomainEvent> _events = new();
    public IReadOnlyList<DomainEvent> Events => _events.AsReadOnly();

    private Track() { }

    public static Track Create(
        Guid ownerId,
        TrackMetadata metadata,
        TrackGeometry geometry,
        TrackStats stats)
    {
        if (metadata is null) throw new ArgumentNullException(nameof(metadata));
        if (geometry is null) throw new ArgumentNullException(nameof(geometry));
        if (stats is null) throw new ArgumentNullException(nameof(stats));
        EnsureGeometryIsValid(geometry);

        var track = new Track
        {
            Id = Uuid7.NewUuid7(),
            OwnerId = ownerId,
            Metadata = metadata,
            Geometry = geometry,
            Stats = stats,
        };

        track._events.Add(new TrackCreated(
            track.Id,
            track.OwnerId,
            track.Metadata,
            track.Geometry,
            track.Stats));

        return track;
    }

    public void Update(Guid requestUserId, TrackUpdateParameters updates)
    {
        if (updates is null) throw new ArgumentNullException(nameof(updates));
        if (!updates.HasAnyChange) return;

        TrackGeometry? newGeometryForEvent = null;
        TrackMetadataUpdate? metadataChangesForEvent = null;
        TrackStats? newStatsForEvent = null;
        var changed = false;

        if (updates.Geometry is not null)
        {
            EnsureGeometryIsValid(updates.Geometry);
            Geometry = updates.Geometry;
            newGeometryForEvent = updates.Geometry;
            changed = true;
        }

        if (updates.Metadata is not null && updates.Metadata.HasAnyChange)
        {
            var name = updates.Metadata.Name ?? Metadata.Name;
            var description = updates.Metadata.Description ?? Metadata.Description;
            var colorHex = updates.Metadata.ColorHex ?? Metadata.ColorHex;
            var iconKey = updates.Metadata.IconKey ?? Metadata.IconKey;
            var lineStyleKey = updates.Metadata.LineStyleKey ?? Metadata.LineStyleKey;
            var smoothing = updates.Metadata.Smoothing ?? Metadata.Smoothing;

            var nextMetadata = new TrackMetadata(name, description, colorHex, iconKey, lineStyleKey, smoothing);
            if (!nextMetadata.Equals(Metadata))
            {
                Metadata = nextMetadata;
                metadataChangesForEvent = updates.Metadata;
                changed = true;
            }
        }

        if (updates.Stats is not null)
        {
            if (!updates.Stats.Equals(Stats))
            {
                Stats = updates.Stats;
                newStatsForEvent = updates.Stats;
                changed = true;
            }
        }

        if (changed)
        {
            _events.Add(new TrackUpdated(
                Id,
                OwnerId,
                new TrackUpdateParameters(newGeometryForEvent, metadataChangesForEvent, newStatsForEvent)));
        }
    }

    public void Delete(Guid requestUserId)
    {
        _events.Add(new TrackDeleted(Id, OwnerId));
    }

    // Authorization moved out of the aggregate; write handlers gate on
    // Turboapi.Sharing.IAccessControl so friend-granted access works.
    private void EnsureUserIsAuthorized(Guid requestUserId)
    {
        if (OwnerId != requestUserId)
            throw new UnauthorizedException("Only the owner can modify this track");
    }

    private static void EnsureGeometryIsValid(TrackGeometry geometry)
    {
        if (geometry.Points.Count < 2)
            throw new ArgumentException("A track must contain at least two points", nameof(geometry));
        if (geometry.Elevations is not null && geometry.Elevations.Count != geometry.Points.Count)
            throw new ArgumentException(
                "If elevations are present, the array length must match the number of points",
                nameof(geometry));
    }

    public static Track Reconstitute(
        Guid id,
        Guid ownerId,
        TrackMetadata metadata,
        TrackGeometry geometry,
        TrackStats stats)
    {
        return new Track
        {
            Id = id,
            OwnerId = ownerId,
            Metadata = metadata,
            Geometry = geometry,
            Stats = stats,
        };
    }
}
