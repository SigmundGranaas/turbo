namespace Turbo.Messaging;

/// <summary>
/// CloudEvents-lite envelope used as the on-wire and outbox representation
/// of a domain event. Concrete <see cref="IDomainEvent"/> instances are
/// serialized into <see cref="Data"/>; everything outside <see cref="Data"/>
/// is transport metadata the broker or in-process bus needs to route, trace,
/// and acknowledge the message.
/// </summary>
public sealed record EventEnvelope(
    Guid EventId,
    string Type,
    string Source,
    DateTime Time,
    string DataContentType,
    ReadOnlyMemory<byte> Data,
    IReadOnlyDictionary<string, string> Headers);
