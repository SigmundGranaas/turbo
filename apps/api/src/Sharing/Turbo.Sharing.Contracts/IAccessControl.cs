using Turboapi.Sharing.value;

namespace Turboapi.Sharing;

/// <summary>
/// The single authorization gate every payload module funnels through.
/// Resolves "can user U read/write resource R" against the resource's
/// owner, the user's direct grants, the user's group memberships, and
/// the resource's coarse visibility.
///
/// Lives in Contracts (not Core) so payload modules — Collections,
/// Geo, Tracks — can depend on the interface without taking a
/// transitive dependency on Sharing's domain or persistence layers.
/// Implementations live in Sharing.Infrastructure; payload-module
/// hosts wire one up in their DI.
/// </summary>
public interface IAccessControl
{
    /// <summary>
    /// True if <paramref name="userId"/> can see <paramref name="resourceId"/>.
    /// Includes owner, any active user grant, group grants where the user is a
    /// member, and public visibility.
    /// </summary>
    Task<bool> CanReadAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default);

    /// <summary>
    /// True if <paramref name="userId"/> can mutate
    /// <paramref name="resourceId"/>. Owner or any active editor grant
    /// (direct or via group). Link grants do not confer write access by
    /// default; if they should in a future flow, resolve them at the HTTP
    /// boundary before reaching here.
    /// </summary>
    Task<bool> CanWriteAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default);

    /// <summary>
    /// Returns the effective role of <paramref name="userId"/> on
    /// <paramref name="resourceId"/>: <c>Owner</c>, the explicit grant role,
    /// or <c>null</c> if no access.
    /// </summary>
    Task<EffectiveRole?> EffectiveRoleAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default);

    /// <summary>Throws <see cref="AccessDeniedException"/> if write is denied.</summary>
    Task RequireWriteAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default);

    /// <summary>Throws <see cref="AccessDeniedException"/> if read is denied.</summary>
    Task RequireReadAsync(Guid userId, Guid resourceId, CancellationToken cancellationToken = default);
}

/// <summary>
/// The role a user effectively holds on a resource. <see cref="Owner"/> is
/// not a grant role — it's the resource's <c>owner_id</c> match.
/// </summary>
public enum EffectiveRole
{
    Viewer = 0,
    Editor = 1,
    Owner = 2,
}

public static class EffectiveRoleExtensions
{
    public static string ToWire(this EffectiveRole role) => role switch
    {
        EffectiveRole.Viewer => "viewer",
        EffectiveRole.Editor => "editor",
        EffectiveRole.Owner => "owner",
        _ => throw new ArgumentOutOfRangeException(nameof(role), role, null),
    };

    public static bool AllowsWrite(this EffectiveRole role)
        => role is EffectiveRole.Editor or EffectiveRole.Owner;

    public static EffectiveRole Promote(this EffectiveRole a, EffectiveRole b)
        => (EffectiveRole)Math.Max((int)a, (int)b);

    public static EffectiveRole FromGrant(Role grantRole) => grantRole switch
    {
        Role.Viewer => EffectiveRole.Viewer,
        Role.Editor => EffectiveRole.Editor,
        _ => throw new ArgumentOutOfRangeException(nameof(grantRole), grantRole, null),
    };
}

/// <summary>
/// Thrown by <see cref="IAccessControl.RequireWriteAsync"/> /
/// <see cref="IAccessControl.RequireReadAsync"/> when the caller has
/// no applicable grant on the resource. Payload modules catch this and
/// translate to HTTP 403.
/// </summary>
public sealed class AccessDeniedException : Exception
{
    public AccessDeniedException(Guid userId, Guid resourceId)
        : base($"User {userId} is not authorized for resource {resourceId}.") { }
}
