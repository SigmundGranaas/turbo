using System.Diagnostics;
using Microsoft.Extensions.Logging;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Collections.data.model;
using Turboapi.Collections.domain.events;
using Turboapi.Collections.domain.query;

namespace Turboapi.Collections.data;

public interface ICollectionEventHandler<in TEvent> : IEventHandler<TEvent>
    where TEvent : DomainEvent, IDomainEvent
{
}

public class CollectionCreatedHandler : ICollectionEventHandler<CollectionCreated>
{
    private readonly ICollectionWriteRepository _repo;
    private readonly IIdempotencyStore<CollectionsReadContext> _idempotency;
    private readonly ILogger<CollectionCreatedHandler> _logger;
    private readonly ActivitySource _activitySource;

    public CollectionCreatedHandler(
        ICollectionWriteRepository repo,
        IIdempotencyStore<CollectionsReadContext> idempotency,
        ILogger<CollectionCreatedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
        _activitySource = new ActivitySource("CollectionCreatedHandler");
    }

    public async Task HandleAsync(CollectionCreated @event, CancellationToken cancellationToken)
    {
        using var activity = _activitySource.StartActivity("Handle Collection Created");
        activity?.SetTag("collection.id", @event.CollectionId);

        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed CollectionCreated {EventId}", @event.EventId);
            return;
        }

        try
        {
            var entity = new CollectionEntity
            {
                Id = @event.CollectionId,
                OwnerId = @event.OwnerId,
                Name = @event.Metadata.Name,
                Description = @event.Metadata.Description,
                ColorHex = @event.Metadata.ColorHex,
                IconKey = @event.Metadata.IconKey,
                SortOrder = @event.Metadata.SortOrder,
                SavedFilterJson = @event.Metadata.SavedFilterJson,
                CreatedAt = @event.OccurredAt,
                UpdatedAt = @event.OccurredAt,
                DeletedAt = null,
                Version = 1,
            };
            await _repo.Add(entity);
            _logger.LogInformation("Created collection {CollectionId}", @event.CollectionId);
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to handle CollectionCreated for {CollectionId}", @event.CollectionId);
            throw;
        }
    }
}

public class CollectionUpdatedHandler : ICollectionEventHandler<CollectionUpdated>
{
    private readonly ICollectionWriteRepository _repo;
    private readonly IIdempotencyStore<CollectionsReadContext> _idempotency;
    private readonly ILogger<CollectionUpdatedHandler> _logger;

    public CollectionUpdatedHandler(
        ICollectionWriteRepository repo,
        IIdempotencyStore<CollectionsReadContext> idempotency,
        ILogger<CollectionUpdatedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(CollectionUpdated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed CollectionUpdated {EventId}", @event.EventId);
            return;
        }

        var existing = await _repo.GetById(@event.CollectionId);
        if (existing is null)
        {
            _logger.LogWarning("Collection {CollectionId} not found for update", @event.CollectionId);
            return;
        }

        var u = @event.Updates;
        var updated = new CollectionEntity
        {
            Id = existing.Id,
            OwnerId = existing.OwnerId,
            Name = u.Name ?? existing.Name,
            Description = u.Description ?? existing.Description,
            ColorHex = u.ColorHex ?? existing.ColorHex,
            IconKey = u.IconKey ?? existing.IconKey,
            SortOrder = u.SortOrder ?? existing.SortOrder,
            SavedFilterJson = u.ClearSavedFilter ? null : (u.SavedFilterJson ?? existing.SavedFilterJson),
            CreatedAt = existing.CreatedAt,
            UpdatedAt = @event.OccurredAt,
            DeletedAt = existing.DeletedAt,
            Version = existing.Version,
        };

        await _repo.UpdateMetadata(@event.CollectionId, updated, @event.OccurredAt);
        _logger.LogInformation("Updated collection {CollectionId}", @event.CollectionId);
    }
}

public class CollectionDeletedHandler : ICollectionEventHandler<CollectionDeleted>
{
    private readonly ICollectionWriteRepository _repo;
    private readonly IIdempotencyStore<CollectionsReadContext> _idempotency;
    private readonly ILogger<CollectionDeletedHandler> _logger;

    public CollectionDeletedHandler(
        ICollectionWriteRepository repo,
        IIdempotencyStore<CollectionsReadContext> idempotency,
        ILogger<CollectionDeletedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(CollectionDeleted @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed CollectionDeleted {EventId}", @event.EventId);
            return;
        }
        await _repo.SoftDelete(@event.CollectionId, @event.OccurredAt);
        _logger.LogInformation("Tombstoned collection {CollectionId}", @event.CollectionId);
    }
}

public class CollectionItemAddedHandler : ICollectionEventHandler<CollectionItemAdded>
{
    private readonly ICollectionWriteRepository _repo;
    private readonly IIdempotencyStore<CollectionsReadContext> _idempotency;
    private readonly ILogger<CollectionItemAddedHandler> _logger;

    public CollectionItemAddedHandler(
        ICollectionWriteRepository repo,
        IIdempotencyStore<CollectionsReadContext> idempotency,
        ILogger<CollectionItemAddedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(CollectionItemAdded @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed CollectionItemAdded {EventId}", @event.EventId);
            return;
        }
        await _repo.AddItem(@event.CollectionId, @event.Item.Type, @event.Item.Uuid, @event.OccurredAt);
        _logger.LogInformation(
            "Added {ItemType}:{ItemUuid} to collection {CollectionId}",
            @event.Item.Type, @event.Item.Uuid, @event.CollectionId);
    }
}

public class CollectionItemRemovedHandler : ICollectionEventHandler<CollectionItemRemoved>
{
    private readonly ICollectionWriteRepository _repo;
    private readonly IIdempotencyStore<CollectionsReadContext> _idempotency;
    private readonly ILogger<CollectionItemRemovedHandler> _logger;

    public CollectionItemRemovedHandler(
        ICollectionWriteRepository repo,
        IIdempotencyStore<CollectionsReadContext> idempotency,
        ILogger<CollectionItemRemovedHandler> logger)
    {
        _repo = repo;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(CollectionItemRemoved @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed CollectionItemRemoved {EventId}", @event.EventId);
            return;
        }
        await _repo.RemoveItem(@event.CollectionId, @event.Item.Type, @event.Item.Uuid, @event.OccurredAt);
        _logger.LogInformation(
            "Removed {ItemType}:{ItemUuid} from collection {CollectionId}",
            @event.Item.Type, @event.Item.Uuid, @event.CollectionId);
    }
}
