using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.Freediving.data.model;
using Turboapi.Activities.Freediving.events;

namespace Turboapi.Activities.Freediving.data;

public sealed class FreedivingActivityCreatedHandler : IEventHandler<FreedivingActivityCreated>
{
    private readonly FreedivingContext _db;
    private readonly IIdempotencyStore<FreedivingContext> _idempotency;
    private readonly ILogger<FreedivingActivityCreatedHandler> _logger;

    public FreedivingActivityCreatedHandler(FreedivingContext db, IIdempotencyStore<FreedivingContext> idempotency, ILogger<FreedivingActivityCreatedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(FreedivingActivityCreated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken)) return;

        var factory = new GeometryFactory(new PrecisionModel(), 4326);
        var d = @event.Details;
        _db.Activities.Add(new FreedivingActivityEntity
        {
            Id = @event.ActivityId,
            OwnerId = @event.OwnerId,
            Name = @event.Name,
            Description = @event.Description,
            Geometry = factory.CreatePoint(new Coordinate(@event.Longitude, @event.Latitude)),
            WaterBody = (short)d.WaterBody,
            BottomType = (short)d.BottomType,
            MaxDepthMeters = d.MaxDepthMeters,
            TypicalVisibilityMeters = d.TypicalVisibilityMeters,
            HarpoonAllowed = d.HarpoonAllowed,
            ShoreEntry = d.ShoreEntry,
            AccessNotes = d.AccessNotes,
            CreatedAt = @event.OccurredAt,
            UpdatedAt = @event.OccurredAt,
            Version = 1,
            TargetSpecies = d.TargetSpecies
                .Select(t => new TargetSpeciesEntity { ActivityId = @event.ActivityId, SpeciesCode = t.SpeciesCode, Notes = t.Notes })
                .ToList(),
        });
        await _db.SaveChangesAsync(cancellationToken);
        _logger.LogInformation("Projected freediving activity {Id}", @event.ActivityId);
    }
}

public sealed class FreedivingActivityUpdatedHandler : IEventHandler<FreedivingActivityUpdated>
{
    private readonly FreedivingContext _db;
    private readonly IIdempotencyStore<FreedivingContext> _idempotency;
    private readonly ILogger<FreedivingActivityUpdatedHandler> _logger;

    public FreedivingActivityUpdatedHandler(FreedivingContext db, IIdempotencyStore<FreedivingContext> idempotency, ILogger<FreedivingActivityUpdatedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(FreedivingActivityUpdated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken)) return;

        var entity = await _db.Activities.Include(a => a.TargetSpecies)
            .FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null) { _logger.LogWarning("Freediving activity {Id} not found for update", @event.ActivityId); return; }
        if (@event.Version <= entity.Version) return;

        if (@event.Name is not null) entity.Name = @event.Name;
        if (@event.Description is not null) entity.Description = @event.Description;
        if (@event.Longitude is not null && @event.Latitude is not null)
        {
            var factory = new GeometryFactory(new PrecisionModel(), 4326);
            entity.Geometry = factory.CreatePoint(new Coordinate(@event.Longitude.Value, @event.Latitude.Value));
        }
        if (@event.Details is { } d)
        {
            entity.WaterBody = (short)d.WaterBody;
            entity.BottomType = (short)d.BottomType;
            entity.MaxDepthMeters = d.MaxDepthMeters;
            entity.TypicalVisibilityMeters = d.TypicalVisibilityMeters;
            entity.HarpoonAllowed = d.HarpoonAllowed;
            entity.ShoreEntry = d.ShoreEntry;
            entity.AccessNotes = d.AccessNotes;
            _db.RemoveRange(entity.TargetSpecies);
            entity.TargetSpecies = d.TargetSpecies
                .Select(t => new TargetSpeciesEntity { ActivityId = entity.Id, SpeciesCode = t.SpeciesCode, Notes = t.Notes })
                .ToList();
        }
        entity.UpdatedAt = @event.OccurredAt;
        entity.Version = @event.Version;
        await _db.SaveChangesAsync(cancellationToken);
    }
}

public sealed class FreedivingActivityDeletedHandler : IEventHandler<FreedivingActivityDeleted>
{
    private readonly FreedivingContext _db;
    private readonly IIdempotencyStore<FreedivingContext> _idempotency;
    private readonly ILogger<FreedivingActivityDeletedHandler> _logger;

    public FreedivingActivityDeletedHandler(FreedivingContext db, IIdempotencyStore<FreedivingContext> idempotency, ILogger<FreedivingActivityDeletedHandler> logger)
    { _db = db; _idempotency = idempotency; _logger = logger; }

    public async Task HandleAsync(FreedivingActivityDeleted @event, CancellationToken cancellationToken)
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
