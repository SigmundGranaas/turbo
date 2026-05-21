namespace Turboapi.Geo.domain.exception;

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
