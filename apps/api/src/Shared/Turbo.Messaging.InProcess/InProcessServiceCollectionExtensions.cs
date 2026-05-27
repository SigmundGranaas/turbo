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
    /// The dispatcher resolves every registered
    /// <see cref="IEventHandler{TEvent}"/> in a fresh DI scope per
    /// delivery and invokes each in sequence, so multiple modules can
    /// react to the same event (e.g. a Collections read-model projector
    /// and a Sharing resource-sidecar maintainer).
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
                var handlers = sp.GetServices<IEventHandler<TEvent>>().ToList();
                if (handlers.Count == 0)
                    throw new InvalidOperationException(
                        $"No IEventHandler<{typeof(TEvent).Name}> registered for subject {subject}");
                foreach (var handler in handlers)
                    await handler.HandleAsync(evt, ct);
            }
        });
        return services;
    }
}
