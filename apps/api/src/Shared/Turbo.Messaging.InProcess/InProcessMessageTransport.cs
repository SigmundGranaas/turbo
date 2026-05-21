namespace Turbo.Messaging.InProcess;

/// <summary>
/// Publish-side <see cref="IMessageTransport"/> for the modulith deploy.
/// Forwards the envelope to a singleton <see cref="InProcessMessageBus"/>;
/// no broker, no serialization round-trip, no network. The outbox
/// dispatcher remains the source of durability — this transport just
/// hands envelopes to the in-process subscriber host.
/// </summary>
public sealed class InProcessMessageTransport : IMessageTransport
{
    private readonly InProcessMessageBus _bus;

    public InProcessMessageTransport(InProcessMessageBus bus) => _bus = bus;

    public Task PublishAsync(EventEnvelope envelope, CancellationToken cancellationToken)
        => _bus.PublishAsync(envelope, cancellationToken);
}
