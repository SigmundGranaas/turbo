using Microsoft.EntityFrameworkCore;

namespace Turbo.Outbox.Postgres;

/// <summary>
/// EF Core-backed <see cref="IIdempotencyStore{TDbContext}"/>. Inserts a
/// <see cref="ProcessedEventRow"/> for the event id; if the row already
/// exists the insert returns 0 rows affected via PG's
/// <c>ON CONFLICT DO NOTHING</c> semantics, and the caller knows to skip.
///
/// The insert runs through the configured execution strategy so retry
/// behaves correctly: a second attempt sees the row from the first
/// attempt (committed by the strategy's retriable unit) and reports
/// already-processed.
/// </summary>
public sealed class PgIdempotencyStore<TDbContext> : IIdempotencyStore<TDbContext>
    where TDbContext : DbContext
{
    private readonly TDbContext _db;

    public PgIdempotencyStore(TDbContext db) => _db = db;

    public async Task<bool> TryMarkProcessedAsync(Guid eventId, CancellationToken cancellationToken)
    {
        var schema = _db.Model.FindEntityType(typeof(ProcessedEventRow))?.GetSchema();
        var table = schema is null ? "\"processed_events\"" : $"\"{schema}\".\"processed_events\"";

#pragma warning disable EF1002 // schema/table from EF metadata, not user input
        var inserted = await _db.Database.ExecuteSqlRawAsync(
            $"INSERT INTO {table} (event_id) VALUES ({{0}}) ON CONFLICT (event_id) DO NOTHING;",
            new object[] { eventId },
            cancellationToken);
#pragma warning restore EF1002

        return inserted > 0;
    }
}
