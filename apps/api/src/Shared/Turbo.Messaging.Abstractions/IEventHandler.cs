namespace Turbo.Messaging;

/// <summary>
/// Receives a single event of type <typeparamref name="TEvent"/>. The
/// hosting transport (in-process, NATS JetStream, …) is responsible for
/// resolving the handler in a fresh DI scope per delivery and for ACK/NAK
/// semantics. Handlers must be idempotent because the transport delivers
/// at-least-once.
/// </summary>
public interface IEventHandler<in TEvent> where TEvent : class
{
    Task HandleAsync(TEvent @event, CancellationToken cancellationToken);
}
