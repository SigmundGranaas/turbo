using Turbo.Messaging;
using Turboapi.Sharing.value;
using Turboapi.Tracks.domain.events;

namespace Turboapi.Sharing.integration;

public sealed class PathResourceSidecarHandler
    : IEventHandler<TrackCreated>,
      IEventHandler<TrackDeleted>
{
    private readonly ResourceSidecar _sidecar;

    public PathResourceSidecarHandler(ResourceSidecar sidecar) => _sidecar = sidecar;

    public Task HandleAsync(TrackCreated @event, CancellationToken cancellationToken)
        => _sidecar.EnsureCreatedAsync(
            @event.EventId, @event.TrackId, ResourceType.Path,
            @event.OwnerId, @event.OccurredAt, cancellationToken);

    public Task HandleAsync(TrackDeleted @event, CancellationToken cancellationToken)
        => _sidecar.SoftDeleteAsync(@event.EventId, @event.TrackId, @event.OccurredAt, cancellationToken);
}
