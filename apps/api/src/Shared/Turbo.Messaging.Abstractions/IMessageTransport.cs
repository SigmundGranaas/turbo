namespace Turbo.Messaging;

/// <summary>
/// Publish-side abstraction over whatever delivers <see cref="EventEnvelope"/>
/// instances out of a module — in-process bus, NATS JetStream, RabbitMQ, etc.
/// The outbox dispatcher is the only caller in production; modules never see
/// this type, because the outbox sits between the domain and the transport.
/// </summary>
public interface IMessageTransport
{
    Task PublishAsync(EventEnvelope envelope, CancellationToken cancellationToken);
}
