using System.Text.Json;

namespace Turbo.Messaging;

/// <summary>
/// Builds an <see cref="EventEnvelope"/> from a typed <see cref="IDomainEvent"/>.
/// Concrete events know nothing about envelopes; the factory is the only
/// place that decides type names, content type, and timestamp.
/// </summary>
public static class EventEnvelopeFactory
{
    /// <summary>
    /// Default serializer options. Property names are left as-is (PascalCase)
    /// so the on-wire JSON round-trips through System.Text.Json's
    /// case-sensitive default deserialization without per-event configuration.
    /// </summary>
    private static readonly JsonSerializerOptions DefaultOptions = new();

    public static EventEnvelope For<TEvent>(
        TEvent @event,
        string source,
        IReadOnlyDictionary<string, string>? headers = null,
        JsonSerializerOptions? jsonOptions = null,
        DateTime? occurredAt = null,
        Guid? eventId = null)
        where TEvent : IDomainEvent
    {
        ArgumentNullException.ThrowIfNull(@event);
        ArgumentException.ThrowIfNullOrWhiteSpace(source);

        var payload = JsonSerializer.SerializeToUtf8Bytes(@event, @event.GetType(), jsonOptions ?? DefaultOptions);
        return new EventEnvelope(
            EventId: eventId ?? Guid.NewGuid(),
            Type: $"turbo.{source}.{@event.GetType().Name}",
            Source: source,
            Time: occurredAt ?? DateTime.UtcNow,
            DataContentType: "application/json",
            Data: payload,
            Headers: headers ?? new Dictionary<string, string>());
    }
}
