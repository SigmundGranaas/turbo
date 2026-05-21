using Turbo.Messaging;

namespace Turbo.Outbox;

public static class OutboxExtensions
{
    /// <summary>
    /// Turns a sequence of <see cref="IDomainEvent"/> instances into
    /// <see cref="EventEnvelope"/>s tagged with the module's source name
    /// and the given aggregate id, then appends them to
    /// <paramref name="outbox"/>. The source name comes from
    /// <typeparamref name="TScope"/>'s static <c>SourceName</c> property
    /// so handlers can't accidentally publish events with the wrong
    /// module identifier.
    /// </summary>
    public static async Task AppendEventsAsync<TScope, TEvent>(
        this IOutbox<TScope> outbox,
        Guid aggregateId,
        IEnumerable<TEvent> events,
        CancellationToken cancellationToken = default)
        where TScope : IModuleScope
        where TEvent : IDomainEvent
    {
        var headers = new Dictionary<string, string> { ["aggregateId"] = aggregateId.ToString() };
        foreach (var @event in events)
        {
            var envelope = EventEnvelopeFactory.For(@event, TScope.SourceName, headers);
            await outbox.AppendAsync(envelope, cancellationToken);
        }
    }
}
