using System.Text.Json;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Helpers shared by the snapshotting-provider decorators. Each decorator
/// follows the same shape: call the inner cached provider, then — if we
/// got fresh data this tick — append a row to
/// <see cref="IConditionsSnapshotStore"/>. The detection of "fresh vs
/// cache hit" is fuzzy because the inner cache doesn't expose that
/// signal; we treat any successful call as worth recording at most once
/// per (provider, grid, observed_at) tuple, with the store side using
/// the grid + observed_at columns to keep history dense without
/// duplicate rows from cache replays.
///
/// The orchestrator pipeline reads the store via
/// <c>GetPercentileAsync</c> / <c>GetTrendAsync</c>; the snapshotter
/// daemon's job is to keep history populated even when no client is
/// browsing.
/// </summary>
internal static class SnapshotWrite
{
    public static async Task AppendAsync<T>(
        IConditionsSnapshotStore store,
        string providerKey,
        string gridCell,
        T slice,
        DateTimeOffset observedAt,
        DateTimeOffset fetchedAt,
        ILogger logger,
        CancellationToken ct)
        where T : class
    {
        try
        {
            var payload = JsonSerializer.SerializeToUtf8Bytes(slice);
            await store.WriteAsync(new ConditionsSnapshot(
                ProviderKey: providerKey,
                GridCell: gridCell,
                ObservedAt: observedAt,
                FetchedAt: fetchedAt,
                Payload: payload,
                PayloadSchemaVersion: 1), ct);
        }
        catch (Exception ex)
        {
            logger.LogDebug(ex, "Snapshot append failed for {Provider} {Grid} {ObservedAt}",
                providerKey, gridCell, observedAt);
            // Soft failure — the cache hit still returns, and the next
            // tick gets another chance to populate history.
        }
    }
}

/// <summary>Weather snapshotting decorator. Sits outside CachedWeatherProvider
/// so cache hits also append rows (the snapshotter daemon does most of
/// the populating; analysis requests piggy-back).</summary>
public sealed class SnapshottingWeatherProvider : IWeatherProvider
{
    private readonly IWeatherProvider _inner;
    private readonly IConditionsSnapshotStore _store;
    private readonly ILogger<SnapshottingWeatherProvider> _logger;
    private readonly TimeProvider _clock;

    public SnapshottingWeatherProvider(
        IWeatherProvider inner, IConditionsSnapshotStore store,
        ILogger<SnapshottingWeatherProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _store = store; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<WeatherSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var slice = await _inner.GetAsync(latitude, longitude, at, cancellationToken);
        var grid = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        await SnapshotWrite.AppendAsync(
            _store, _inner.Key, grid, slice, slice.ValidAt, _clock.GetUtcNow(), _logger, cancellationToken);
        return slice;
    }

    // The forecast view is a read-only convenience for the conditions endpoint;
    // snapshots are populated by the per-instant GetAsync path / daemon, so just
    // delegate so the inner cache decorator's single-fetch override is reached.
    public Task<IReadOnlyList<WeatherSlice>> GetForecastAsync(
        double latitude, double longitude, CancellationToken cancellationToken)
        => _inner.GetForecastAsync(latitude, longitude, cancellationToken);
}

public sealed class SnapshottingAvalancheProvider : IAvalancheProvider
{
    private readonly IAvalancheProvider _inner;
    private readonly IConditionsSnapshotStore _store;
    private readonly ILogger<SnapshottingAvalancheProvider> _logger;
    private readonly TimeProvider _clock;

    public SnapshottingAvalancheProvider(
        IAvalancheProvider inner, IConditionsSnapshotStore store,
        ILogger<SnapshottingAvalancheProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _store = store; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<AvalancheSlice> GetAsync(
        int varsomRegionId, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var slice = await _inner.GetAsync(varsomRegionId, at, cancellationToken);
        var grid = $"region_{varsomRegionId}";
        await SnapshotWrite.AppendAsync(
            _store, _inner.Key, grid, slice, slice.ValidFor, _clock.GetUtcNow(), _logger, cancellationToken);
        return slice;
    }
}

public sealed class SnapshottingTideProvider : ITideProvider
{
    private readonly ITideProvider _inner;
    private readonly IConditionsSnapshotStore _store;
    private readonly ILogger<SnapshottingTideProvider> _logger;
    private readonly TimeProvider _clock;

