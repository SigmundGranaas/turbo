using System.Diagnostics;

namespace Turbo.Collections.Behaviour;

internal static class Eventually
{
    /// <summary>
    /// Default timeout for collections probes. Higher than the Geo / Tracks
    /// default because Collections has more events per step (CollectionCreated
    /// → CollectionItemAdded → CollectionItemRemoved), and the outbox
    /// dispatcher's idle backoff can grow to 2s between cycles. A single
    /// add-then-remove chain can hit two consecutive backoffs, so 20s gives
    /// the dispatcher comfortable room to catch up.
    /// </summary>
    private static readonly TimeSpan DefaultTimeout = TimeSpan.FromSeconds(20);

    public static async Task<T> Returns<T>(
        Func<Task<T?>> probe,
        TimeSpan? timeout = null,
        string? description = null) where T : class
    {
        timeout ??= DefaultTimeout;
        var sw = Stopwatch.StartNew();
        while (sw.Elapsed < timeout)
        {
            try
            {
                var value = await probe();
                if (value is not null) return value;
            }
            catch
            {
                // probe is allowed to throw while the projection has not caught up
            }
            await Task.Delay(100);
        }
        throw new TimeoutException(
            description is null
                ? $"Probe did not produce a value within {timeout.Value.TotalSeconds:F1}s"
                : $"{description}: probe did not produce a value within {timeout.Value.TotalSeconds:F1}s");
    }
}
