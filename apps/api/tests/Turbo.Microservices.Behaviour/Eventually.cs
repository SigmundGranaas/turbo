namespace Turbo.Microservices.Behaviour;

internal static class Eventually
{
    public static async Task<T> Returns<T>(
        Func<Task<T?>> probe,
        TimeSpan? timeout = null,
        string? description = null) where T : class
    {
        var deadline = DateTime.UtcNow + (timeout ?? TimeSpan.FromSeconds(20));
        Exception? lastError = null;
        while (DateTime.UtcNow < deadline)
        {
            try
            {
                var value = await probe();
                if (value is not null) return value;
            }
            catch (Exception ex) { lastError = ex; }
            await Task.Delay(150);
        }
        throw new TimeoutException(
            $"{description ?? "probe"}: did not produce a value within {(timeout ?? TimeSpan.FromSeconds(20)).TotalSeconds:F1}s"
            + (lastError is null ? "" : $" (last error: {lastError.Message})"));
    }
}
