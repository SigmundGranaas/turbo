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
/// Cache decorator for turbidity providers. Day bucket (turbidity
/// changes over hours-to-days, not minutes), 0.01° grid (Sentinel-2
/// pixel scale). Keeps freediving viz reads cheap.
/// </summary>
public sealed class CachedTurbidityProvider : ITurbidityProvider
{
    private static readonly TimeSpan CacheTtl = TimeSpan.FromHours(12);

    private readonly ITurbidityProvider _inner;
    private readonly IConditionsCache _cache;
    private readonly ILogger<CachedTurbidityProvider> _logger;
    private readonly TimeProvider _clock;

    public CachedTurbidityProvider(
        ITurbidityProvider inner, IConditionsCache cache,
        ILogger<CachedTurbidityProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _cache = cache; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<TurbiditySlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var bucket = ConditionsCacheKey.DayBucket(at);
        var grid = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        var hit = await _cache.TryGetAsync(_inner.Key, grid, bucket, cancellationToken);
        if (hit is not null && hit.ExpiresAt > _clock.GetUtcNow())
        {
            try
            {
                var cached = JsonSerializer.Deserialize<TurbiditySlice>(hit.Payload.Span);
                if (cached is not null) return cached;
            }
            catch (JsonException ex)
            {
                _logger.LogWarning(ex, "Discarding corrupt turbidity cache for {Grid} {Bucket}", grid, bucket);
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
/// Cache decorator for snowpack (regObs) providers. Day bucket; cell key
/// snaps lat/lon to ~0.1° (the radius-based regObs query already
/// aggregates within a few km, so a tighter grid would multiply upstream
/// calls without improving relevance).
/// </summary>
public sealed class CachedSnowpackProvider : ISnowpackProvider
{
    private static readonly TimeSpan CacheTtl = TimeSpan.FromHours(2);

    private readonly ISnowpackProvider _inner;
    private readonly IConditionsCache _cache;
    private readonly ILogger<CachedSnowpackProvider> _logger;
    private readonly TimeProvider _clock;

    public CachedSnowpackProvider(
        ISnowpackProvider inner, IConditionsCache cache,
        ILogger<CachedSnowpackProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _cache = cache; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<SnowpackSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, int lookbackDays,
        CancellationToken cancellationToken)
    {
        var bucket = ConditionsCacheKey.DayBucket(at);
        var grid = $"{Math.Round(latitude, 1):F1}_{Math.Round(longitude, 1):F1}_{lookbackDays}d";
        var hit = await _cache.TryGetAsync(_inner.Key, grid, bucket, cancellationToken);
        if (hit is not null && hit.ExpiresAt > _clock.GetUtcNow())
        {
            try
            {
                var cached = JsonSerializer.Deserialize<SnowpackSlice>(hit.Payload.Span);
                if (cached is not null) return cached;
            }
            catch (JsonException ex)
            {
                _logger.LogWarning(ex, "Discarding corrupt snowpack cache for {Grid} {Bucket}", grid, bucket);
            }
        }
        var fresh = await _inner.GetAsync(latitude, longitude, at, lookbackDays, cancellationToken);
        var now = _clock.GetUtcNow();
        await _cache.PutAsync(_inner.Key, grid, bucket,
            JsonSerializer.SerializeToUtf8Bytes(fresh), now, now + CacheTtl, cancellationToken);
        return fresh;
    }
}

/// <summary>
/// Cache decorator for gridded snow (seNorge) providers. Day bucket; cell
/// key snaps to seNorge's ~1km grid (0.01°).
/// </summary>
public sealed class CachedGriddedSnowProvider : IGriddedSnowProvider
{
    private static readonly TimeSpan CacheTtl = TimeSpan.FromHours(6);

    private readonly IGriddedSnowProvider _inner;
    private readonly IConditionsCache _cache;
    private readonly ILogger<CachedGriddedSnowProvider> _logger;
    private readonly TimeProvider _clock;

    public CachedGriddedSnowProvider(
        IGriddedSnowProvider inner, IConditionsCache cache,
        ILogger<CachedGriddedSnowProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _cache = cache; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<GriddedSnowSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var bucket = ConditionsCacheKey.DayBucket(at);
        var grid = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        var hit = await _cache.TryGetAsync(_inner.Key, grid, bucket, cancellationToken);
        if (hit is not null && hit.ExpiresAt > _clock.GetUtcNow())
        {
            try
            {
                var cached = JsonSerializer.Deserialize<GriddedSnowSlice>(hit.Payload.Span);
                if (cached is not null) return cached;
            }
            catch (JsonException ex)
            {
                _logger.LogWarning(ex, "Discarding corrupt gridded snow cache for {Grid} {Bucket}", grid, bucket);
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
