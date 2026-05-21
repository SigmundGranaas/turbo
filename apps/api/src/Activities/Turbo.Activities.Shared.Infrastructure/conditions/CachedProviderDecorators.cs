using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Cache decorator for avalanche providers. Snaps the request to a
/// day bucket (Varsom updates once per day) and keys by region.
/// </summary>
public sealed class CachedAvalancheProvider : IAvalancheProvider
{
    private static readonly TimeSpan CacheTtl = TimeSpan.FromHours(3);

    private readonly IAvalancheProvider _inner;
    private readonly IConditionsCache _cache;
    private readonly ILogger<CachedAvalancheProvider> _logger;
    private readonly TimeProvider _clock;

    public CachedAvalancheProvider(
        IAvalancheProvider inner, IConditionsCache cache,
        ILogger<CachedAvalancheProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _cache = cache; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<AvalancheSlice> GetAsync(
        int varsomRegionId, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var bucket = ConditionsCacheKey.DayBucket(at);
        var grid = $"region_{varsomRegionId}";
        var hit = await _cache.TryGetAsync(_inner.Key, grid, bucket, cancellationToken);
        if (hit is not null && hit.ExpiresAt > _clock.GetUtcNow())
        {
            try
            {
                var cached = JsonSerializer.Deserialize<AvalancheSlice>(hit.Payload.Span);
                if (cached is not null) return cached;
            }
            catch (JsonException ex)
            {
                _logger.LogWarning(ex, "Discarding corrupt avalanche cache for {Grid} {Bucket}", grid, bucket);
            }
        }
        var fresh = await _inner.GetAsync(varsomRegionId, at, cancellationToken);
        var now = _clock.GetUtcNow();
        await _cache.PutAsync(_inner.Key, grid, bucket,
            JsonSerializer.SerializeToUtf8Bytes(fresh), now, now + CacheTtl, cancellationToken);
        return fresh;
    }
}

/// <summary>
/// Cache decorator for river-flow providers. 1h time bucket per station.
/// </summary>
public sealed class CachedRiverFlowProvider : IRiverFlowProvider
{
    private static readonly TimeSpan CacheTtl = TimeSpan.FromMinutes(30);

    private readonly IRiverFlowProvider _inner;
    private readonly IConditionsCache _cache;
    private readonly ILogger<CachedRiverFlowProvider> _logger;
    private readonly TimeProvider _clock;

    public CachedRiverFlowProvider(
        IRiverFlowProvider inner, IConditionsCache cache,
        ILogger<CachedRiverFlowProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _cache = cache; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<RiverFlowSlice> GetAsync(
        string nveStationCode, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var bucket = ConditionsCacheKey.HourBucket(at);
        var grid = $"station_{nveStationCode}";
        var hit = await _cache.TryGetAsync(_inner.Key, grid, bucket, cancellationToken);
        if (hit is not null && hit.ExpiresAt > _clock.GetUtcNow())
        {
            try
            {
                var cached = JsonSerializer.Deserialize<RiverFlowSlice>(hit.Payload.Span);
                if (cached is not null) return cached;
            }
            catch (JsonException ex)
            {
                _logger.LogWarning(ex, "Discarding corrupt river-flow cache for {Grid} {Bucket}", grid, bucket);
            }
        }
        var fresh = await _inner.GetAsync(nveStationCode, at, cancellationToken);
        var now = _clock.GetUtcNow();
        await _cache.PutAsync(_inner.Key, grid, bucket,
            JsonSerializer.SerializeToUtf8Bytes(fresh), now, now + CacheTtl, cancellationToken);
        return fresh;
    }
}

/// <summary>
/// Cache decorator for tide providers. 1h time bucket, 0.01° grid
/// (same shape as weather).
/// </summary>
public sealed class CachedTideProvider : ITideProvider
{
    private static readonly TimeSpan CacheTtl = TimeSpan.FromMinutes(30);

    private readonly ITideProvider _inner;
    private readonly IConditionsCache _cache;
    private readonly ILogger<CachedTideProvider> _logger;
    private readonly TimeProvider _clock;

    public CachedTideProvider(
        ITideProvider inner, IConditionsCache cache,
        ILogger<CachedTideProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _cache = cache; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<TideSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var bucket = ConditionsCacheKey.HourBucket(at);
        var grid = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        var hit = await _cache.TryGetAsync(_inner.Key, grid, bucket, cancellationToken);
        if (hit is not null && hit.ExpiresAt > _clock.GetUtcNow())
        {
            try
            {
                var cached = JsonSerializer.Deserialize<TideSlice>(hit.Payload.Span);
                if (cached is not null) return cached;
            }
            catch (JsonException ex)
            {
                _logger.LogWarning(ex, "Discarding corrupt tide cache for {Grid} {Bucket}", grid, bucket);
            }
        }
        var fresh = await _inner.GetAsync(latitude, longitude, at, cancellationToken);
        var now = _clock.GetUtcNow();
        await _cache.PutAsync(_inner.Key, grid, bucket,
            JsonSerializer.SerializeToUtf8Bytes(fresh), now, now + CacheTtl, cancellationToken);
        return fresh;
    }
}

/// <summary>
/// Cache decorator for grooming providers. 1h time bucket; the trail
/// id is the only key (no geography needed since the feed is per-
/// trail).
/// </summary>
public sealed class CachedGroomingProvider : IGroomingProvider
{
    private static readonly TimeSpan CacheTtl = TimeSpan.FromMinutes(20);

    private readonly IGroomingProvider _inner;
    private readonly IConditionsCache _cache;
    private readonly ILogger<CachedGroomingProvider> _logger;
    private readonly TimeProvider _clock;

    public CachedGroomingProvider(
        IGroomingProvider inner, IConditionsCache cache,
        ILogger<CachedGroomingProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _cache = cache; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<GroomingSlice> GetAsync(
        string feedKey, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var bucket = ConditionsCacheKey.HourBucket(at);
        var grid = $"feed_{feedKey}";
        var hit = await _cache.TryGetAsync(_inner.Key, grid, bucket, cancellationToken);
        if (hit is not null && hit.ExpiresAt > _clock.GetUtcNow())
        {
            try
            {
                var cached = JsonSerializer.Deserialize<GroomingSlice>(hit.Payload.Span);
                if (cached is not null) return cached;
            }
            catch (JsonException ex)
            {
                _logger.LogWarning(ex, "Discarding corrupt grooming cache for {Grid} {Bucket}", grid, bucket);
            }
        }
        var fresh = await _inner.GetAsync(feedKey, at, cancellationToken);
        var now = _clock.GetUtcNow();
        await _cache.PutAsync(_inner.Key, grid, bucket,
            JsonSerializer.SerializeToUtf8Bytes(fresh), now, now + CacheTtl, cancellationToken);
        return fresh;
    }
}
