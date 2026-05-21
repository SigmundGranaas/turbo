using Turbo.Outbox;
using Turboapi.Tracks.domain.commands;
using Turboapi.Tracks.domain.model;

namespace Turboapi.Tracks.domain.handler;

public class CreateTrackHandler
{
    private readonly IOutbox<TracksScope> _outbox;
    private readonly IUnitOfWork<TracksScope> _uow;

    public CreateTrackHandler(IOutbox<TracksScope> outbox, IUnitOfWork<TracksScope> uow)
    {
        _outbox = outbox;
        _uow = uow;
    }

    public async Task<Guid> Handle(CreateTrackCommand command)
    {
        var track = Track.Create(command.UserId, command.Metadata, command.Geometry, command.Stats);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(track.Id, track.Events, ct));
        return track.Id;
    }
}
