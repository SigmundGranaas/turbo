using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Sharing.controller.request;
using Turboapi.Sharing.controller.response;
using Turboapi.Sharing.domain.exception;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.controller;

/// <summary>
/// The Resource envelope surface. Read side: discover resources the caller
/// can see (owned + shared) and delta-sync. Write side: the ONE owner
/// mutation the envelope carries — visibility (grants change WHO can see a
/// resource; visibility changes HOW WIDE it is by default).
/// </summary>
[ApiController]
[Route("api/sharing/resources")]
[Authorize]
public class ResourcesController : ControllerBase
{
    private const int DefaultLimit = 500;

    private readonly IResourceSyncService _sync;
    private readonly IResourceVisibilityService _visibility;

    public ResourcesController(IResourceSyncService sync, IResourceVisibilityService visibility)
    {
        _sync = sync;
        _visibility = visibility;
    }

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

    /// <summary>
    /// Sets a resource's visibility (owner only). Body:
    /// <c>{"visibility": "private" | "friends" | "unlisted_link" | "public"}</c>.
    /// </summary>
    [HttpPut("{resourceId}/visibility")]
    public async Task<IActionResult> SetVisibility(Guid resourceId, [FromBody] SetVisibilityRequest body)
    {
        try
        {
            var visibility = VisibilityExtensions.ParseVisibility(body.Visibility);
            await _visibility.SetVisibilityAsync(GetAuthenticatedUserId(), resourceId, visibility);
            return NoContent();
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
    }
}
