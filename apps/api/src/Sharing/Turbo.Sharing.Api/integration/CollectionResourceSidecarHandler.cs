using Turbo.Messaging;
using Turboapi.Collections.domain.events;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.integration;

public sealed class CollectionResourceSidecarHandler
    : IEventHandler<CollectionCreated>,
      IEventHandler<CollectionDeleted>
{
    private readonly ResourceSidecar _sidecar;

    public CollectionResourceSidecarHandler(ResourceSidecar sidecar) => _sidecar = sidecar;

    public Task HandleAsync(CollectionCreated @event, CancellationToken cancellationToken)
        => _sidecar.EnsureCreatedAsync(
            @event.EventId, @event.CollectionId, ResourceType.Collection,
            @event.OwnerId, @event.OccurredAt, cancellationToken);

    public Task HandleAsync(CollectionDeleted @event, CancellationToken cancellationToken)
        => _sidecar.SoftDeleteAsync(@event.EventId, @event.CollectionId, @event.OccurredAt, cancellationToken);
}
