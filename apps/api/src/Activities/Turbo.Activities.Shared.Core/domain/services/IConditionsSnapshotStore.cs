namespace Turboapi.Activities.domain.services;

/// <summary>
/// Persistent time-series of conditions snapshots. Where
/// <see cref="IConditionsCache"/> is a TTL hot cache (entries vanish after
/// the per-provider TTL), the snapshot store keeps history so orchestrators
/// can answer questions no upstream API can — today's flow vs. the
/// 5-year day-of-year percentile at this station, this spot's 24h pressure
/// trend, last 14d of weather above a threshold.
/// </summary>
public interface IConditionsSnapshotStore
{
    /// <summary>Append a snapshot. Writers should call this in the same
    /// codepath as <see cref="IConditionsCache.PutAsync"/> — the
    /// <c>SnapshottingXProvider</c> decorator does this transparently.</summary>
    Task WriteAsync(ConditionsSnapshot snapshot, CancellationToken cancellationToken);

    /// <summary>Most recent snapshot for a key.</summary>
    Task<ConditionsSnapshot?> GetLatestAsync(
        string providerKey,
        string gridCell,
        CancellationToken cancellationToken);

    /// <summary>Recent snapshots ordered by <see cref="ConditionsSnapshot.ObservedAt"/>
    /// descending. Used for trend computation (e.g. 24h pressure slope).</summary>
    Task<IReadOnlyList<ConditionsSnapshot>> GetRecentAsync(
        string providerKey,
        string gridCell,
        DateTimeOffset since,
        DateTimeOffset until,
        int limit,
        CancellationToken cancellationToken);

    /// <summary>Percentile rank of the latest value for a key within a
    /// day-of-year window (e.g. "today's NVE flow vs DOY ±7 over all
    /// stored history") and a numeric metric path. Implementations
    /// resolve the metric via an <see cref="IMetricExtractor"/> registered
    /// per provider so this store stays agnostic of payload shapes.
    /// Returns <c>null</c> when too few historical samples exist to be
    /// meaningful (the orchestrator should treat this as zero
    /// confidence).</summary>
    Task<double?> GetPercentileAsync(
        string providerKey,
        string gridCell,
        int doyWindowDays,
        string metricKey,
        double currentValue,
        CancellationToken cancellationToken);
}

/// <summary>
/// One row in the snapshot store. <see cref="Payload"/> is the same
/// serialized slice shape the cache holds; the store doesn't interpret
/// it directly — extraction happens through registered
/// <see cref="IMetricExtractor"/>s.
/// </summary>
public sealed record ConditionsSnapshot(
    string ProviderKey,
    string GridCell,
    DateTimeOffset ObservedAt,
    DateTimeOffset FetchedAt,
    ReadOnlyMemory<byte> Payload,
    short PayloadSchemaVersion);

/// <summary>
/// Per-provider strategy for pulling a scalar value out of a snapshot
/// payload. Implementations live next to the provider they extract from
/// (so the WeatherProvider's extractor lives in Shared.Infrastructure
/// alongside <c>MetNoWeatherProvider</c>). The snapshot store reads
/// extractors through <see cref="IMetricExtractorRegistry"/>.
/// </summary>
public interface IMetricExtractor
{
    string ProviderKey { get; }

    /// <summary>Pull a scalar metric out of a payload by key
    /// (<c>"airPressureHpa"</c>, <c>"currentCumecs"</c>, …). Return
    /// <c>null</c> if the key is unknown or the payload is malformed.</summary>
    double? Extract(ReadOnlyMemory<byte> payload, string metricKey);
}

public interface IMetricExtractorRegistry
{
    IMetricExtractor? GetFor(string providerKey);
}
