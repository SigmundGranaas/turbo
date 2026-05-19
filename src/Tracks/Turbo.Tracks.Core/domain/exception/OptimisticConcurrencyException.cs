namespace Turboapi.Tracks.domain.exception;

/// <summary>
/// Raised when the caller's <c>If-Match</c> version does not match the
/// current row version. The controller maps this to HTTP 412 with the
/// server's current row in the body so the client can merge.
/// </summary>
public class OptimisticConcurrencyException : Exception
{
    public long ExpectedVersion { get; }
    public long ActualVersion { get; }

    public OptimisticConcurrencyException(long expectedVersion, long actualVersion)
        : base($"Optimistic concurrency check failed: expected version {expectedVersion} but current version is {actualVersion}")
    {
        ExpectedVersion = expectedVersion;
        ActualVersion = actualVersion;
    }
}
