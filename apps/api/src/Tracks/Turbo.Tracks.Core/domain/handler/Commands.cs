using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.domain.commands;

public record CreateTrackCommand
{
    public Guid UserId { get; init; }
    public TrackMetadata Metadata { get; init; }
    public TrackGeometry Geometry { get; init; }
    public TrackStats Stats { get; init; }

    public CreateTrackCommand(Guid userId, TrackMetadata metadata, TrackGeometry geometry, TrackStats stats)
    {
        UserId = userId;
        Metadata = metadata ?? throw new ArgumentNullException(nameof(metadata));
        Geometry = geometry ?? throw new ArgumentNullException(nameof(geometry));
        Stats = stats ?? throw new ArgumentNullException(nameof(stats));
    }
}

public record UpdateTrackCommand
{
    public Guid UserId { get; init; }
    public Guid TrackId { get; init; }
    public TrackUpdateParameters Updates { get; init; }
    public long? IfMatchVersion { get; init; }

    public UpdateTrackCommand(Guid userId, Guid trackId, TrackUpdateParameters updates, long? ifMatchVersion = null)
    {
        UserId = userId;
        TrackId = trackId;
        Updates = updates ?? throw new ArgumentNullException(nameof(updates));
        IfMatchVersion = ifMatchVersion;
        if (!updates.HasAnyChange)
            throw new ArgumentException(
                "At least one update parameter must be specified within the updates.",
                nameof(updates));
    }
}

public record DeleteTrackCommand
{
    public Guid UserId { get; init; }
    public Guid TrackId { get; init; }
    public long? IfMatchVersion { get; init; }

    public DeleteTrackCommand(Guid userId, Guid trackId, long? ifMatchVersion = null)
    {
        UserId = userId;
        TrackId = trackId;
        IfMatchVersion = ifMatchVersion;
    }
}
