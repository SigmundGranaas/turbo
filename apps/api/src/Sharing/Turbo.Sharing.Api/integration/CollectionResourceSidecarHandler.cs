using Microsoft.EntityFrameworkCore;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Collections.domain.events;
using Turboapi.Sharing.data;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.integration;

/// <summary>
/// Maintains the Resource sidecar for the Collections module. When a
/// CollectionCreated event arrives, upsert a Resource keyed on the same
/// id. When a CollectionDeleted event arrives, soft-delete the Resource.
///
/// This is the integration point that lets Sharing operate on Collections
/// without touching the Collections module itself. The Collection retains
/// its OwnerId column for the existing CQRS path; the Sharing service
/// holds the single source of truth for who can read/write the collection
/// once grants come into play.
///
/// Idempotent: re-running the same event is safe via the
/// IIdempotencyStore on the Sharing read context.
/// </summary>
public sealed class CollectionResourceSidecarHandler
    : IEventHandler<CollectionCreated>,
      IEventHandler<CollectionDeleted>
{
    private readonly SharingReadContext _db;
    private readonly IIdempotencyStore<SharingReadContext> _idempotency;
    private readonly ILogger<CollectionResourceSidecarHandler> _logger;

    public CollectionResourceSidecarHandler(
        SharingReadContext db,
        IIdempotencyStore<SharingReadContext> idempotency,
        ILogger<CollectionResourceSidecarHandler> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task HandleAsync(CollectionCreated @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed CollectionCreated {EventId}", @event.EventId);
            return;
        }

        var existing = await _db.Resources
            .FirstOrDefaultAsync(r => r.Id == @event.CollectionId, cancellationToken);
        if (existing is not null)
        {
            // Someone else created the resource (manual backfill, e.g.) —
            // leave it alone but ensure the owner matches.
            if (existing.OwnerId != @event.OwnerId)
                _logger.LogWarning(
                    "Resource {ResourceId} already exists with different owner; not overwriting",
                    @event.CollectionId);
            return;
        }

        _db.Resources.Add(new ResourceEntity
        {
            Id = @event.CollectionId,
            Type = ResourceType.Collection,
            OwnerId = @event.OwnerId,
            Visibility = Visibility.Private.ToWire(),
            Version = 1,
            UpdatedAt = @event.OccurredAt,
        });
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task HandleAsync(CollectionDeleted @event, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(@event.EventId, cancellationToken))
            return;

        var existing = await _db.Resources
            .FirstOrDefaultAsync(r => r.Id == @event.CollectionId, cancellationToken);
        if (existing is null) return;
        existing.DeletedAt = @event.OccurredAt;
        existing.UpdatedAt = @event.OccurredAt;
        existing.Version += 1;
        await _db.SaveChangesAsync(cancellationToken);
    }
}
