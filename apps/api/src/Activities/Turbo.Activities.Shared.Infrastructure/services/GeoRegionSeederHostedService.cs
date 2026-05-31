using System.Text.Json;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using NetTopologySuite.Features;
using NetTopologySuite.IO;
using Turboapi.Activities.data;
using Turboapi.Activities.data.model;

namespace Turboapi.Activities.services;

/// <summary>
/// One-shot seeder that loads region-polygon GeoJSON files into the
/// <c>activities.geo_regions</c> table. Configured by
/// <see cref="GeoRegionSeederOptions"/>:
///
/// <code>
/// "GeoRegionSeeder": {
///   "Sources": [
///     { "Source": "varsom_region",
///       "FilePath": "/var/turbo/seed/varsom-regions.geojson",
///       "RegionIdProperty": "OmradeId",
///       "NameProperty": "OmradeNavn" }
///   ]
/// }
/// </code>
///
/// Each feature in the GeoJSON file becomes one row keyed on
/// (source, region_id). Re-runs upsert — existing rows are updated
/// when the file is reloaded. Soft-fails individual rows so a single
/// malformed feature doesn't abort the whole seed.
/// </summary>
public sealed class GeoRegionSeederHostedService : BackgroundService
{
    private readonly IServiceScopeFactory _scopes;
    private readonly IOptions<GeoRegionSeederOptions> _options;
    private readonly ILogger<GeoRegionSeederHostedService> _logger;

    public GeoRegionSeederHostedService(
        IServiceScopeFactory scopes,
        IOptions<GeoRegionSeederOptions> options,
        ILogger<GeoRegionSeederHostedService> logger)
    {
        _scopes = scopes;
        _options = options;
        _logger = logger;
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        // Stagger so other migrations finish first.
        try { await Task.Delay(TimeSpan.FromSeconds(5), stoppingToken); }
        catch (OperationCanceledException) { return; }

        var sources = _options.Value.Sources ?? Array.Empty<GeoRegionSource>();
        if (sources.Count == 0)
        {
            _logger.LogDebug("No GeoRegionSeeder sources configured — skipping.");
            return;
        }

        foreach (var src in sources)
        {
            if (stoppingToken.IsCancellationRequested) return;
            try
            {
                await SeedOneAsync(src, stoppingToken);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "GeoRegion seed failed for source {Source}", src.Source);
            }
        }
    }

    private async Task SeedOneAsync(GeoRegionSource src, CancellationToken ct)
    {
        if (string.IsNullOrWhiteSpace(src.FilePath))
        {
            _logger.LogWarning("GeoRegion source {Source} has no FilePath — skipping.", src.Source);
            return;
        }
        if (!File.Exists(src.FilePath))
        {
            _logger.LogWarning("GeoRegion source {Source} file not found: {Path}",
                src.Source, src.FilePath);
            return;
        }

        var reader = new GeoJsonReader();
        FeatureCollection features;
        try
        {
            await using var stream = File.OpenRead(src.FilePath);
            using var sr = new StreamReader(stream);
            var json = await sr.ReadToEndAsync(ct);
            features = reader.Read<FeatureCollection>(json);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to parse GeoJSON {Path}", src.FilePath);
            return;
        }

        using var scope = _scopes.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<ActivitySummariesContext>();

        var existingIds = await db.GeoRegions
            .Where(r => r.Source == src.Source)
            .Select(r => r.RegionId)
            .ToListAsync(ct);
        var existing = existingIds.ToHashSet(StringComparer.Ordinal);

        var added = 0;
        var updated = 0;
        foreach (var feature in features)
        {
            if (ct.IsCancellationRequested) break;
            try
            {
                var regionId = feature.Attributes?[src.RegionIdProperty]?.ToString();
                if (string.IsNullOrEmpty(regionId)) continue;
                var name = feature.Attributes?[src.NameProperty]?.ToString() ?? regionId;
                var geom = feature.Geometry;
                if (geom is null) continue;
                if (geom.SRID != 4326) geom.SRID = 4326;

                if (existing.Contains(regionId))
                {
                    var row = await db.GeoRegions.FirstOrDefaultAsync(
                        r => r.Source == src.Source && r.RegionId == regionId, ct);
                    if (row is null) continue;
                    row.Name = name;
                    row.Geometry = geom;
                    updated++;
                }
                else
                {
                    db.GeoRegions.Add(new GeoRegionEntity
                    {
                        Source = src.Source,
                        RegionId = regionId,
                        Name = name,
                        Geometry = geom,
                    });
                    added++;
                }
            }
            catch (Exception ex)
            {
                _logger.LogWarning(ex, "Skipping malformed feature in {Source}", src.Source);
            }
        }
        await db.SaveChangesAsync(ct);
        _logger.LogInformation(
            "GeoRegion seed for {Source}: {Added} added, {Updated} updated.",
            src.Source, added, updated);
    }
}

public sealed class GeoRegionSeederOptions
{
    public IReadOnlyList<GeoRegionSource>? Sources { get; set; }
}

public sealed class GeoRegionSource
{
    /// <summary>Polygon set key (<c>"varsom_region"</c>,
    /// <c>"mareano_cell"</c>, <c>"watershed"</c>).</summary>
    public string Source { get; set; } = string.Empty;

    /// <summary>Filesystem path to the GeoJSON FeatureCollection.</summary>
    public string FilePath { get; set; } = string.Empty;

    /// <summary>Feature property whose value is the upstream region id
    /// (e.g. Varsom's <c>OmradeId</c>).</summary>
    public string RegionIdProperty { get; set; } = "id";

    /// <summary>Feature property whose value is the display name.</summary>
    public string NameProperty { get; set; } = "name";
}
