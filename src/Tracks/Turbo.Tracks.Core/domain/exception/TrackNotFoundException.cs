namespace Turboapi.Tracks.domain.exception;

public class TrackNotFoundException : Exception
{
    public TrackNotFoundException(string? message) : base(message) { }
}
