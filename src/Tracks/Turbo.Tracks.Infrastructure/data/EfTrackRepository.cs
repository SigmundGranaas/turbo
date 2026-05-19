using System.Diagnostics;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using Turboapi.Tracks.data.model;
using Turboapi.Tracks.domain.query;
using Turboapi.Tracks.domain.value;
using Track = Turboapi.Tracks.domain.model.Track;

namespace Turboapi.Tracks.data;

public class EfTrackWriteRepository : ITrackWriteRepository
{
    private readonly TrackReadContext _context;
    private readonly ILogger<EfTrackWriteRepository> _logger;

    public EfTrackWriteRepository(TrackReadContext context, ILogger<EfTrackWriteRepository> logger)
    {
        _context = context;
        _logger = logger;
    }

    public async Task<TrackEntity?> GetById(Guid id)
    {
        var sw = Stopwatch.StartNew();
        var result = await _context.Tracks.FindAsync(id);
        sw.Stop();
        _logger.LogDebug("GetById for {TrackId} completed in {ElapsedMs}ms", id, sw.ElapsedMilliseconds);
        return result;
    }

    public async Task Add(TrackEntity entity)
    {
        _context.Tracks.Add(entity);
        await _context.SaveChangesAsync();
        _logger.LogInformation("Added track {TrackId}", entity.Id);
    }

    public async Task Update(TrackEntity entity)
    {
        _context.Entry(entity).State = EntityState.Modified;
        await _context.SaveChangesAsync();
        _logger.LogInformation("Updated track {TrackId} to version {Version}", entity.Id, entity.Version);
    }

    public async Task SoftDelete(Guid id, DateTime deletedAt)
    {
        await _context.Tracks
            .Where(t => t.Id == id)
            .ExecuteUpdateAsync(setters => setters
                .SetProperty(t => t.DeletedAt, deletedAt)
                .SetProperty(t => t.UpdatedAt, deletedAt)
                .SetProperty(t => t.Version, t => t.Version + 1));
        _logger.LogInformation("Soft-deleted track {TrackId} at {DeletedAt}", id, deletedAt);
    }

    public class EfTrackReadRepository : ITrackReadRepository
    {
        private readonly TrackReadContext _context;

        public EfTrackReadRepository(TrackReadContext context) => _context = context;

        public async Task<Track?> GetById(Guid id)
        {
            var entity = await _context.Tracks.FindAsync(id);
            if (entity is null) return null;
            return Reconstitute(entity);
        }

        public async Task<TrackEntity?> GetEntityById(Guid id)
            => await _context.Tracks.AsNoTracking().FirstOrDefaultAsync(t => t.Id == id);

        public async Task<IEnumerable<Track>> GetUserTracks(Guid ownerId, int? limit = null)
        {
            IQueryable<TrackEntity> q = _context.Tracks
                .AsNoTracking()
                .Where(t => t.OwnerId == ownerId)
                .Where(t => t.DeletedAt == null)
                .OrderByDescending(t => t.UpdatedAt);
            if (limit is { } n) q = q.Take(n);
            var entities = await q.ToListAsync();
            return entities.Select(Reconstitute);
        }

        public async Task<IEnumerable<TrackEntity>> GetChangedSince(Guid ownerId, DateTime since, int limit)
        {
            // Ensure the comparator and the stored values are both UTC. The
            // EF model stores TIMESTAMPTZ, but a caller can pass DateTimeKind.Unspecified.
            var sinceUtc = DateTime.SpecifyKind(since.ToUniversalTime(), DateTimeKind.Utc);
            return await _context.Tracks
                .AsNoTracking()
                .Where(t => t.OwnerId == ownerId)
                .Where(t => t.UpdatedAt > sinceUtc)
                .OrderBy(t => t.UpdatedAt)
                .Take(limit)
                .ToListAsync();
        }

        public async Task<DateTime?> GetCurrentServerTime()
        {
            var conn = _context.Database.GetDbConnection();
            var wasOpen = conn.State == System.Data.ConnectionState.Open;
            if (!wasOpen) await conn.OpenAsync();
            try
            {
                await using var cmd = conn.CreateCommand();
                cmd.CommandText = "SELECT CURRENT_TIMESTAMP AT TIME ZONE 'UTC'";
                var result = await cmd.ExecuteScalarAsync();
                if (result is DateTime dt)
                    return DateTime.SpecifyKind(dt, DateTimeKind.Utc);
                return null;
            }
            finally
            {
                if (!wasOpen) await conn.CloseAsync();
            }
        }

        private static Track Reconstitute(TrackEntity e)
        {
            var metadata = new TrackMetadata(
                e.Name, e.Description, e.ColorHex, e.IconKey, e.LineStyleKey, e.Smoothing);
            var geometry = TrackGeometry.FromLineString(
                e.Geometry,
                e.Elevations is null ? null : (IReadOnlyList<double>)e.Elevations);
            var stats = new TrackStats(
                e.DistanceMeters, e.AscentMeters, e.DescentMeters, e.MovingTimeSeconds, e.RecordedAt);
            return Track.Reconstitute(e.Id, e.OwnerId, metadata, geometry, stats);
        }
    }
}
