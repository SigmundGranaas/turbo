using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.data;
using Turboapi.Activities.data.model;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Postgres-backed <see cref="IConditionsCache"/>. UPSERTs via EF Core's
/// FindAsync + assign, which under Npgsql turns into INSERT … ON
/// CONFLICT DO UPDATE on the composite primary key. Reads are
/// no-tracking.
/// </summary>
public sealed class PgConditionsCache : IConditionsCache
{
    private readonly ActivitySummariesContext _db;

    public PgConditionsCache(ActivitySummariesContext db) => _db = db;

    public async Task<CachedConditionsSlice?> TryGetAsync(
        string providerKey, string gridCell, DateTimeOffset timeBucket,
        CancellationToken cancellationToken)
    {
        var bucketUtc = timeBucket.UtcDateTime;
        var row = await _db.ConditionsCache.AsNoTracking()
            .FirstOrDefaultAsync(
                c => c.ProviderKey == providerKey
                     && c.GridCell == gridCell
                     && c.TimeBucket == bucketUtc,
                cancellationToken);
        if (row is null) return null;

        return new CachedConditionsSlice(
            ProviderKey: row.ProviderKey,
            GridCell: row.GridCell,
            TimeBucket: new DateTimeOffset(DateTime.SpecifyKind(row.TimeBucket, DateTimeKind.Utc)),
            Payload: row.Payload,
            FetchedAt: new DateTimeOffset(DateTime.SpecifyKind(row.FetchedAt, DateTimeKind.Utc)),
            ExpiresAt: new DateTimeOffset(DateTime.SpecifyKind(row.ExpiresAt, DateTimeKind.Utc)));
    }

    public async Task PutAsync(
        string providerKey, string gridCell, DateTimeOffset timeBucket,
        ReadOnlyMemory<byte> payload, DateTimeOffset fetchedAt, DateTimeOffset expiresAt,
        CancellationToken cancellationToken)
    {
        var bucketUtc = timeBucket.UtcDateTime;
        var existing = await _db.ConditionsCache.FirstOrDefaultAsync(
            c => c.ProviderKey == providerKey
                 && c.GridCell == gridCell
                 && c.TimeBucket == bucketUtc,
            cancellationToken);

        if (existing is null)
        {
            _db.ConditionsCache.Add(new ConditionsCacheEntity
            {
                ProviderKey = providerKey,
                GridCell = gridCell,
                TimeBucket = bucketUtc,
                Payload = payload.ToArray(),
                FetchedAt = fetchedAt.UtcDateTime,
                ExpiresAt = expiresAt.UtcDateTime,
            });
        }
        else
        {
            existing.Payload = payload.ToArray();
            existing.FetchedAt = fetchedAt.UtcDateTime;
            existing.ExpiresAt = expiresAt.UtcDateTime;
        }
        await _db.SaveChangesAsync(cancellationToken);
    }
}
