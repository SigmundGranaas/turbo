using Turboapi.Sharing.value;

namespace Turboapi.Sharing.domain.service;

/// <summary>
/// Owner-only mutation of a resource's coarse visibility
/// (private / friends / unlisted_link / public). The one write the
/// otherwise read-only resource surface exposes: grants change WHO,
/// visibility changes HOW WIDE.
/// </summary>
public interface IResourceVisibilityService
{
    /// <summary>
    /// Sets <paramref name="resourceId"/>'s visibility. No-op (but still
    /// succeeds) when the value is unchanged. Throws
    /// <see cref="Turboapi.Sharing.domain.exception.ResourceNotFoundException"/> /
    /// <see cref="Turboapi.Sharing.domain.exception.AccessDeniedException"/>
    /// when the resource is missing or the actor is not its owner.
    /// </summary>
    Task SetVisibilityAsync(Guid actorId, Guid resourceId, Visibility visibility, CancellationToken cancellationToken = default);
}
