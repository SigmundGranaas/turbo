using System.Diagnostics;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using Turboapi.Geo.data.model;
using Turboapi.Geo.domain.query;
using Turboapi.Geo.domain.query.model;
using Turboapi.Geo.domain.value;
using Coordinates = Turboapi.Geo.domain.value.Coordinates;
using Location = Turboapi.Geo.domain.model.Location;

namespace Turboapi.Geo.data;

public class EfLocationWriteRepository : ILocationWriteRepository
{
    private readonly LocationReadContext _context;
    private readonly ILogger<EfLocationWriteRepository> _logger;

    public EfLocationWriteRepository(
        LocationReadContext context,
        ILogger<EfLocationWriteRepository> logger)
    {
        _context = context;
        _logger = logger;
    }

    public async Task<LocationEntity?> GetById(Guid id)
    {
        var stopwatch = Stopwatch.StartNew();
        var result = await _context.Locations.FindAsync(id);
        stopwatch.Stop();

        _logger.LogDebug("GetById for {LocationId} completed in {ElapsedMs}ms",
            id, stopwatch.ElapsedMilliseconds);

        return result;
    }

    public async Task Add(LocationEntity entity)
    {
        var stopwatch = Stopwatch.StartNew();
        _context.Locations.Add(entity);
        await _context.SaveChangesAsync();
        stopwatch.Stop();

        _logger.LogInformation("Added location {LocationId} in {ElapsedMs}ms",
            entity.Id, stopwatch.ElapsedMilliseconds);
    }

    public async Task Update(LocationEntity entity)
    {
        var stopwatch = Stopwatch.StartNew();
        _context.Entry(entity).State = EntityState.Modified;
        await _context.SaveChangesAsync();
        stopwatch.Stop();

        _logger.LogInformation("Updated location {LocationId} (full update) in {ElapsedMs}ms",
            entity.Id, stopwatch.ElapsedMilliseconds);
    }

    public async Task UpdatePartial(Guid id, Coordinates? geometry, DisplayUpdate? displayInformation, DateTime updatedAt)
    {
        var stopwatch = Stopwatch.StartNew();

        if (geometry != null)
        {
            var factory = new GeometryFactory();
            await _context.Locations
                .Where(l => l.Id == id)
                .ExecuteUpdateAsync(setters => setters
                    .SetProperty(l => l.Geometry, geometry.ToPoint(factory)));
        }

        if (displayInformation != null)
        {
            if (displayInformation.Name is not null)
            {
                await _context.Locations
                    .Where(l => l.Id == id)
                    .ExecuteUpdateAsync(setters => setters
                        .SetProperty(l => l.Name, displayInformation.Name));
            }
            if (displayInformation.Description is not null)
            {
                await _context.Locations
                    .Where(l => l.Id == id)
                    .ExecuteUpdateAsync(setters => setters
                        .SetProperty(l => l.Description, displayInformation.Description));
            }
            if (displayInformation.Icon is not null)
            {
                await _context.Locations
                    .Where(l => l.Id == id)
                    .ExecuteUpdateAsync(setters => setters
                        .SetProperty(l => l.Icon, displayInformation.Icon));
            }
        }

        // Stamp the sync fields atomically: every committed update bumps
        // UpdatedAt and Version so the delta endpoint and ETag stay
        // consistent with what the projection just wrote.
        await _context.Locations
            .Where(l => l.Id == id)
            .ExecuteUpdateAsync(setters => setters
                .SetProperty(l => l.UpdatedAt, updatedAt)
                .SetProperty(l => l.Version, l => l.Version + 1));

        stopwatch.Stop();

        _logger.LogInformation("Updated position for location {LocationId} in {ElapsedMs}ms",
            id, stopwatch.ElapsedMilliseconds);
    }

    public async Task SoftDelete(Guid id, DateTime deletedAt)
    {
        var stopwatch = Stopwatch.StartNew();
        await _context.Locations
            .Where(l => l.Id == id)
            .ExecuteUpdateAsync(setters => setters
                .SetProperty(l => l.DeletedAt, deletedAt)
                .SetProperty(l => l.UpdatedAt, deletedAt)
                .SetProperty(l => l.Version, l => l.Version + 1));
        stopwatch.Stop();

        _logger.LogInformation("Soft-deleted location {LocationId} at {DeletedAt} in {ElapsedMs}ms",
            id, deletedAt, stopwatch.ElapsedMilliseconds);
    }

    public class EfLocationReadRepository : ILocationReadRepository
    {
        private readonly LocationReadContext _context;

        public EfLocationReadRepository(LocationReadContext context)
        {
            _context = context;
        }

        public async Task<Location?> GetById(Guid id)
        {
            var location = await _context.Locations.FindAsync(id);
            if (location is null || location.DeletedAt is not null)
                return null;

            return Location.Reconstitute(
                location.Id,
                location.OwnerId,
                Coordinates.FromPoint(location.Geometry),
                new DisplayInformation(location.Name, location.Description, location.Icon));
        }

        public async Task<LocationEntity?> GetEntityById(Guid id)
            => await _context.Locations.AsNoTracking().FirstOrDefaultAsync(l => l.Id == id);

        public async Task<IEnumerable<Location>> GetLocationsInExtent(
            Guid ownerId,
            double minLongitude,
            double minLatitude,
            double maxLongitude,
            double maxLatitude
        )
        {
            var geometryFactory = new GeometryFactory(new PrecisionModel(), 4326);
            var extent = geometryFactory.CreatePolygon(new Coordinate[]
            {
                new(minLongitude, minLatitude),
                new(maxLongitude, minLatitude),
                new(maxLongitude, maxLatitude),
                new(minLongitude, maxLatitude),
                new(minLongitude, minLatitude)
            });

            var query = _context.Locations
                .AsNoTracking()
                .Where(l => l.OwnerId == ownerId)
                .Where(l => l.DeletedAt == null)
                .Where(l => extent.Contains(l.Geometry));

            var results = await query.ToListAsync();

            return results.Select(l => Location.Reconstitute(l.Id, l.OwnerId, Coordinates.FromPoint(l.Geometry),
                new DisplayInformation(l.Name, l.Description, l.Icon)));
        }

        public async Task<IEnumerable<LocationEntity>> GetChangedSince(Guid ownerId, DateTime since, int limit)
        {
            var sinceUtc = DateTime.SpecifyKind(since.ToUniversalTime(), DateTimeKind.Utc);
            return await _context.Locations
                .AsNoTracking()
                .Where(l => l.OwnerId == ownerId)
                .Where(l => l.UpdatedAt > sinceUtc)
                .OrderBy(l => l.UpdatedAt)
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
                if (result is DateTime dt) return DateTime.SpecifyKind(dt, DateTimeKind.Utc);
                return null;
            }
            finally
            {
                if (!wasOpen) await conn.CloseAsync();
            }
        }
    }
}
