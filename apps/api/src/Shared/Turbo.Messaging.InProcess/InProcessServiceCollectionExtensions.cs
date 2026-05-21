using System.Text.Json;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;

namespace Turbo.Messaging.InProcess;

public static class InProcessServiceCollectionExtensions
{
    /// <summary>
    /// Wires the in-process publish + subscribe path for the modulith
    /// deploy. The outbox dispatcher publishes to <see cref="IMessageTransport"/>
    /// — here that's <see cref="InProcessMessageTransport"/> — and the
    /// <see cref="InProcessSubscriberHost"/> drains the bus and routes
    /// to handlers. No broker is started.
    /// </summary>
    public static IServiceCollection AddInProcessMessaging(this IServiceCollection services)
    {
        services.TryAddSingleton<InProcessMessageBus>();
        services.TryAddSingleton<IMessageTransport, InProcessMessageTransport>();
        services.AddHostedService<InProcessSubscriberHost>();
        return services;
    }

    /// <summary>
    /// Registers an in-process subscription for one event type. The
    /// subject must match the <c>EventEnvelope.Type</c> the outbox
    /// dispatcher produced (e.g. <c>turbo.activity.ActivityCreated</c>).
    /// The handler is resolved via <see cref="IEventHandler{TEvent}"/>
    /// in a fresh DI scope per delivery.
    /// </summary>
    public static IServiceCollection AddInProcessSubscriber<TEvent>(
        this IServiceCollection services,
        string subject)
        where TEvent : class, IDomainEvent
    {
        services.AddSingleton(new InProcessSubscriberRegistration
        {
            Subject = subject,
            EventType = typeof(TEvent),
            Dispatcher = async (sp, data, ct) =>
            {
                var evt = JsonSerializer.Deserialize<TEvent>(data.Span)
                          ?? throw new InvalidOperationException(
                              $"In-process payload for subject {subject} deserialized to null for {typeof(TEvent).Name}");
                var handler = sp.GetRequiredService<IEventHandler<TEvent>>();
                await handler.HandleAsync(evt, ct);
            }
        });
        return services;
    }
}
