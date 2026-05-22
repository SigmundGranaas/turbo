namespace Turbo.Messaging.Nats;

/// <summary>
/// One registration per (subject, durable consumer, .NET event type)
/// triple. The hosted <see cref="NatsSubscriberHost"/> walks the
/// registrations at startup and binds a JetStream consumer for each.
/// </summary>
public sealed class NatsSubscriberRegistration
{
    public required string Subject { get; init; }
    public required string DurableName { get; init; }
    public required Type EventType { get; init; }
    public required Func<IServiceProvider, ReadOnlyMemory<byte>, CancellationToken, Task> Dispatcher { get; init; }
}
