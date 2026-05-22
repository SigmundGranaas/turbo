namespace Turbo.Modulith.Behaviour;

internal static class Eventually
{
    public static async Task<T> Returns<T>(
        Func<Task<T?>> probe,
        TimeSpan? timeout = null,
        string? description = null) where T : class
    {
        var deadline = DateTime.UtcNow + (timeout ?? TimeSpan.FromSeconds(15));
        Exception? lastError = null;
        while (DateTime.UtcNow < deadline)
        {
            try
            {
                var value = await probe();
                if (value is not null) return value;
            }
            catch (Exception ex)
            {
                lastError = ex;
            }
            await Task.Delay(150);
        }
        var label = description ?? "probe";
        throw new TimeoutException(
            $"{label}: did not produce a value within {(timeout ?? TimeSpan.FromSeconds(15)).TotalSeconds:F1}s"
            + (lastError is null ? "" : $" (last error: {lastError.Message})"));
    }

    /// <summary>
    /// Polls a boolean condition until it returns true or the deadline
    /// elapses. Use when there is no value to return — e.g. asserting
    /// that a typed read endpoint has stopped finding a deleted row.
    /// </summary>
    public static async Task UntilAsync(
        Func<Task<bool>> condition,
        TimeSpan? timeout = null,
        string? description = null)
    {
        var deadline = DateTime.UtcNow + (timeout ?? TimeSpan.FromSeconds(15));
        Exception? lastError = null;
        while (DateTime.UtcNow < deadline)
        {
            try
            {
                if (await condition()) return;
            }
            catch (Exception ex)
            {
                lastError = ex;
            }
            await Task.Delay(150);
        }
        var label = description ?? "condition";
        throw new TimeoutException(
            $"{label}: did not become true within {(timeout ?? TimeSpan.FromSeconds(15)).TotalSeconds:F1}s"
            + (lastError is null ? "" : $" (last error: {lastError.Message})"));
    }
}
