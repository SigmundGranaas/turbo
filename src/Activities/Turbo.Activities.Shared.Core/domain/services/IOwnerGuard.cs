namespace Turboapi.Activities.domain.services;

/// <summary>
/// Composition point for owner-enforcement on a per-handler basis. Kind
/// handlers inject this and call <see cref="RequireOwner"/> as their first
/// action; the implementation throws
/// <see cref="UnauthorizedActivityException"/> if the caller is not the
/// owner. Keeps the rule out of the aggregate (which is unaware of
/// authentication) and out of the controller (which is unaware of domain
/// invariants).
/// </summary>
public interface IOwnerGuard
{
    void RequireOwner(Guid callerId, Guid resourceOwnerId);
}

public sealed class OwnerGuard : IOwnerGuard
{
    public void RequireOwner(Guid callerId, Guid resourceOwnerId)
    {
        if (callerId == Guid.Empty)
            throw new UnauthorizedActivityException("Caller id is not set");
        if (callerId != resourceOwnerId)
            throw new UnauthorizedActivityException("Caller does not own this activity");
    }
}

public sealed class UnauthorizedActivityException : Exception
{
    public UnauthorizedActivityException(string message) : base(message) { }
}

public sealed class ActivityNotFoundException : Exception
{
    public ActivityNotFoundException(Guid id) : base($"Activity {id} was not found") { }
}
