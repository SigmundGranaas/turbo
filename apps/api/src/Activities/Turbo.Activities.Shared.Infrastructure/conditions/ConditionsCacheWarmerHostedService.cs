using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Turboapi.Activities.data;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Background service that periodically warms the weather cache for the
/// most-recently-updated activity summaries. Bounded by configuration
/// (max activities per tick, dedupe by grid cell) so a one-time scan
/// can't melt the upstream provider's quota. Runs only when there's a
/// configured <see cref="IWeatherProvider"/>; if no advisor in the
/// system uses weather the work is harmless but pointless.
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
        if (weather is null) return 0; // no provider configured — nothing to do

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
        var seen = new HashSet<string>();
        var warmed = 0;
        var at = _clock.GetUtcNow();
        foreach (var summary in summaries)
        {
            if (ct.IsCancellationRequested) break;
            var centroid = summary.Geometry.Centroid;
            var grid = $"{Math.Round(centroid.Y, 2):F2}_{Math.Round(centroid.X, 2):F2}";
            if (!seen.Add(grid)) continue;

            try
            {
                await weather.GetAsync(centroid.Y, centroid.X, at, ct);
                warmed++;
            }
            catch (Exception ex)
            {
                _logger.LogDebug(ex, "Cache warmer failed for grid {Grid}", grid);
                // Soft failure — next tick will retry.
            }
        }
        return warmed;
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