    public SnapshottingTideProvider(
        ITideProvider inner, IConditionsSnapshotStore store,
        ILogger<SnapshottingTideProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _store = store; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<TideSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var slice = await _inner.GetAsync(latitude, longitude, at, cancellationToken);
        var grid = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        await SnapshotWrite.AppendAsync(
            _store, _inner.Key, grid, slice, slice.ValidAt, _clock.GetUtcNow(), _logger, cancellationToken);
        return slice;
    }
}

public sealed class SnapshottingRiverFlowProvider : IRiverFlowProvider
{
    private readonly IRiverFlowProvider _inner;
    private readonly IConditionsSnapshotStore _store;
    private readonly ILogger<SnapshottingRiverFlowProvider> _logger;
    private readonly TimeProvider _clock;

    public SnapshottingRiverFlowProvider(
        IRiverFlowProvider inner, IConditionsSnapshotStore store,
        ILogger<SnapshottingRiverFlowProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _store = store; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<RiverFlowSlice> GetAsync(
        string nveStationCode, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var slice = await _inner.GetAsync(nveStationCode, at, cancellationToken);
        var grid = $"station_{nveStationCode}";
        await SnapshotWrite.AppendAsync(
            _store, _inner.Key, grid, slice, slice.ValidAt, _clock.GetUtcNow(), _logger, cancellationToken);
        return slice;
    }
}

public sealed class SnapshottingGroomingProvider : IGroomingProvider
{
    private readonly IGroomingProvider _inner;
    private readonly IConditionsSnapshotStore _store;
    private readonly ILogger<SnapshottingGroomingProvider> _logger;
    private readonly TimeProvider _clock;

    public SnapshottingGroomingProvider(
        IGroomingProvider inner, IConditionsSnapshotStore store,
        ILogger<SnapshottingGroomingProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _store = store; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<GroomingSlice> GetAsync(
        string feedKey, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var slice = await _inner.GetAsync(feedKey, at, cancellationToken);
        var grid = $"feed_{feedKey}";
        await SnapshotWrite.AppendAsync(
            _store, _inner.Key, grid, slice, slice.ValidAt, _clock.GetUtcNow(), _logger, cancellationToken);
        return slice;
    }
}

public sealed class SnapshottingSnowpackProvider : ISnowpackProvider
{
    private readonly ISnowpackProvider _inner;
    private readonly IConditionsSnapshotStore _store;
    private readonly ILogger<SnapshottingSnowpackProvider> _logger;
    private readonly TimeProvider _clock;

    public SnapshottingSnowpackProvider(
        ISnowpackProvider inner, IConditionsSnapshotStore store,
        ILogger<SnapshottingSnowpackProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _store = store; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<SnowpackSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, int lookbackDays,
        CancellationToken cancellationToken)
    {
        var slice = await _inner.GetAsync(latitude, longitude, at, lookbackDays, cancellationToken);
        var grid = $"{Math.Round(latitude, 1):F1}_{Math.Round(longitude, 1):F1}_{lookbackDays}d";
        await SnapshotWrite.AppendAsync(
            _store, _inner.Key, grid, slice, slice.ValidAt, _clock.GetUtcNow(), _logger, cancellationToken);
        return slice;
    }
}

public sealed class SnapshottingTurbidityProvider : ITurbidityProvider
{
    private readonly ITurbidityProvider _inner;
    private readonly IConditionsSnapshotStore _store;
    private readonly ILogger<SnapshottingTurbidityProvider> _logger;
    private readonly TimeProvider _clock;

    public SnapshottingTurbidityProvider(
        ITurbidityProvider inner, IConditionsSnapshotStore store,
        ILogger<SnapshottingTurbidityProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _store = store; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<TurbiditySlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var slice = await _inner.GetAsync(latitude, longitude, at, cancellationToken);
        var grid = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        await SnapshotWrite.AppendAsync(
            _store, _inner.Key, grid, slice, slice.ValidAt, _clock.GetUtcNow(), _logger, cancellationToken);
        return slice;
    }
}

public sealed class SnapshottingGriddedSnowProvider : IGriddedSnowProvider
{
    private readonly IGriddedSnowProvider _inner;
    private readonly IConditionsSnapshotStore _store;
    private readonly ILogger<SnapshottingGriddedSnowProvider> _logger;
    private readonly TimeProvider _clock;

    public SnapshottingGriddedSnowProvider(
        IGriddedSnowProvider inner, IConditionsSnapshotStore store,
        ILogger<SnapshottingGriddedSnowProvider> logger, TimeProvider? clock = null)
    {
        _inner = inner; _store = store; _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    public string Key => _inner.Key;

    public async Task<GriddedSnowSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var slice = await _inner.GetAsync(latitude, longitude, at, cancellationToken);
        var grid = $"{Math.Round(latitude, 2):F2}_{Math.Round(longitude, 2):F2}";
        await SnapshotWrite.AppendAsync(
            _store, _inner.Key, grid, slice, slice.ValidAt, _clock.GetUtcNow(), _logger, cancellationToken);
        return slice;
    }
}
