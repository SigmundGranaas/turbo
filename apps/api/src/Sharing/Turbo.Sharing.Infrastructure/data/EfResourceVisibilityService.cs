using Microsoft.EntityFrameworkCore;
using Turboapi.Sharing.domain.exception;
using Turboapi.Sharing.domain.model;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.data;

public sealed class EfResourceVisibilityService : IResourceVisibilityService
{
    private readonly SharingReadContext _db;

    public EfResourceVisibilityService(SharingReadContext db) => _db = db;

    public async Task SetVisibilityAsync(Guid actorId, Guid resourceId, Visibility visibility, CancellationToken cancellationToken = default)
    {
        var entity = await _db.Resources
            .Where(r => r.Id == resourceId && r.DeletedAt == null)
            .FirstOrDefaultAsync(cancellationToken)
            ?? throw new ResourceNotFoundException(resourceId);
        if (entity.OwnerId != actorId)
            throw new AccessDeniedException(actorId, resourceId);

        var next = visibility.ToWire();
        if (entity.Visibility == next) return;

        // Mirrors Resource.ChangeVisibility: the change bumps the version so
        // delta-syncing clients pick the envelope up on their next pull.
        entity.Visibility = next;
        entity.Version += 1;
        entity.UpdatedAt = DateTime.UtcNow;
        await _db.SaveChangesAsync(cancellationToken);
    }
}
