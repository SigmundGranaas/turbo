using Turbo.Outbox;
using Turboapi.Tracks.domain.commands;
using Turboapi.Tracks.domain.exception;
using Turboapi.Tracks.domain.model;
using Turboapi.Tracks.domain.query;
using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.domain.handler;

public class DeleteTrackHandler
{
    private readonly ITrackReadRepository _read;
    private readonly IOutbox<TracksScope> _outbox;
    private readonly IUnitOfWork<TracksScope> _uow;

    public DeleteTrackHandler(
        ITrackReadRepository read,
        IOutbox<TracksScope> outbox,
        IUnitOfWork<TracksScope> uow)
    {
        _read = read;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task Handle(DeleteTrackCommand command)
    {
        var entity = await _read.GetEntityById(command.TrackId);
        if (entity is null || entity.DeletedAt is not null)
            throw new TrackNotFoundException($"Track with ID {command.TrackId} not found");

        if (command.IfMatchVersion is { } expected && entity.Version != expected)
            throw new OptimisticConcurrencyException(expected, entity.Version);

        var aggregate = Track.Reconstitute(
            entity.Id, entity.OwnerId,
            new TrackMetadata(entity.Name, entity.Description, entity.ColorHex, entity.IconKey, entity.LineStyleKey, entity.Smoothing),
            TrackGeometry.FromLineString(entity.Geometry,
                entity.Elevations is null ? null : (IReadOnlyList<double>)entity.Elevations),
            new TrackStats(entity.DistanceMeters, entity.AscentMeters, entity.DescentMeters, entity.MovingTimeSeconds, entity.RecordedAt));

        aggregate.Delete(command.UserId);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(aggregate.Id, aggregate.Events, ct));
    }
}
