using Turbo.Messaging;
using Turboapi.Geo.domain.events;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.integration;

public sealed class MarkerResourceSidecarHandler
    : IEventHandler<LocationCreated>,
      IEventHandler<LocationDeleted>
{
    private readonly ResourceSidecar _sidecar;

    public MarkerResourceSidecarHandler(ResourceSidecar sidecar) => _sidecar = sidecar;

    public Task HandleAsync(LocationCreated @event, CancellationToken cancellationToken)
        => _sidecar.EnsureCreatedAsync(
            @event.EventId, @event.LocationId, ResourceType.Marker,
            @event.OwnerId, @event.OccurredAt, cancellationToken);

    public Task HandleAsync(LocationDeleted @event, CancellationToken cancellationToken)
        => _sidecar.SoftDeleteAsync(@event.EventId, @event.LocationId, @event.OccurredAt, cancellationToken);
}
