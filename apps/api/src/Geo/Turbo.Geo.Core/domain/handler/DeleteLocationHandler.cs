using Turbo.Outbox;
using Turboapi.Geo.domain.commands;
using Turboapi.Geo.domain.exception;
using Turboapi.Geo.domain.query;
using Turboapi.Sharing;

namespace Turboapi.Geo.domain.handler;

public class DeleteLocationHandler
{

    private readonly IOutbox<GeoScope> _outbox;
    private readonly IUnitOfWork<GeoScope> _uow;
    private readonly ILocationReadRepository _locationReadRepository;
    private readonly IAccessControl _access;

    public DeleteLocationHandler(
        IOutbox<GeoScope> outbox,
        IUnitOfWork<GeoScope> uow,
        ILocationReadRepository locationReadRepository,
        IAccessControl access)
    {
        _outbox = outbox;
        _uow = uow;
        _locationReadRepository = locationReadRepository;
        _access = access;
    }

    public async Task Handle(DeleteLocationCommand command)
    {
        var entity = await _locationReadRepository.GetEntityById(command.LocationId);
        if (entity is null || entity.DeletedAt is not null)
            throw new LocationNotFoundException($"Location with ID {command.LocationId} not found");

        if (command.IfMatchVersion is { } expected && entity.Version != expected)
            throw new OptimisticConcurrencyException(expected, entity.Version);

        await _access.RequireWriteAsync(command.UserId, command.LocationId);

        var location = await _locationReadRepository.GetById(command.LocationId);
        if (location is null)
            throw new LocationNotFoundException($"Location with ID {command.LocationId} not found");

        location.Delete(command.UserId);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(location.Id, location.Events, ct));
    }
}
