namespace Turboapi.Geo.domain.exception;

public class UnauthorizedException: Exception
{
    public UnauthorizedException(string? message) : base(message)
    {
    }
}