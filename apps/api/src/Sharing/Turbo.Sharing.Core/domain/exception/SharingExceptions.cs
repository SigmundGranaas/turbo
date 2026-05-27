namespace Turboapi.Sharing.domain.exception;

public sealed class ResourceNotFoundException : Exception
{
    public ResourceNotFoundException(Guid resourceId)
        : base($"Resource {resourceId} not found.") { }
}

public sealed class AccessDeniedException : Exception
{
    public AccessDeniedException(Guid userId, Guid resourceId)
        : base($"User {userId} is not authorized for resource {resourceId}.") { }
}

public sealed class FriendshipAlreadyExistsException : Exception
{
    public FriendshipAlreadyExistsException(Guid a, Guid b)
        : base($"A friendship between {a} and {b} already exists.") { }
}

public sealed class GrantAlreadyExistsException : Exception
{
    public GrantAlreadyExistsException(Guid resourceId)
        : base($"A grant of this subject for resource {resourceId} already exists.") { }
}
