namespace Turboapi.Collections.domain.exception;

public class CollectionNotFoundException : Exception
{
    public CollectionNotFoundException(string? message) : base(message) { }
}
