using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using Npgsql;
using Turboapi.Sharing.data;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.integration;

/// <summary>
/// One-shot backfill that ensures every existing collection / marker /
/// path in the per-module read models has a corresponding Resource
/// envelope in the Sharing schema.
///
/// New entities (created after the Sharing service is deployed) flow
/// through the event-driven sidecars in <see cref="ResourceSidecar"/>.
/// This service exists for the bootstrap case: a deployment where the
/// payload modules already hold rows from before Sharing existed.
///
/// Idempotent and safe to re-run — rows already present are skipped.
/// Reads from the payload modules' read tables via raw SQL (each
/// service has its own database, so this is the only sane way to do
/// it from a single executable without forcing the payload modules to
/// republish their entire history).
/// </summary>
public sealed class SharingBackfillService
{
    private readonly SharingReadContext _db;
    private readonly string? _collectionsConn;
    private readonly string? _geoConn;
    private readonly string? _tracksConn;
    private readonly ILogger<SharingBackfillService> _logger;

    public SharingBackfillService(
        SharingReadContext db,
        string? collectionsConnectionString,
        string? geoConnectionString,
        string? tracksConnectionString,
        ILogger<SharingBackfillService> logger)
    {
        _db = db;
        _collectionsConn = collectionsConnectionString;
        _geoConn = geoConnectionString;
        _tracksConn = tracksConnectionString;
        _logger = logger;
    }

    public async Task RunAsync(CancellationToken cancellationToken = default)
    {
        var summary = new BackfillSummary();
        await BackfillFromAsync(
            _collectionsConn,
            "SELECT id, owner_id, created_at FROM collections_read WHERE deleted_at IS NULL",
            ResourceType.Collection, summary, cancellationToken);
        await BackfillFromAsync(
            _geoConn,
            "SELECT id, owner_id, created_at FROM locations_read WHERE deleted_at IS NULL",
            ResourceType.Marker, summary, cancellationToken);
        await BackfillFromAsync(
            _tracksConn,
            "SELECT id, owner_id, created_at FROM tracks_read WHERE deleted_at IS NULL",
            ResourceType.Path, summary, cancellationToken);

        _logger.LogInformation(
            "Sharing backfill complete: inserted {Inserted}, skipped {Skipped} across {Scanned} payload rows",
            summary.Inserted, summary.Skipped, summary.Scanned);
    }

    private async Task BackfillFromAsync(
        string? connectionString,
        string sourceSql,
        string resourceType,
        BackfillSummary summary,
        CancellationToken cancellationToken)
    {
        if (string.IsNullOrWhiteSpace(connectionString))
        {
            _logger.LogDebug("Skipping {Type} backfill — no connection string configured", resourceType);
            return;
        }

        IReadOnlyList<(Guid Id, Guid OwnerId, DateTime CreatedAt)> rows;
        try
        {
            rows = await ReadSourceRowsAsync(connectionString, sourceSql, cancellationToken);
        }
        catch (PostgresException ex) when (ex.SqlState == "42P01")
        {
            // Source read table missing — payload module not migrated yet.
            _logger.LogInformation("Source table for {Type} not present; skipping backfill", resourceType);
            return;
        }

        if (rows.Count == 0) return;

        var ids = rows.Select(r => r.Id).ToList();
        var existing = await _db.Resources
            .AsNoTracking()
            .Where(r => ids.Contains(r.Id))
            .Select(r => r.Id)
            .ToListAsync(cancellationToken);
        var existingSet = existing.ToHashSet();

        foreach (var row in rows)
        {
            summary.Scanned++;
            if (existingSet.Contains(row.Id))
            {
                summary.Skipped++;
                continue;
            }
            _db.Resources.Add(new ResourceEntity
            {
                Id = row.Id,
                Type = resourceType,
                OwnerId = row.OwnerId,
                Visibility = Visibility.Private.ToWire(),
                Version = 1,
                UpdatedAt = row.CreatedAt == default ? DateTime.UtcNow : row.CreatedAt.ToUniversalTime(),
            });
            summary.Inserted++;
        }

        if (summary.Inserted > 0)
            await _db.SaveChangesAsync(cancellationToken);
    }

    private static async Task<IReadOnlyList<(Guid Id, Guid OwnerId, DateTime CreatedAt)>> ReadSourceRowsAsync(
        string connectionString, string sql, CancellationToken cancellationToken)
    {
        await using var conn = new NpgsqlConnection(connectionString);
        await conn.OpenAsync(cancellationToken);
        await using var cmd = new NpgsqlCommand(sql, conn);
        await using var reader = await cmd.ExecuteReaderAsync(cancellationToken);
        var rows = new List<(Guid, Guid, DateTime)>();
        while (await reader.ReadAsync(cancellationToken))
        {
            rows.Add((reader.GetGuid(0), reader.GetGuid(1), reader.GetDateTime(2)));
        }
        return rows;
    }

    private sealed class BackfillSummary
    {
        public int Scanned;
        public int Inserted;
        public int Skipped;
    }
}
