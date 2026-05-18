namespace Turboapi.Auth.Domain.Events
{
    /// <summary>
    /// Standardizes pulling an AccountId off a domain event so transport
    /// adapters can use it as a partition key. Implemented by every
    /// account-scoped event.
    /// </summary>
    public interface IAccountAssociatedEvent
    {
        Guid AccountId { get; }
    }
}
