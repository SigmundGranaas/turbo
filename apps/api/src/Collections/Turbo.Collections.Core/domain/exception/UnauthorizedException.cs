namespace Turboapi.Collections.domain.exception;

public class UnauthorizedException : Exception
{
    public UnauthorizedException(string? message) : base(message) { }
}
