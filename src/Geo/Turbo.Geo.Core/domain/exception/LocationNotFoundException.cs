namespace Turboapi.Geo.domain.exception;

public class LocationNotFoundException: Exception
{
    public LocationNotFoundException(string? message) : base(message)
    {
    }
}