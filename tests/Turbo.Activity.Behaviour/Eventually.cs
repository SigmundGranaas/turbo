using System.Diagnostics;

namespace Turbo.Activity.Behaviour;

internal static class Eventually
{
    public static async Task<T> Returns<T>(
        Func<Task<T?>> probe,
        TimeSpan? timeout = null,
        string? description = null) where T : class
    {
        timeout ??= TimeSpan.FromSeconds(10);
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
