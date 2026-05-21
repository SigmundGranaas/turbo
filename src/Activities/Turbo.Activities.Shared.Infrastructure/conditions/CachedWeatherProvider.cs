using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Decorator that caches an <see cref="IWeatherProvider"/>'s responses
/// via <see cref="IConditionsCache"/>. Lookups snap (lat,lon) to an
/// 0.01° grid and time to the start of the hour, so nearby concurrent
/// requests share a single upstream call. Cache TTL is 30 minutes for
/// current/near-future data; downstream callers should treat it as a
/// soft hint, not a strong consistency guarantee.
/// </summary>
public sealed class CachedWeatherProvider : IWeatherProvider
{
    private static readonly TimeSpan CacheTtl = TimeSpan.FromMinutes(30);

    private readonly IWeatherProvider _inner;
    private readonly IConditionsCache _cache;
    private readonly ILogger<CachedWeatherProvider> _logger;
    private readonly TimeProvider _clock;

    public CachedWeatherProvider(
        IWeatherProvider inner,
        IConditionsCache cache,
        ILogger<CachedWeatherProvider> logger,
        TimeProvider? clock = null)
    {
        _inner = inner;
        _cache = cache;
        _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<WeatherSlice> GetAsync(
        double latitude, double longitude,
        DateTimeOffset at,
        CancellationToken cancellationToken)
    {
        var grid = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        var bucket = ConditionsCacheKey.HourBucket(at);

        var hit = await _cache.TryGetAsync(_inner.Key, grid, bucket, cancellationToken);
        if (hit is not null && hit.ExpiresAt > _clock.GetUtcNow())
        {
            try
            {
                var cached = JsonSerializer.Deserialize<WeatherSlice>(hit.Payload.Span);
                if (cached is not null)
                {
                    _logger.LogDebug("Weather cache hit for {Key} {Grid} {Bucket}", _inner.Key, grid, bucket);
                    return cached;
                }
            }
            catch (JsonException ex)
            {
                _logger.LogWarning(ex, "Discarding corrupt cache entry for {Key} {Grid} {Bucket}", _inner.Key, grid, bucket);
            }
        }

        var fresh = await _inner.GetAsync(latitude, longitude, at, cancellationToken);
        var now = _clock.GetUtcNow();
        var payload = JsonSerializer.SerializeToUtf8Bytes(fresh);
        await _cache.PutAsync(_inner.Key, grid, bucket, payload, now, now + CacheTtl, cancellationToken);
        return fresh;
    }
}
