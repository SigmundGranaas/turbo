using System.Diagnostics;
using Microsoft.Extensions.Logging;
using NetTopologySuite.Geometries;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Tracks.data;
using Turboapi.Tracks.data.model;
using Turboapi.Tracks.domain.events;
using Turboapi.Tracks.domain.query;
using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.data;

public interface ITrackEventHandler<in TEvent> : IEventHandler<TEvent>
    where TEvent : DomainEvent, IDomainEvent
{
}

public class TrackCreatedHandler : ITrackEventHandler<TrackCreated>
{
    private readonly ITrackWriteRepository _repo;
    private readonly IIdempotencyStore<TrackReadContext> _idempotency;
    private readonly ILogger<TrackCreatedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public TrackCreatedHandler(
        ITrackWriteRepository repo,
        IIdempotencyStore<TrackReadContext> idempotency,
        ILogger<TrackCreatedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("TrackCreatedHandler");
    }

    public async Task HandleAsync(TrackCreated @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Track Created");
        activity?.SetTag("track.id", @event.TrackId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed TrackCreated {EventId}", @event.EventId);
            return;
        }

        try
        {
            var factory = new GeometryFactory(new PrecisionModel(), 4326);
            var entity = new TrackEntity
            {
                Id = @event.TrackId,
                OwnerId = @event.OwnerId,
                Geometry = @event.Geometry.ToLineString(factory),
                Elevations = @event.Geometry.Elevations?.ToArray(),
                Name = @event.Metadata.Name,
                Description = @event.Metadata.Description,
                ColorHex = @event.Metadata.ColorHex,
                IconKey = @event.Metadata.IconKey,
                LineStyleKey = @event.Metadata.LineStyleKey,
                Smoothing = @event.Metadata.Smoothing,
                DistanceMeters = @event.Stats.DistanceMeters,
                AscentMeters = @event.Stats.AscentMeters,
                DescentMeters = @event.Stats.DescentMeters,
                MovingTimeSeconds = @event.Stats.MovingTimeSeconds,
                RecordedAt = @event.Stats.RecordedAt,
                CreatedAt = @event.OccurredAt,
                UpdatedAt = @event.OccurredAt,
                DeletedAt = null,
                Version = 1,
            };
            await _repo.Add(entity);
            _logger.LogInformation("Created track {TrackId} for owner {OwnerId}",
                @event.TrackId, @event.OwnerId);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle TrackCreated for {TrackId}", @event.TrackId);
            throw;
        }
    }
}

public class TrackUpdatedHandler : ITrackEventHandler<TrackUpdated>
{
    private readonly ITrackWriteRepository _repo;
    private readonly IIdempotencyStore<TrackReadContext> _idempotency;
    private readonly ILogger<TrackUpdatedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public TrackUpdatedHandler(
        ITrackWriteRepository repo,
        IIdempotencyStore<TrackReadContext> idempotency,
        ILogger<TrackUpdatedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("TrackUpdatedHandler");
    }

    public async Task HandleAsync(TrackUpdated @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Track Updated");
        activity?.SetTag("track.id", @event.TrackId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed TrackUpdated {EventId}", @event.EventId);
            return;
        }

        var entity = await _repo.GetById(@event.TrackId);
        if (entity is null)
        {
            _logger.LogWarning("Track {TrackId} not found for update; the create event may not have projected yet",
                @event.TrackId);
            return;
        }

        try
        {
            if (@event.Updates.Geometry is not null)
            {
                var factory = new GeometryFactory(new PrecisionModel(), 4326);
                entity.Geometry = @event.Updates.Geometry.ToLineString(factory);
                entity.Elevations = @event.Updates.Geometry.Elevations?.ToArray();
            }
            if (@event.Updates.Metadata is { } m)
            {
                if (m.Name is not null) entity.Name = m.Name;
                if (m.Description is not null) entity.Description = m.Description;
                if (m.ColorHex is not null) entity.ColorHex = m.ColorHex;
                if (m.IconKey is not null) entity.IconKey = m.IconKey;
                if (m.LineStyleKey is not null) entity.LineStyleKey = m.LineStyleKey;
                if (m.Smoothing is not null) entity.Smoothing = m.Smoothing.Value;
            }
            if (@event.Updates.Stats is { } s)
            {
                entity.DistanceMeters = s.DistanceMeters;
                entity.AscentMeters = s.AscentMeters;
                entity.DescentMeters = s.DescentMeters;
                entity.MovingTimeSeconds = s.MovingTimeSeconds;
                entity.RecordedAt = s.RecordedAt;
            }
            entity.UpdatedAt = @event.OccurredAt;
            entity.Version += 1;
            await _repo.Update(entity);
            _logger.LogInformation("Updated track {TrackId} to version {Version}", entity.Id, entity.Version);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle TrackUpdated for {TrackId}", @event.TrackId);
            throw;
        }
    }
}

public class TrackDeletedHandler : ITrackEventHandler<TrackDeleted>
{
    private readonly ITrackWriteRepository _repo;
    private readonly IIdempotencyStore<TrackReadContext> _idempotency;
    private readonly ILogger<TrackDeletedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public TrackDeletedHandler(
        ITrackWriteRepository repo,
        IIdempotencyStore<TrackReadContext> idempotency,
        ILogger<TrackDeletedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("TrackDeletedHandler");
    }

    public async Task HandleAsync(TrackDeleted @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Track Deleted");
        activity?.SetTag("track.id", @event.TrackId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed TrackDeleted {EventId}", @event.EventId);
            return;
        }

        try
        {
            await _repo.SoftDelete(@event.TrackId, @event.OccurredAt);
            _logger.LogInformation("Tombstoned track {TrackId}", @event.TrackId);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle TrackDeleted for {TrackId}", @event.TrackId);
            throw;
        }
    }
}
