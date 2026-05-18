using Turbo.Outbox;
using Turboapi.Geo.domain.commands;
using Turboapi.Geo.domain.exception;
using Turboapi.Geo.domain.query;

namespace Turboapi.Geo.domain.handler;

public class DeleteLocationHandler
{

    private readonly IOutbox<GeoScope> _outbox;
    private readonly IUnitOfWork<GeoScope> _uow;
    private readonly ILocationReadRepository _locationReadRepository;

    public DeleteLocationHandler(
        IOutbox<GeoScope> outbox,
        IUnitOfWork<GeoScope> uow,
        ILocationReadRepository locationReadRepository)
    {
        _outbox = outbox;
        _uow = uow;
        _locationReadRepository = locationReadRepository;
    }

    public async Task Handle(DeleteLocationCommand command)
    {
        var location = await _locationReadRepository.GetById(command.LocationId);
        if (location == null)
            throw new LocationNotFoundException($"Location with ID {command.LocationId} not found");

        location.Delete(command.UserId);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(location.Id, location.Events, ct));
    }
}
