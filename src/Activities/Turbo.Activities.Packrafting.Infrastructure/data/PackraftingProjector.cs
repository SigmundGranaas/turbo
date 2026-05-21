using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.Packrafting.data.model;
using Turboapi.Activities.Packrafting.events;

namespace Turboapi.Activities.Packrafting.data;

public sealed class PackraftingActivityCreatedHandler : IEventHandler<PackraftingActivityCreated>
{
    private readonly PackraftingContext _db;
    private readonly IIdempotencyStore<PackraftingContext> _idempotency;
    private readonly ILogger<PackraftingActivityCreatedHandler> _logger;

    public PackraftingActivityCreatedHandler(PackraftingContext db, IIdempotencyStore<PackraftingContext> idempotency, ILogger<PackraftingActivityCreatedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(PackraftingActivityCreated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken)) return;

        var reader = new WKTReader();
        var route = (LineString)reader.Read(@event.RouteWkt);
        if (route.SRID != 4326) route.SRID = 4326;

        var d = @event.Details;
        _db.Activities.Add(new PackraftingActivityEntity
        {
            Id = @event.ActivityId,
            OwnerId = @event.OwnerId,
            Name = @event.Name,
            Description = @event.Description,
            Route = route,
            DistanceMeters = d.DistanceMeters,
            PaddleDistanceMeters = d.PaddleDistanceMeters,
            PortageDistanceMeters = d.PortageDistanceMeters,
            MaxGrade = (short)d.MaxGrade,
            TypicalGrade = (short)d.TypicalGrade,
            PutInLat = d.PutInLat, PutInLon = d.PutInLon,
            TakeOutLat = d.TakeOutLat, TakeOutLon = d.TakeOutLon,
            NveStationCode = d.NveStationCode,
            MinFlowCumecs = d.MinFlowCumecs,
            MaxFlowCumecs = d.MaxFlowCumecs,
            CreatedAt = @event.OccurredAt,
            UpdatedAt = @event.OccurredAt,
            Version = 1,
            Segments = d.Segments.Select((s, i) =>
            {
                var sr = new WKTReader();
                var sg = (LineString)sr.Read(s.PolylineWkt);
                if (sg.SRID != 4326) sg.SRID = 4326;
                return new RouteSegmentEntity
                {
                    ActivityId = @event.ActivityId,
                    Ordinal = i,
                    Kind = (short)s.Kind,
                    Grade = s.Grade is { } g ? (short)g : null,
                    DistanceMeters = s.DistanceMeters,
                    Geometry = sg,
                    Notes = s.Notes,
                };
            }).ToList(),
        });
        await _db.SaveChangesAsync(cancellationToken);
        _logger.LogInformation("Projected packrafting activity {Id}", @event.ActivityId);
    }
}

public sealed class PackraftingActivityUpdatedHandler : IEventHandler<PackraftingActivityUpdated>
{
    private readonly PackraftingContext _db;
    private readonly IIdempotencyStore<PackraftingContext> _idempotency;
    private readonly ILogger<PackraftingActivityUpdatedHandler> _logger;

    public PackraftingActivityUpdatedHandler(PackraftingContext db, IIdempotencyStore<PackraftingContext> idempotency, ILogger<PackraftingActivityUpdatedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(PackraftingActivityUpdated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken)) return;

        var entity = await _db.Activities.Include(a => a.Segments)
            .FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null) { _logger.LogWarning("Packrafting activity {Id} not found for update", @event.ActivityId); return; }
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
            entity.PaddleDistanceMeters = d.PaddleDistanceMeters;
            entity.PortageDistanceMeters = d.PortageDistanceMeters;
            entity.MaxGrade = (short)d.MaxGrade;
            entity.TypicalGrade = (short)d.TypicalGrade;
            entity.PutInLat = d.PutInLat; entity.PutInLon = d.PutInLon;
            entity.TakeOutLat = d.TakeOutLat; entity.TakeOutLon = d.TakeOutLon;
            entity.NveStationCode = d.NveStationCode;
            entity.MinFlowCumecs = d.MinFlowCumecs;
            entity.MaxFlowCumecs = d.MaxFlowCumecs;

            _db.RemoveRange(entity.Segments);
            entity.Segments = d.Segments.Select((s, i) =>
            {
                var sr = new WKTReader();
                var sg = (LineString)sr.Read(s.PolylineWkt);
                if (sg.SRID != 4326) sg.SRID = 4326;
                return new RouteSegmentEntity
                {
                    ActivityId = entity.Id, Ordinal = i,
                    Kind = (short)s.Kind, Grade = s.Grade is { } g ? (short)g : null,
                    DistanceMeters = s.DistanceMeters, Geometry = sg, Notes = s.Notes,
                };
            }).ToList();
        }
        entity.UpdatedAt = @event.OccurredAt;
        entity.Version = @event.Version;
        await _db.SaveChangesAsync(cancellationToken);
    }
}

public sealed class PackraftingActivityDeletedHandler : IEventHandler<PackraftingActivityDeleted>
{
    private readonly PackraftingContext _db;
    private readonly IIdempotencyStore<PackraftingContext> _idempotency;
    private readonly ILogger<PackraftingActivityDeletedHandler> _logger;

    public PackraftingActivityDeletedHandler(PackraftingContext db, IIdempotencyStore<PackraftingContext> idempotency, ILogger<PackraftingActivityDeletedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(PackraftingActivityDeleted @event, CancellationToken cancellationToken)
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
