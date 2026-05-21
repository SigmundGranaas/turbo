using System.Diagnostics;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.BackcountrySki.data.model;
using Turboapi.Activities.BackcountrySki.events;
using Turboapi.Activities.BackcountrySki.value;

namespace Turboapi.Activities.BackcountrySki.data;

public sealed class BackcountrySkiActivityCreatedHandler : IEventHandler<BackcountrySkiActivityCreated>
{
    private readonly BackcountrySkiContext _db;
    private readonly IIdempotencyStore<BackcountrySkiContext> _idempotency;
    private readonly ILogger<BackcountrySkiActivityCreatedHandler> _logger;
    private readonly ActivitySource _trace;

    public BackcountrySkiActivityCreatedHandler(
        BackcountrySkiContext db,
        IIdempotencyStore<BackcountrySkiContext> idempotency,
        ILogger<BackcountrySkiActivityCreatedHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
        _trace = new ActivitySource("BackcountrySkiActivityCreatedHandler");
    }

    public async Task HandleAsync(BackcountrySkiActivityCreated @event, CancellationToken cancellationToken)
    {
        using var span = _trace.StartActivity("Handle BackcountrySkiActivityCreated");
        span?.SetTag("activity.id", @event.ActivityId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed BackcountrySkiActivityCreated {EventId}", @event.EventId);
            return;
        }

        var reader = new WKTReader();
        var routeGeom = (LineString)reader.Read(@event.RouteWkt);
        if (routeGeom.SRID != 4326) routeGeom.SRID = 4326;

        var d = @event.Details;
        var entity = new BackcountrySkiActivityEntity
        {
            Id = @event.ActivityId,
            OwnerId = @event.OwnerId,
            Name = @event.Name,
            Description = @event.Description,
            Route = routeGeom,
            AscentMeters = d.AscentMeters,
            DescentMeters = d.DescentMeters,
            DistanceMeters = d.DistanceMeters,
            ElevationMinMeters = d.ElevationMinMeters,
            ElevationMaxMeters = d.ElevationMaxMeters,
            AtesRating = (short)d.AtesRating,
            DominantAspect = d.DominantAspect is { } a ? (short)a : null,
            VarsomRegionId = d.VarsomRegionId,
            PreferredAvalancheMaxLevel = d.PreferredAvalancheMaxLevel,
            CreatedAt = @event.OccurredAt,
            UpdatedAt = @event.OccurredAt,
            DeletedAt = null,
            Version = 1,
            AspectMix = d.AspectMix
                .Select(a => new AspectShareEntity
                {
                    ActivityId = @event.ActivityId,
                    Aspect = (short)a.Aspect,
                    Fraction = a.Fraction,
                })
                .ToList(),
            Legs = d.Legs
                .Select((l, i) =>
                {
                    var lr = new WKTReader();
                    var lg = (LineString)lr.Read(l.PolylineWkt);
                    if (lg.SRID != 4326) lg.SRID = 4326;
                    return new RouteLegEntity
                    {
                        ActivityId = @event.ActivityId,
                        Ordinal = i,
                        LegKind = (short)l.Kind,
                        StartElevationMeters = l.StartElevationMeters,
                        EndElevationMeters = l.EndElevationMeters,
                        Geometry = lg,
                    };
                })
                .ToList(),
        };

        _db.Activities.Add(entity);
        await _db.SaveChangesAsync(cancellationToken);
        _logger.LogInformation("Projected backcountry ski activity {Id} for owner {OwnerId}", entity.Id, entity.OwnerId);
    }
}

public sealed class BackcountrySkiActivityUpdatedHandler : IEventHandler<BackcountrySkiActivityUpdated>
{
    private readonly BackcountrySkiContext _db;
    private readonly IIdempotencyStore<BackcountrySkiContext> _idempotency;
    private readonly ILogger<BackcountrySkiActivityUpdatedHandler> _logger;

    public BackcountrySkiActivityUpdatedHandler(
        BackcountrySkiContext db,
        IIdempotencyStore<BackcountrySkiContext> idempotency,
        ILogger<BackcountrySkiActivityUpdatedHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(BackcountrySkiActivityUpdated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed BackcountrySkiActivityUpdated {EventId}", @event.EventId);
            return;
        }

        var entity = await _db.Activities
            .Include(a => a.AspectMix)
            .Include(a => a.Legs)
            .FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null)
        {
            _logger.LogWarning("Backcountry ski activity {Id} not found for update", @event.ActivityId);
            return;
        }
        if (@event.Version <= entity.Version)
        {
            _logger.LogDebug("Stale BackcountrySkiActivityUpdated for {Id}", @event.ActivityId);
            return;
        }

        if (@event.Name is not null) entity.Name = @event.Name;
        if (@event.Description is not null) entity.Description = @event.Description;
        if (@event.RouteWkt is not null)
        {
            var reader = new WKTReader();
            var routeGeom = (LineString)reader.Read(@event.RouteWkt);
            if (routeGeom.SRID != 4326) routeGeom.SRID = 4326;
            entity.Route = routeGeom;
        }
        if (@event.Details is { } d)
        {
            entity.AscentMeters = d.AscentMeters;
            entity.DescentMeters = d.DescentMeters;
            entity.DistanceMeters = d.DistanceMeters;
            entity.ElevationMinMeters = d.ElevationMinMeters;
            entity.ElevationMaxMeters = d.ElevationMaxMeters;
            entity.AtesRating = (short)d.AtesRating;
            entity.DominantAspect = d.DominantAspect is { } a ? (short)a : null;
            entity.VarsomRegionId = d.VarsomRegionId;
            entity.PreferredAvalancheMaxLevel = d.PreferredAvalancheMaxLevel;

            _db.RemoveRange(entity.AspectMix);
            _db.RemoveRange(entity.Legs);
            entity.AspectMix = d.AspectMix
                .Select(a => new AspectShareEntity
                {
                    ActivityId = entity.Id,
                    Aspect = (short)a.Aspect,
                    Fraction = a.Fraction,
                })
                .ToList();
            entity.Legs = d.Legs
                .Select((l, i) =>
                {
                    var lr = new WKTReader();
                    var lg = (LineString)lr.Read(l.PolylineWkt);
                    if (lg.SRID != 4326) lg.SRID = 4326;
                    return new RouteLegEntity
                    {
                        ActivityId = entity.Id,
                        Ordinal = i,
                        LegKind = (short)l.Kind,
                        StartElevationMeters = l.StartElevationMeters,
                        EndElevationMeters = l.EndElevationMeters,
                        Geometry = lg,
                    };
                })
                .ToList();
        }

        entity.UpdatedAt = @event.OccurredAt;
        entity.Version = @event.Version;
        await _db.SaveChangesAsync(cancellationToken);
    }
}

public sealed class BackcountrySkiActivityDeletedHandler : IEventHandler<BackcountrySkiActivityDeleted>
{
    private readonly BackcountrySkiContext _db;
    private readonly IIdempotencyStore<BackcountrySkiContext> _idempotency;
    private readonly ILogger<BackcountrySkiActivityDeletedHandler> _logger;

    public BackcountrySkiActivityDeletedHandler(
        BackcountrySkiContext db,
        IIdempotencyStore<BackcountrySkiContext> idempotency,
        ILogger<BackcountrySkiActivityDeletedHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(BackcountrySkiActivityDeleted @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            return;
        }

        var entity = await _db.Activities.FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null) return;
        entity.DeletedAt = @event.OccurredAt;
        entity.UpdatedAt = @event.OccurredAt;
        entity.Version += 1;
        await _db.SaveChangesAsync(cancellationToken);
    }
}
