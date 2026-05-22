namespace Turbo.Messaging.InProcess;

/// <summary>
/// One registration per (envelope.Type subject, .NET event type)
/// dispatched in the same process. The host walks the registrations
/// at startup so it knows how to deserialize each envelope and which
/// <see cref="IEventHandler{TEvent}"/> to resolve in a fresh DI scope.
/// </summary>
public sealed class InProcessSubscriberRegistration
{
    public required string Subject { get; init; }
    public required Type EventType { get; init; }
    public required Func<IServiceProvider, ReadOnlyMemory<byte>, CancellationToken, Task> Dispatcher { get; init; }
}
