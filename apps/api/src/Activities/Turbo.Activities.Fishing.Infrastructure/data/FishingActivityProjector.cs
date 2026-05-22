using System.Diagnostics;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.Fishing.data.model;
using Turboapi.Activities.Fishing.events;
using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.data;

/// <summary>
/// Read-model subscriber for the typed fishing.activities table. Mirror
/// of Tracks' TrackCreatedHandler / TrackUpdatedHandler / TrackDeletedHandler
/// pattern — each event type gets its own idempotent handler.
/// </summary>
public sealed class FishingActivityCreatedHandler : IEventHandler<FishingActivityCreated>
{
    private readonly FishingContext _db;
    private readonly IIdempotencyStore<FishingContext> _idempotency;
    private readonly ILogger<FishingActivityCreatedHandler> _logger;
    private readonly ActivitySource _trace;

    public FishingActivityCreatedHandler(
        FishingContext db,
        IIdempotencyStore<FishingContext> idempotency,
        ILogger<FishingActivityCreatedHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
        _trace = new ActivitySource("FishingActivityCreatedHandler");
    }

    public async Task HandleAsync(FishingActivityCreated @event, CancellationToken cancellationToken)
    {
        using var span = _trace.StartActivity("Handle FishingActivityCreated");
        span?.SetTag("activity.id", @event.ActivityId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed FishingActivityCreated {EventId}", @event.EventId);
            return;
        }

        var factory = new GeometryFactory(new PrecisionModel(), 4326);
        var entity = new FishingActivityEntity
        {
            Id = @event.ActivityId,
            OwnerId = @event.OwnerId,
            Name = @event.Name,
            Description = @event.Description,
            Geometry = factory.CreatePoint(new Coordinate(@event.Longitude, @event.Latitude)),
            WaterKind = (short)@event.Details.WaterKind,
            ShoreOrBoat = (short)@event.Details.ShoreOrBoat,
            AccessNotes = @event.Details.AccessNotes,
            PreferredPressureMinHpa = @event.Details.Preferred?.PressureMinHpa,
            PreferredPressureMaxHpa = @event.Details.Preferred?.PressureMaxHpa,
            PreferredWindMaxMs = @event.Details.Preferred?.WindMaxMs,
            CreatedAt = @event.OccurredAt,
            UpdatedAt = @event.OccurredAt,
            DeletedAt = null,
            Version = 1,
            TargetSpecies = @event.Details.TargetSpecies
                .Select(s => new TargetSpeciesEntity
                {
                    ActivityId = @event.ActivityId,
                    SpeciesCode = s.SpeciesCode,
                    Notes = s.Notes,
                })
                .ToList(),
            DepthSamples = @event.Details.KnownDepths
                .Select((d, i) => new DepthSampleEntity
                {
                    ActivityId = @event.ActivityId,
                    Ordinal = i,
                    Lat = d.Lat,
                    Lon = d.Lon,
                    DepthMeters = d.DepthMeters,
                })
                .ToList(),
        };

        _db.Activities.Add(entity);
        await _db.SaveChangesAsync(cancellationToken);
        _logger.LogInformation("Projected fishing activity {Id} for owner {OwnerId}", entity.Id, entity.OwnerId);
    }
}

public sealed class FishingActivityUpdatedHandler : IEventHandler<FishingActivityUpdated>
{
    private readonly FishingContext _db;
    private readonly IIdempotencyStore<FishingContext> _idempotency;
    private readonly ILogger<FishingActivityUpdatedHandler> _logger;

    public FishingActivityUpdatedHandler(
        FishingContext db,
        IIdempotencyStore<FishingContext> idempotency,
        ILogger<FishingActivityUpdatedHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(FishingActivityUpdated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed FishingActivityUpdated {EventId}", @event.EventId);
            return;
        }

        var entity = await _db.Activities
            .Include(a => a.TargetSpecies)
            .Include(a => a.DepthSamples)
            .FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null)
        {
            _logger.LogWarning(
                "Fishing activity {Id} not found for update; create projection may not have arrived yet",
                @event.ActivityId);
            return;
        }
        if (@event.Version <= entity.Version)
        {
            _logger.LogDebug(
                "Stale FishingActivityUpdated for {Id}: event v={EV} row v={RV}",
                @event.ActivityId, @event.Version, entity.Version);
            return;
        }

        if (@event.Name is not null) entity.Name = @event.Name;
        if (@event.Description is not null) entity.Description = @event.Description;
        if (@event.Longitude is not null && @event.Latitude is not null)
        {
            var factory = new GeometryFactory(new PrecisionModel(), 4326);
            entity.Geometry = factory.CreatePoint(new Coordinate(@event.Longitude.Value, @event.Latitude.Value));
        }
        if (@event.Details is { } d)
        {
            entity.WaterKind = (short)d.WaterKind;
            entity.ShoreOrBoat = (short)d.ShoreOrBoat;
            entity.AccessNotes = d.AccessNotes;
            entity.PreferredPressureMinHpa = d.Preferred?.PressureMinHpa;
            entity.PreferredPressureMaxHpa = d.Preferred?.PressureMaxHpa;
            entity.PreferredWindMaxMs = d.Preferred?.WindMaxMs;

            // Replace owned collections wholesale — typed delete + re-add.
            _db.RemoveRange(entity.TargetSpecies);
            _db.RemoveRange(entity.DepthSamples);
            entity.TargetSpecies = d.TargetSpecies
                .Select(s => new TargetSpeciesEntity
                {
                    ActivityId = entity.Id,
                    SpeciesCode = s.SpeciesCode,
                    Notes = s.Notes,
                })
                .ToList();
            entity.DepthSamples = d.KnownDepths
                .Select((ds, i) => new DepthSampleEntity
                {
                    ActivityId = entity.Id,
                    Ordinal = i,
                    Lat = ds.Lat,
                    Lon = ds.Lon,
                    DepthMeters = ds.DepthMeters,
                })
                .ToList();
        }

        entity.UpdatedAt = @event.OccurredAt;
        entity.Version = @event.Version;
        await _db.SaveChangesAsync(cancellationToken);
        _logger.LogInformation("Updated fishing activity {Id} to v{V}", entity.Id, entity.Version);
    }
}

public sealed class FishingActivityDeletedHandler : IEventHandler<FishingActivityDeleted>
{
    private readonly FishingContext _db;
    private readonly IIdempotencyStore<FishingContext> _idempotency;
    private readonly ILogger<FishingActivityDeletedHandler> _logger;

    public FishingActivityDeletedHandler(
        FishingContext db,
        IIdempotencyStore<FishingContext> idempotency,
        ILogger<FishingActivityDeletedHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(FishingActivityDeleted @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed FishingActivityDeleted {EventId}", @event.EventId);
            return;
        }

        var entity = await _db.Activities.FirstOrDefaultAsync(a => a.Id == @event.ActivityId, cancellationToken);
        if (entity is null)
        {
            _logger.LogDebug("Tombstone for unknown fishing activity {Id}", @event.ActivityId);
            return;
        }
        entity.DeletedAt = @event.OccurredAt;
        entity.UpdatedAt = @event.OccurredAt;
        entity.Version += 1;
        await _db.SaveChangesAsync(cancellationToken);
    }
}
