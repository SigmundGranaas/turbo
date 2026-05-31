namespace Turbo.Messaging.Nats;

/// <summary>
/// One registration per (subject, durable consumer, .NET event type)
/// triple. The hosted <see cref="NatsSubscriberHost"/> walks the
/// registrations at startup and binds a JetStream consumer for each.
///
/// <see cref="StreamName"/> is optional: when null, the host uses its
/// default <see cref="NatsMessagingOptions.StreamName"/>. Cross-service
/// subscribers (e.g. the Sharing service consuming from Collections,
/// Geo, and Tracks streams) populate this per registration.
/// </summary>
public sealed class NatsSubscriberRegistration
{
    public required string Subject { get; init; }
    public required string DurableName { get; init; }
    public required Type EventType { get; init; }
    public required Func<IServiceProvider, ReadOnlyMemory<byte>, CancellationToken, Task> Dispatcher { get; init; }

    /// <summary>
    /// Optional stream override. When null, the subscriber binds to the
    /// host's default stream.
    /// </summary>
    public string? StreamName { get; init; }
}
