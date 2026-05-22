using Turbo.Messaging;

namespace Turboapi.Auth.Domain;

/// <summary>
/// Aggregates that emit domain events implement this so the UnitOfWork can
/// drain them at SaveChanges time and route them through the transactional
/// outbox in the same database transaction.
/// </summary>
public interface IHasDomainEvents
{
    IReadOnlyCollection<IDomainEvent> DomainEvents { get; }
    void ClearDomainEvents();
}
