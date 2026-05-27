using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Sharing.domain.service;

namespace Turboapi.Sharing.controller;

/// <summary>
/// Read-only view of the Resource envelope. Used by clients to discover
/// resources they can see (owned + shared) and to delta-sync.
/// </summary>
[ApiController]
[Route("api/sharing/resources")]
[Authorize]
public class ResourcesController : ControllerBase
{
    private const int DefaultLimit = 500;

    private readonly IResourceSyncService _sync;

    public ResourcesController(IResourceSyncService sync) => _sync = sync;

    private Guid GetAuthenticatedUserId() =>
        Guid.Parse(User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not found in token"));

    /// <summary>
    /// Returns the resource envelopes visible to the calling user that have
    /// changed strictly after <paramref name="since"/>. Without
    /// <paramref name="since"/> returns the full current set.
    /// <paramref name="types"/> is a comma-separated allow-list of resource
    /// types (eg <c>collection,marker,path</c>); omitted = all types.
    /// </summary>
    [HttpGet("sync")]
    public async Task<ActionResult<ResourceSyncPage>> Sync(
        [FromQuery] DateTime? since,
        [FromQuery] string? types,
        [FromQuery] int? limit)
    {
        var userId = GetAuthenticatedUserId();
        var typeList = string.IsNullOrWhiteSpace(types)
            ? null
            : (IReadOnlyCollection<string>)types.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        var effectiveLimit = limit ?? DefaultLimit;
        return Ok(await _sync.SyncAsync(userId, since, typeList, effectiveLimit));
    }
}
