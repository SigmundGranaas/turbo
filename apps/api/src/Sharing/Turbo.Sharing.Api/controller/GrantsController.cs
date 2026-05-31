using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Sharing.controller.request;
using Turboapi.Sharing.controller.response;
using Turboapi.Sharing.domain.exception;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.controller;

[ApiController]
[Route("api/sharing/grants")]
[Authorize]
public class GrantsController : ControllerBase
{
    private readonly IGrantService _grants;

    public GrantsController(IGrantService grants) => _grants = grants;

    private Guid GetAuthenticatedUserId() =>
        Guid.Parse(User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not found in token"));

    [HttpGet("resources/{resourceId}")]
    public async Task<ActionResult<IReadOnlyList<GrantDto>>> ListForResource(Guid resourceId)
    {
        try
        {
            return Ok(await _grants.ListForResourceAsync(GetAuthenticatedUserId(), resourceId));
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
    }

    [HttpPost("users")]
    public async Task<ActionResult<GrantDto>> GrantToUser([FromBody] GrantToUserRequest body)
    {
        try
        {
            var role = RoleExtensions.ParseRole(body.Role);
            return Ok(await _grants.GrantToUserAsync(
                GetAuthenticatedUserId(), body.ResourceId, body.UserId, role, body.ExpiresAt));
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
    }

    [HttpPost("groups")]
    public async Task<ActionResult<GrantDto>> GrantToGroup([FromBody] GrantToGroupRequest body)
    {
        try
        {
            var role = RoleExtensions.ParseRole(body.Role);
            return Ok(await _grants.GrantToGroupAsync(
                GetAuthenticatedUserId(), body.ResourceId, body.GroupId, role, body.ExpiresAt));
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
        catch (InvalidOperationException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
    }

    [HttpPost("links")]
    public async Task<ActionResult<LinkGrantDto>> GrantAsLink([FromBody] GrantAsLinkRequest body)
    {
        try
        {
            var role = RoleExtensions.ParseRole(body.Role);
            return Ok(await _grants.GrantAsLinkAsync(
                GetAuthenticatedUserId(), body.ResourceId, role, body.ExpiresAt));
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
    }

    [HttpDelete("resources/{resourceId}/users/{userId}")]
    public async Task<IActionResult> RevokeUser(Guid resourceId, Guid userId)
    {
        try
        {
            await _grants.RevokeUserAsync(GetAuthenticatedUserId(), resourceId, userId);
            return NoContent();
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
    }

    [HttpDelete("resources/{resourceId}/groups/{groupId}")]
    public async Task<IActionResult> RevokeGroup(Guid resourceId, Guid groupId)
    {
        try
        {
            await _grants.RevokeGroupAsync(GetAuthenticatedUserId(), resourceId, groupId);
            return NoContent();
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
    }

    [HttpDelete("resources/{resourceId}/links/{linkSubjectId}")]
    public async Task<IActionResult> RevokeLink(Guid resourceId, Guid linkSubjectId)
    {
        try
        {
            await _grants.RevokeLinkAsync(GetAuthenticatedUserId(), resourceId, linkSubjectId);
            return NoContent();
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
    }

    /// <summary>
    /// Redeems a link token in favour of the calling user. Materializes the
    /// link grant as a per-user grant so the resource flows through normal
    /// sync afterwards. Idempotent. Returns the resource id and type so the
    /// client can navigate.
    /// </summary>
    [HttpPost("links/{token}/redeem")]
    public async Task<ActionResult<LinkRedemptionDto>> RedeemLink(string token)
    {
        try
        {
            return Ok(await _grants.RedeemLinkAsync(GetAuthenticatedUserId(), token));
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (InvalidOperationException ex) { return NotFound(new ErrorResponse("Link invalid", ex.Message)); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
    }
}
