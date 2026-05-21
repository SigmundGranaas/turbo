using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.XcSki.data.model;
using Turboapi.Activities.XcSki.events;

namespace Turboapi.Activities.XcSki.data;

public sealed class XcSkiActivityCreatedHandler : IEventHandler<XcSkiActivityCreated>
{
    private readonly XcSkiContext _db;
    private readonly IIdempotencyStore<XcSkiContext> _idempotency;
    private readonly ILogger<XcSkiActivityCreatedHandler> _logger;

    public XcSkiActivityCreatedHandler(XcSkiContext db, IIdempotencyStore<XcSkiContext> idempotency, ILogger<XcSkiActivityCreatedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(XcSkiActivityCreated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken)) return;

        var reader = new WKTReader();
        var route = (LineString)reader.Read(@event.RouteWkt);
        if (route.SRID != 4326) route.SRID = 4326;

        var d = @event.Details;
        _db.Activities.Add(new XcSkiActivityEntity
        {
            Id = @event.ActivityId,
            OwnerId = @event.OwnerId,
            Name = @event.Name,
            Description = @event.Description,
            Route = route,
            DistanceMeters = d.DistanceMeters,
            AscentMeters = d.AscentMeters,
            DescentMeters = d.DescentMeters,
            Technique = (short)d.Technique,
            GroomingStatus = (short)d.GroomingStatus,
            IsLit = d.IsLit,
            RequiresSeasonPass = d.RequiresSeasonPass,
            GroomingFeedKey = d.GroomingFeedKey,
            CreatedAt = @event.OccurredAt,
            UpdatedAt = @event.OccurredAt,
            Version = 1,
        });
        await _db.SaveChangesAsync(cancellationToken);
        _logger.LogInformation("Projected XC ski activity {Id}", @event.ActivityId);
    }
}

public sealed class XcSkiActivityUpdatedHandler : IEventHandler<XcSkiActivityUpdated>
{
    private readonly XcSkiContext _db;
    private readonly IIdempotencyStore<XcSkiContext> _idempotency;
    private readonly ILogger<XcSkiActivityUpdatedHandler> _logger;

    public XcSkiActivityUpdatedHandler(XcSkiContext db, IIdempotencyStore<XcSkiContext> idempotency, ILogger<XcSkiActivityUpdatedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(XcSkiActivityUpdated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken)) return;

        var entity = await _db.Activities.FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null) { _logger.LogWarning("XC ski activity {Id} not found for update", @event.ActivityId); return; }
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
            entity.Technique = (short)d.Technique;
            entity.GroomingStatus = (short)d.GroomingStatus;
            entity.IsLit = d.IsLit;
            entity.RequiresSeasonPass = d.RequiresSeasonPass;
            entity.GroomingFeedKey = d.GroomingFeedKey;
        }
        entity.UpdatedAt = @event.OccurredAt;
        entity.Version = @event.Version;
        await _db.SaveChangesAsync(cancellationToken);
    }
}

public sealed class XcSkiActivityDeletedHandler : IEventHandler<XcSkiActivityDeleted>
{
    private readonly XcSkiContext _db;
    private readonly IIdempotencyStore<XcSkiContext> _idempotency;
    private readonly ILogger<XcSkiActivityDeletedHandler> _logger;

    public XcSkiActivityDeletedHandler(XcSkiContext db, IIdempotencyStore<XcSkiContext> idempotency, ILogger<XcSkiActivityDeletedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(XcSkiActivityDeleted @event, CancellationToken cancellationToken)
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
