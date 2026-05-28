using Microsoft.EntityFrameworkCore;
using Turbo.Outbox;
using Turboapi.Sharing.data;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.integration;

/// <summary>
/// Idempotent operations that maintain the Resource sidecar for any
/// payload module. Each module's typed handler unpacks its event into
/// (resourceId, ownerId, occurredAt) and calls into here. Centralizes
/// the upsert / soft-delete / safety checks so adding a new shareable
/// type is just declaring an event handler that calls one of these
/// methods.
/// </summary>
public sealed class ResourceSidecar
{
    private readonly SharingReadContext _db;
    private readonly IIdempotencyStore<SharingReadContext> _idempotency;
    private readonly ILogger<ResourceSidecar> _logger;

    public ResourceSidecar(
        SharingReadContext db,
        IIdempotencyStore<SharingReadContext> idempotency,
        ILogger<ResourceSidecar> logger)
    {
        _db = db;
        _idempotency = idempotency;
        _logger = logger;
    }

    public async Task EnsureCreatedAsync(
        Guid eventId, Guid resourceId, string type, Guid ownerId, DateTime occurredAt,
        CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(eventId, cancellationToken))
        {
            _logger.LogDebug("Skipping already-processed sidecar event {EventId}", eventId);
            return;
        }

        var existing = await _db.Resources
            .FirstOrDefaultAsync(r => r.Id == resourceId, cancellationToken);
        if (existing is not null)
        {
            if (existing.OwnerId != ownerId)
                _logger.LogWarning(
                    "Resource {ResourceId} ({Type}) already exists with different owner; not overwriting",
                    resourceId, type);
            return;
        }

        _db.Resources.Add(new ResourceEntity
        {
            Id = resourceId,
            Type = type,
            OwnerId = ownerId,
            Visibility = Visibility.Private.ToWire(),
            Version = 1,
            UpdatedAt = occurredAt,
        });
        await _db.SaveChangesAsync(cancellationToken);
    }

    public async Task SoftDeleteAsync(Guid eventId, Guid resourceId, DateTime occurredAt, CancellationToken cancellationToken)
    {
        if (!await _idempotency.TryMarkProcessedAsync(eventId, cancellationToken))
            return;

        var existing = await _db.Resources
            .FirstOrDefaultAsync(r => r.Id == resourceId, cancellationToken);
        if (existing is null) return;
        existing.DeletedAt = occurredAt;
        existing.UpdatedAt = occurredAt;
        existing.Version += 1;
        await _db.SaveChangesAsync(cancellationToken);
    }
}
