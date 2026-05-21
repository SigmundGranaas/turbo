namespace Turboapi.Activities.domain.exception;

/// <summary>
/// Raised when an update command's <c>If-Match</c> version does not
/// match the current row version. Each kind's controller catches this
/// and surfaces HTTP 412 with the server's actual version so the client
/// can merge and retry. Composition over inheritance: per-kind handlers
/// throw the same shared exception instead of each defining their own.
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
