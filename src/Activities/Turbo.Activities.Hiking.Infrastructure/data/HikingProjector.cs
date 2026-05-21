using System.Diagnostics;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.Hiking.data.model;
using Turboapi.Activities.Hiking.events;

namespace Turboapi.Activities.Hiking.data;

public sealed class HikingActivityCreatedHandler : IEventHandler<HikingActivityCreated>
{
    private readonly HikingContext _db;
    private readonly IIdempotencyStore<HikingContext> _idempotency;
    private readonly ILogger<HikingActivityCreatedHandler> _logger;
    private readonly ActivitySource _trace;

    public HikingActivityCreatedHandler(HikingContext db, IIdempotencyStore<HikingContext> idempotency, ILogger<HikingActivityCreatedHandler> logger)
    {
        _db = db; _idempotency = idempotency; _logger = logger;
        _trace = new ActivitySource("HikingActivityCreatedHandler");
    }

    public async Task HandleAsync(HikingActivityCreated @event, CancellationToken cancellationToken)
    {
        using var span = _trace.StartActivity("Handle HikingActivityCreated");
        span?.SetTag("activity.id", @event.ActivityId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed HikingActivityCreated {EventId}", @event.EventId);
            return;
        }

        var reader = new WKTReader();
        var route = (LineString)reader.Read(@event.RouteWkt);
        if (route.SRID != 4326) route.SRID = 4326;

        var d = @event.Details;
        var entity = new HikingActivityEntity
        {
            Id = @event.ActivityId,
            OwnerId = @event.OwnerId,
            Name = @event.Name,
            Description = @event.Description,
            Route = route,
            DistanceMeters = d.DistanceMeters,
            AscentMeters = d.AscentMeters,
            DescentMeters = d.DescentMeters,
            ElevationMinMeters = d.ElevationMinMeters,
            ElevationMaxMeters = d.ElevationMaxMeters,
            Difficulty = (short)d.Difficulty,
            Surface = (short)d.Surface,
            Marking = (short)d.Marking,
            EstimatedHours = d.EstimatedHours,
            HasWaterSources = d.HasWaterSources,
            HasShelter = d.HasShelter,
            CreatedAt = @event.OccurredAt,
            UpdatedAt = @event.OccurredAt,
            Version = 1,
            WaterSources = d.WaterSources.Select((w, i) => new WaterSourceEntity
            {
                ActivityId = @event.ActivityId, Ordinal = i,
                Lat = w.Lat, Lon = w.Lon, Kind = w.Kind, Notes = w.Notes,
            }).ToList(),
        };

        _db.Activities.Add(entity);
        await _db.SaveChangesAsync(cancellationToken);
        _logger.LogInformation("Projected hiking activity {Id}", entity.Id);
    }
}

public sealed class HikingActivityUpdatedHandler : IEventHandler<HikingActivityUpdated>
{
    private readonly HikingContext _db;
    private readonly IIdempotencyStore<HikingContext> _idempotency;
    private readonly ILogger<HikingActivityUpdatedHandler> _logger;

    public HikingActivityUpdatedHandler(HikingContext db, IIdempotencyStore<HikingContext> idempotency, ILogger<HikingActivityUpdatedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(HikingActivityUpdated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken)) return;

        var entity = await _db.Activities.Include(a => a.WaterSources)
            .FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null)
        {
            _logger.LogWarning("Hiking activity {Id} not found for update", @event.ActivityId);
            return;
        }
        if (@event.Version <= entity.Version) return;

        if (@event.Name is not null) entity.Name = @event.Name;
        if (@event.Description is not null) entity.Description = @event.Description;
        if (@event.RouteWkt is not null)
        {
            var reader = new WKTReader();
            var route = (LineString)reader.Read(@event.RouteWkt);
            if (route.SRID != 4326) route.SRID = 4326;
            entity.Route = route;
        }
        if (@event.Details is { } d)
        {
            entity.DistanceMeters = d.DistanceMeters;
            entity.AscentMeters = d.AscentMeters;
            entity.DescentMeters = d.DescentMeters;
            entity.ElevationMinMeters = d.ElevationMinMeters;
            entity.ElevationMaxMeters = d.ElevationMaxMeters;
            entity.Difficulty = (short)d.Difficulty;
            entity.Surface = (short)d.Surface;
            entity.Marking = (short)d.Marking;
            entity.EstimatedHours = d.EstimatedHours;
            entity.HasWaterSources = d.HasWaterSources;
            entity.HasShelter = d.HasShelter;

            _db.RemoveRange(entity.WaterSources);
            entity.WaterSources = d.WaterSources.Select((w, i) => new WaterSourceEntity
            {
                ActivityId = entity.Id, Ordinal = i,
                Lat = w.Lat, Lon = w.Lon, Kind = w.Kind, Notes = w.Notes,
            }).ToList();
        }
        entity.UpdatedAt = @event.OccurredAt;
        entity.Version = @event.Version;
        await _db.SaveChangesAsync(cancellationToken);
    }
}

public sealed class HikingActivityDeletedHandler : IEventHandler<HikingActivityDeleted>
{
    private readonly HikingContext _db;
    private readonly IIdempotencyStore<HikingContext> _idempotency;
    private readonly ILogger<HikingActivityDeletedHandler> _logger;

    public HikingActivityDeletedHandler(HikingContext db, IIdempotencyStore<HikingContext> idempotency, ILogger<HikingActivityDeletedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(HikingActivityDeleted @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken)) return;
        var entity = await _db.Activities.FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null) return;
        entity.DeletedAt = @event.OccurredAt;
        entity.UpdatedAt = @event.OccurredAt;
        entity.Version += 1;
        await _db.SaveChangesAsync(cancellationToken);
    }
}
