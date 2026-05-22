namespace Turbo.Messaging;

/// <summary>
/// Each module declares a single concrete <see cref="IModuleScope"/>
/// implementation (e.g. <c>ActivityScope</c>) carrying its source name —
/// the string that ends up on every <see cref="EventEnvelope.Source"/>
/// field. <c>IOutbox&lt;TScope&gt;</c> and <c>IUnitOfWork&lt;TScope&gt;</c>
/// take this as their type parameter so handlers never have to pass the
/// source as a magic string.
/// </summary>
public interface IModuleScope
{
    static abstract string SourceName { get; }
}

/// <summary>
/// Root contract for every domain event produced by a module. The two
/// required properties exist so subscribers can de-duplicate at-least-once
/// redeliveries by <see cref="EventId"/> without poking the envelope, and
/// so projections can break ties on out-of-order arrival by
/// <see cref="OccurredAt"/>. Transport metadata (Source, headers, content
/// type) still lives on <see cref="EventEnvelope"/>; only the two fields
/// the application layer actually reads are on the event itself.
/// </summary>
public interface IDomainEvent
{
    /// <summary>Stable identifier assigned when the aggregate emits the event.</summary>
    Guid EventId { get; }

    /// <summary>UTC instant the aggregate raised the event.</summary>
    DateTime OccurredAt { get; }
}

/// <summary>
/// Concrete events inherit this record and get the two required
/// <see cref="IDomainEvent"/> properties with sensible defaults — fresh
/// GUID + UtcNow. Each module's events used to declare these themselves
/// under different names (Activity's <c>Id</c>+<c>Timestamp</c>, Geo's
/// <c>Id</c>+<c>OccurredAt</c>, Auth's nothing); this base unifies them.
/// </summary>
public abstract record DomainEvent : IDomainEvent
{
    public Guid EventId { get; init; } = Guid.NewGuid();
    public DateTime OccurredAt { get; init; } = DateTime.UtcNow;
}
