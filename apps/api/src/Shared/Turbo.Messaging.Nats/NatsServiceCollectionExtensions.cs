using System.Text.Json;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;

namespace Turbo.Messaging.Nats;

public static class NatsServiceCollectionExtensions
{
    /// <summary>
    /// Registers <see cref="NatsMessageTransport"/> as the publish-side
    /// <see cref="IMessageTransport"/> and the host that consumes any
    /// subscriptions added via <see cref="AddNatsSubscriber{TEvent}"/>.
    /// </summary>
    public static IServiceCollection AddNatsMessaging(
        this IServiceCollection services,
        Action<NatsMessagingOptions> configure)
    {
        services.Configure(configure);
        services.TryAddSingleton<IMessageTransport, NatsMessageTransport>();
        services.AddHostedService<NatsSubscriberHost>();
        return services;
    }

    /// <summary>
    /// Registers a JetStream subscription that resolves
    /// <see cref="IEventHandler{TEvent}"/> from a fresh DI scope and
    /// invokes <c>HandleAsync</c> for each delivered envelope.
    /// </summary>
    public static IServiceCollection AddNatsSubscriber<TEvent>(
        this IServiceCollection services,
        string subject,
        string durableName)
        where TEvent : class, IDomainEvent
        => services.AddNatsSubscriberCore<TEvent>(subject, durableName, streamName: null);

    /// <summary>
    /// Cross-service variant: binds the consumer to a stream owned by
    /// another service (e.g. Sharing's sidecar subscribing to events on
    /// the TURBO_COLLECTIONS stream). The stream must already exist —
    /// the owning service creates it on boot.
    /// </summary>
    public static IServiceCollection AddNatsSubscriberOnStream<TEvent>(
        this IServiceCollection services,
        string streamName,
        string subject,
        string durableName)
        where TEvent : class, IDomainEvent
        => services.AddNatsSubscriberCore<TEvent>(subject, durableName, streamName);

    private static IServiceCollection AddNatsSubscriberCore<TEvent>(
        this IServiceCollection services,
        string subject,
        string durableName,
        string? streamName)
        where TEvent : class, IDomainEvent
    {
        services.AddSingleton(new NatsSubscriberRegistration
        {
            Subject = subject,
            DurableName = durableName,
            EventType = typeof(TEvent),
            StreamName = streamName,
            Dispatcher = async (sp, data, ct) =>
            {
                var evt = JsonSerializer.Deserialize<TEvent>(data.Span)
                          ?? throw new InvalidOperationException(
                              $"NATS payload for subject {subject} deserialized to null for {typeof(TEvent).Name}");
                var handler = sp.GetRequiredService<IEventHandler<TEvent>>();
                await handler.HandleAsync(evt, ct);
            }
        });
        return services;
    }
}
