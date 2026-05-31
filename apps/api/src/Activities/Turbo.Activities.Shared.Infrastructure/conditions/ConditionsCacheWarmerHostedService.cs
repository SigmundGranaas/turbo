using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Turboapi.Activities.data;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Background service that periodically calls each configured provider
/// for the most-recently-updated activities. Two effects per tick:
///   1. The shared cache stays warm (a user opening a recent activity
///      sees a sub-second analysis).
///   2. Every call traverses the snapshotting decorator, so
///      <c>activities.conditions_snapshots</c> accumulates the per-grid
///      time-series the orchestrator's percentile + trend queries need.
///
/// Bounded by configuration (max activities per tick, dedupe by grid
/// cell, single per-provider concurrent call) so a scan can't melt
/// upstream quotas.
/// </summary>
public sealed class ConditionsCacheWarmerHostedService : BackgroundService
{
    private readonly IServiceScopeFactory _scopeFactory;
    private readonly IOptionsMonitor<ConditionsCacheWarmerOptions> _options;
    private readonly ILogger<ConditionsCacheWarmerHostedService> _logger;
    private readonly TimeProvider _clock;

    public ConditionsCacheWarmerHostedService(
        IServiceScopeFactory scopeFactory,
        IOptionsMonitor<ConditionsCacheWarmerOptions> options,
        ILogger<ConditionsCacheWarmerHostedService> logger,
        TimeProvider? clock = null)
    {
        _scopeFactory = scopeFactory;
        _options = options;
        _logger = logger;
        _clock = clock ?? TimeProvider.System;
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        // Stagger initial run so multiple instances don't all warm at boot.
        try { await Task.Delay(TimeSpan.FromSeconds(30), stoppingToken); }
        catch (OperationCanceledException) { return; }

        while (!stoppingToken.IsCancellationRequested)
        {
            var opts = _options.CurrentValue;
            if (opts.Enabled)
            {
                try
                {
                    var warmed = await WarmOnceAsync(opts, stoppingToken);
                    if (warmed > 0)
                        _logger.LogInformation("Conditions cache warmer touched {Count} grid cells", warmed);
                }
                catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested) { return; }
                catch (Exception ex)
                {
                    _logger.LogError(ex, "Conditions cache warmer tick failed");
                }
            }

            try { await Task.Delay(opts.Interval, stoppingToken); }
            catch (OperationCanceledException) { return; }
        }
    }

    private async Task<int> WarmOnceAsync(ConditionsCacheWarmerOptions opts, CancellationToken ct)
    {
        using var scope = _scopeFactory.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<ActivitySummariesContext>();
        var weather = scope.ServiceProvider.GetService<IWeatherProvider>();
        var griddedSnow = scope.ServiceProvider.GetService<IGriddedSnowProvider>();
        var snowpack = scope.ServiceProvider.GetService<ISnowpackProvider>();
        if (weather is null && griddedSnow is null && snowpack is null) return 0;

        var cutoff = _clock.GetUtcNow().UtcDateTime - opts.LookbackWindow;
        var summaries = await db.Summaries
            .AsNoTracking()
            .Where(s => s.DeletedAt == null && s.UpdatedAt >= cutoff)
            .OrderByDescending(s => s.UpdatedAt)
            .Take(opts.MaxActivitiesPerTick)
            .ToListAsync(ct);

        // Dedupe by grid cell so two activities in the same 1km cell
        // share one upstream call. Ordering by recency means the more
        // recent activity's geometry wins (lat/lon centroid).
        //
        // Each provider rotates against its own dedupe set so the
        // weather + snow + snowpack calls don't collide on the same
        // SemaphoreSlim — we want per-provider concurrency, not
        // global.
        var weatherSeen = new HashSet<string>();
        var snowSeen = new HashSet<string>();
        var snowpackSeen = new HashSet<string>();
        var touched = 0;
        var at = _clock.GetUtcNow();

        foreach (var summary in summaries)
        {
            if (ct.IsCancellationRequested) break;
            var centroid = summary.Geometry.Centroid;

            if (weather is not null)
            {
                var grid = $"{Math.Round(centroid.Y, 2):F2}_{Math.Round(centroid.X, 2):F2}";
                if (weatherSeen.Add(grid))
                {
                    try
                    {
                        await weather.GetAsync(centroid.Y, centroid.X, at, ct);
                        touched++;
                    }
                    catch (Exception ex)
                    {
                        _logger.LogDebug(ex, "Snapshotter weather call failed for grid {Grid}", grid);
                    }
                }
            }

            if (griddedSnow is not null)
            {
                var grid = $"snow_{Math.Round(centroid.Y, 2):F2}_{Math.Round(centroid.X, 2):F2}";
                if (snowSeen.Add(grid))
                {
                    try
                    {
                        await griddedSnow.GetAsync(centroid.Y, centroid.X, at, ct);
                        touched++;
                    }
                    catch (Exception ex)
                    {
                        _logger.LogDebug(ex, "Snapshotter gridded-snow call failed for grid {Grid}", grid);
                    }
                }
            }

            if (snowpack is not null)
            {
                var grid = $"snowpack_{Math.Round(centroid.Y, 1):F1}_{Math.Round(centroid.X, 1):F1}";
                if (snowpackSeen.Add(grid))
                {
                    try
                    {
                        await snowpack.GetAsync(centroid.Y, centroid.X, at, lookbackDays: 7, ct);
                        touched++;
                    }
                    catch (Exception ex)
                    {
                        _logger.LogDebug(ex, "Snapshotter snowpack call failed for grid {Grid}", grid);
                    }
                }
            }
        }
        return touched;
    }
}

public sealed class ConditionsCacheWarmerOptions
{
    /// <summary>Whether the warmer is allowed to run. Defaults to false
    /// so dev / test runs don't fire background HTTP calls.</summary>
    public bool Enabled { get; set; }

    /// <summary>How often the warmer wakes up.</summary>
    public TimeSpan Interval { get; set; } = TimeSpan.FromMinutes(15);

    /// <summary>How far back to look for "recently updated" summaries.</summary>
    public TimeSpan LookbackWindow { get; set; } = TimeSpan.FromHours(24);

    /// <summary>Per-tick cap on the number of summaries to consider.
    /// After deduping by grid cell, this bounds upstream calls per
    /// tick to at most this many.</summary>
    public int MaxActivitiesPerTick { get; set; } = 50;
}
