namespace Turboapi.Activities.domain.services;

/// <summary>
/// Small wait-and-retry helper for the read-before-write step in update
/// and delete handlers. The activity projection runs asynchronously
/// after the outbox dispatches the create event, so a fast client that
/// POSTs and then immediately PATCHes the new id can hit a window where
/// the read model has no row yet — without this helper the handler
/// would throw <see cref="ActivityNotFoundException"/> for an activity
/// that genuinely exists.
///
/// We give the projector a brief budget to catch up before declaring
/// the activity missing; a real 404 still surfaces once attempts are
/// exhausted. The projector is in-process, so the catch-up window is
/// typically a couple of hundred ms at most.
/// </summary>
public static class ReadModelCatchup
{
    /// <summary>Default schedule used by the no-argument overload.</summary>
    public static readonly TimeSpan[] DefaultDelays =
    {
        TimeSpan.FromMilliseconds(50),
        TimeSpan.FromMilliseconds(150),
        TimeSpan.FromMilliseconds(400),
        TimeSpan.FromMilliseconds(800),
    };

    public static Task<T?> ReadAsync<T>(
        Func<CancellationToken, Task<T?>> read,
        CancellationToken cancellationToken = default)
        where T : class
        => ReadAsync(read, DefaultDelays, cancellationToken);

    public static async Task<T?> ReadAsync<T>(
        Func<CancellationToken, Task<T?>> read,
        IReadOnlyList<TimeSpan> retryDelays,
        CancellationToken cancellationToken = default)
        where T : class
    {
        var value = await read(cancellationToken);
        if (value is not null) return value;
        for (var i = 0; i < retryDelays.Count; i++)
        {
            await Task.Delay(retryDelays[i], cancellationToken);
            value = await read(cancellationToken);
            if (value is not null) return value;
        }
        return null;
    }
}
