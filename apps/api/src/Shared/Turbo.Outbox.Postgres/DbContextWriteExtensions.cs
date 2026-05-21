using Microsoft.EntityFrameworkCore;

namespace Turbo.Outbox.Postgres;

/// <summary>
/// SaveChanges helpers that compose with EF Core's execution strategy. The
/// outbox dispatcher already does this for its read loop; command handlers
/// must do the same on the write path. Without it,
/// <c>EnableRetryOnFailure</c> can either reject the call ("execution strategy
/// does not support user-initiated transactions") or — when retry fires —
/// silently double-insert outbox rows on the second attempt.
/// </summary>
public static class DbContextWriteExtensions
{
    /// <summary>
    /// Runs <paramref name="work"/> inside <see cref="IExecutionStrategy"/>
    /// and calls <c>SaveChangesAsync</c> at the end. The work delegate
    /// typically appends outbox rows and any other tracked entities; the
    /// helper guarantees the whole unit runs as a single retriable
    /// transaction.
    /// </summary>
    public static async Task SaveChangesWithRetryAsync(
        this DbContext db,
        Func<CancellationToken, Task> work,
        CancellationToken cancellationToken = default)
    {
        var strategy = db.Database.CreateExecutionStrategy();
        await strategy.ExecuteAsync(async ct =>
        {
            await work(ct);
            await db.SaveChangesAsync(ct);
        }, cancellationToken);
    }
}
