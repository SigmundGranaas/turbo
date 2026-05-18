using Turbo.Outbox;
using Turboapi.Geo.domain.commands;
using Turboapi.Geo.domain.exception;
using Turboapi.Geo.domain.query;

namespace Turboapi.Geo.domain.handler;

public class UpdateLocationHandler
{

    private readonly ILocationReadRepository _repository;
    private readonly IOutbox<GeoScope> _outbox;
    private readonly IUnitOfWork<GeoScope> _uow;

    public UpdateLocationHandler(
        ILocationReadRepository repository,
        IOutbox<GeoScope> outbox,
        IUnitOfWork<GeoScope> uow)
    {
        _repository = repository;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task<Turboapi.Geo.domain.model.Location> Handle(UpdateLocationCommand command)
    {
        var location = await _repository.GetById(command.LocationId);
        if (location == null)
            throw new LocationNotFoundException(command.LocationId.ToString());

        location.Update(command.UserId, command.Updates);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(location.Id, location.Events, ct));
        return location;
    }
}
