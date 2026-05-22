using System.Diagnostics;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Geo.data.model;
using Turboapi.Geo.domain.events;
using Turboapi.Geo.domain.query.model;

public interface ILocationEventHandler<in TEvent> : IEventHandler<TEvent> where TEvent : DomainEvent, IDomainEvent
{
}

public class LocationCreatedHandler : ILocationEventHandler<LocationCreated>
{
    private readonly ILocationWriteRepository _repo;
    private readonly IIdempotencyStore<LocationReadContext> _idempotency;
    private readonly ILogger<LocationCreatedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public LocationCreatedHandler(
        ILocationWriteRepository repo,
        IIdempotencyStore<LocationReadContext> idempotency,
        ILogger<LocationCreatedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("LocationCreatedHandler");
    }

    public async Task HandleAsync(LocationCreated @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Location Created");
        activity?.SetTag("location.id", @event.LocationId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed LocationCreated {EventId}", @event.EventId);
            return;
        }

        try
        {
            var factory = new GeometryFactory();
            var entity = new LocationEntity
            {
                Id = @event.LocationId,
                OwnerId = @event.OwnerId,
                Geometry = @event.Coordinates.ToPoint(factory),
                Name = @event.Display.Name,
                Description = @event.Display.Description,
                Icon = @event.Display.Icon,
                CreatedAt = @event.OccurredAt,
                UpdatedAt = @event.OccurredAt,
                DeletedAt = null,
                Version = 1,
            };

            await _repo.Add(entity);
            _logger.LogInformation("Created location {LocationId} for owner {OwnerId}",
                @event.LocationId, @event.OwnerId);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle LocationCreated event for {LocationId}",
                @event.LocationId);
            throw;
        }
    }
}

public class LocationUpdatedHandler : ILocationEventHandler<LocationUpdated>
{
    private readonly ILocationWriteRepository _repo;
    private readonly IIdempotencyStore<LocationReadContext> _idempotency;
    private readonly ILogger<LocationUpdatedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public LocationUpdatedHandler(
        ILocationWriteRepository repo,
        IIdempotencyStore<LocationReadContext> idempotency,
        ILogger<LocationUpdatedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("LocationUpdatedHandler");
    }

    public async Task HandleAsync(LocationUpdated @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle location updated");
        activity?.SetTag("location.id", @event.LocationId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed LocationUpdated {EventId}", @event.EventId);
            return;
        }

        try
        {
            if (@event.Updates.HasAnyChange)
            {
                await _repo.UpdatePartial(
                    @event.LocationId,
                    @event.Updates.Coordinates,
                    @event.Updates.Display,
                    @event.OccurredAt);
            }
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle LocationDisplayInformationChanged event for {LocationId}",
                @event.LocationId);
            throw;
        }
    }
}

public class LocationDeletedHandler : ILocationEventHandler<LocationDeleted>
{
    private readonly ILocationWriteRepository _repo;
    private readonly IIdempotencyStore<LocationReadContext> _idempotency;
    private readonly ILogger<LocationDeletedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public LocationDeletedHandler(
        ILocationWriteRepository repo,
        IIdempotencyStore<LocationReadContext> idempotency,
        ILogger<LocationDeletedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("LocationDeletedHandler");
    }

    public async Task HandleAsync(LocationDeleted @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Location Deleted");
        activity?.SetTag("location.id", @event.LocationId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed LocationDeleted {EventId}", @event.EventId);
            return;
        }

        try
        {
            await _repo.SoftDelete(@event.LocationId, @event.OccurredAt);
            _logger.LogInformation("Tombstoned location {LocationId}", @event.LocationId);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle LocationDeleted event for {LocationId}",
                @event.LocationId);
            throw;
        }
    }
}
