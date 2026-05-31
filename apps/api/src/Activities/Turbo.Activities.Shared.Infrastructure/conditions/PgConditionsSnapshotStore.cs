using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.data;
using Turboapi.Activities.data.model;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Postgres-backed <see cref="IConditionsSnapshotStore"/>. Append-only on
/// the write path; reads use the
/// <c>idx_conditions_snapshots_provider_grid_observed_at</c> index for both
/// "most recent" and "within window" queries. Percentile resolution
/// extracts metric values through registered <see cref="IMetricExtractor"/>s
/// — the store stays generic so adding a new provider does not require
/// touching this file.
/// </summary>
public sealed class PgConditionsSnapshotStore : IConditionsSnapshotStore
{
    private readonly ActivitySummariesContext _db;
    private readonly IMetricExtractorRegistry _extractors;
    private readonly ILogger<PgConditionsSnapshotStore> _logger;

    public PgConditionsSnapshotStore(
        ActivitySummariesContext db,
        IMetricExtractorRegistry extractors,
        ILogger<PgConditionsSnapshotStore> logger)
    {
        _db = db;
        _extractors = extractors;
        _logger = logger;
    }

    public async Task WriteAsync(ConditionsSnapshot snapshot, CancellationToken cancellationToken)
    {
        _db.ConditionsSnapshots.Add(new ConditionsSnapshotEntity
        {
            ProviderKey = snapshot.ProviderKey,
            GridCell = snapshot.GridCell,
            ObservedAt = snapshot.ObservedAt.UtcDateTime,
            FetchedAt = snapshot.FetchedAt.UtcDateTime,
            Payload = snapshot.Payload.ToArray(),
            PayloadSchemaVersion = snapshot.PayloadSchemaVersion,
        });
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task<ConditionsSnapshot?> GetLatestAsync(
        string providerKey, string gridCell, CancellationToken cancellationToken)
    {
        var row = await _db.ConditionsSnapshots.AsNoTracking()
            .Where(s => s.ProviderKey == providerKey && s.GridCell == gridCell)
            .OrderByDescending(s => s.ObservedAt)
            .FirstOrDefaultAsync(cancellationToken);
        return row is null ? null : ToDomain(row);
    }

    public async Task<IReadOnlyList<ConditionsSnapshot>> GetRecentAsync(
        string providerKey, string gridCell,
        DateTimeOffset since, DateTimeOffset until, int limit,
        CancellationToken cancellationToken)
    {
        var sinceUtc = since.UtcDateTime;
        var untilUtc = until.UtcDateTime;
        var rows = await _db.ConditionsSnapshots.AsNoTracking()
            .Where(s => s.ProviderKey == providerKey
                        && s.GridCell == gridCell
                        && s.ObservedAt >= sinceUtc
                        && s.ObservedAt <= untilUtc)
            .OrderByDescending(s => s.ObservedAt)
            .Take(limit)
            .ToListAsync(cancellationToken);
        return rows.Select(ToDomain).ToArray();
    }

    public async Task<double?> GetPercentileAsync(
        string providerKey, string gridCell,
        int doyWindowDays, string metricKey, double currentValue,
        CancellationToken cancellationToken)
    {
        var extractor = _extractors.GetFor(providerKey);
        if (extractor is null)
        {
            _logger.LogDebug("No metric extractor registered for {Provider}", providerKey);
            return null;
        }

        // Pull a year of history at most — past that the cost outweighs the
        // gain. Filter to the DOY ±window cohort in-memory because Postgres'
        // extract(doy from ...) over an index works, but EF + jsonb makes
        // this a bigger SQL rewrite than the size of the result justifies
        // for the foundation. The snapshot pruner caps total rows anyway.
        var oneYearAgo = DateTime.UtcNow.AddDays(-365);
        var rows = await _db.ConditionsSnapshots.AsNoTracking()
            .Where(s => s.ProviderKey == providerKey
                        && s.GridCell == gridCell
                        && s.ObservedAt >= oneYearAgo)
            .Select(s => new { s.Payload, s.ObservedAt })
            .ToListAsync(cancellationToken);

        if (rows.Count < 20) return null;

        var todayDoy = DateTime.UtcNow.DayOfYear;
        var withinWindow = new List<double>();
        foreach (var r in rows)
        {
            var doy = r.ObservedAt.DayOfYear;
            if (CircularDistanceDays(doy, todayDoy) > doyWindowDays) continue;
            var value = extractor.Extract(r.Payload, metricKey);
            if (value is not null) withinWindow.Add(value.Value);
        }
        if (withinWindow.Count < 10) return null;

        withinWindow.Sort();
        var below = withinWindow.Count(v => v <= currentValue);
        return (double)below / withinWindow.Count;
    }

    private static int CircularDistanceDays(int a, int b)
    {
        var diff = Math.Abs(a - b);
        return Math.Min(diff, 365 - diff);
    }

    private static ConditionsSnapshot ToDomain(ConditionsSnapshotEntity row) =>
        new(
            ProviderKey: row.ProviderKey,
            GridCell: row.GridCell,
            ObservedAt: new DateTimeOffset(DateTime.SpecifyKind(row.ObservedAt, DateTimeKind.Utc)),
            FetchedAt: new DateTimeOffset(DateTime.SpecifyKind(row.FetchedAt, DateTimeKind.Utc)),
            Payload: row.Payload,
            PayloadSchemaVersion: row.PayloadSchemaVersion);
}

/// <summary>
/// In-memory metric-extractor registry. Composed at DI time from every
/// <see cref="IMetricExtractor"/> registered in the container. Keys are
/// case-sensitive provider keys.
/// </summary>
public sealed class MetricExtractorRegistry : IMetricExtractorRegistry
{
    private readonly IReadOnlyDictionary<string, IMetricExtractor> _byKey;

    public MetricExtractorRegistry(IEnumerable<IMetricExtractor> extractors)
    {
        _byKey = extractors.ToDictionary(e => e.ProviderKey, StringComparer.Ordinal);
    }

    public IMetricExtractor? GetFor(string providerKey) =>
        _byKey.TryGetValue(providerKey, out var x) ? x : null;
}
