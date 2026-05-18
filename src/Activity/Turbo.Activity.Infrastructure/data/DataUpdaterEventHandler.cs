using System.Diagnostics;
using Microsoft.Extensions.Logging;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activity.domain.events;
using Turboapi.Activity.domain.query;

namespace Turboapi.Activity.data;

public class ActivityEventHandler : IEventHandler<ActivityCreated>, IEventHandler<ActivityUpdated>, IEventHandler<ActivityDeleted>
{
    private readonly IActivityWriteRepository _repo;
    private readonly IIdempotencyStore<ActivityContext> _idempotency;
    private readonly ILogger<ActivityEventHandler> _logger;
    private readonly ActivitySource _activitySource;

    public ActivityEventHandler(
        IActivityWriteRepository repo,
        IIdempotencyStore<ActivityContext> idempotency,
        ILogger<ActivityEventHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("ActivityEventHandler");
    }

    public async Task HandleAsync(ActivityCreated @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Activity Created");
        activity?.SetTag("activity.id", @event.activity);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed ActivityCreated {EventId}", @event.EventId);
            return;
        }

        try
        {
            var entity = new ActivityQueryDto()
            {
                ActivityId = @event.activity,
                OwnerId = @event.OwnerId,
                Position = @event.position,
                Name = @event.name,
                Description = @event.description,
                Icon = @event.icon,
            };

            await _repo.Add(entity);
            _logger.LogInformation("Stored activity {LocationId} for owner {OwnerId}",
                @event.activity, @event.OwnerId);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle Activity event for {LocationId}",
                @event.activity);
            throw;
        }
    }
    public async Task HandleAsync(ActivityUpdated @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Activity Updated");
        activity?.SetTag("activity.id", @event.ActivityId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed ActivityUpdated {EventId}", @event.EventId);
            return;
        }

      try{
            var entity = await  _repo.GetById(@event.ActivityId);
            if (entity == null)
            {
                _logger.LogError("Failed to handle Location event for {ActivityId}, because it does not exist in the Database",
                    @event.ActivityId);
                return;
            }

            entity.Name = @event.name;
            entity.Description = @event.description;
            entity.Icon = @event.icon;
            await _repo.Update(entity);
            _logger.LogInformation("Updated  {ActivityId} for owner {OwnerId}",
                entity.ActivityId, entity.OwnerId);
      }
      catch (Exception ex)
      {
          _logger.LogError(ex, "Failed to handle ActivityUpdated event for {ActivityId}",
              @event.ActivityId);
          throw;
      }
    }
    public async Task HandleAsync(ActivityDeleted @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Activity Created");
        activity?.SetTag("activity.id", @event.activityId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed ActivityDeleted {EventId}", @event.EventId);
            return;
        }

        try
        {
            var entity = await  _repo.GetById(@event.activityId);
            if (entity == null)
            {
                _logger.LogError("Failed to handle Activity event for {ActivityId}, because it does not exist in the Database",
                    @event.activityId);
                return;
            }

            await _repo.Delete(entity);
            _logger.LogInformation("Deleted activity {ActivityId}",
                @event.activityId);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle LocationCreated event for {ActivityId}",
                @event.activityId);
            throw;
        }
    }
}
