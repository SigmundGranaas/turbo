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
[Route("api/sharing/invites")]
[Authorize]
public class InvitesController : ControllerBase
{
    private readonly IShareInviteService _invites;

    public InvitesController(IShareInviteService invites) => _invites = invites;

    private Guid GetAuthenticatedUserId() =>
        Guid.Parse(User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not found in token"));

    [HttpGet]
    public async Task<ActionResult<IReadOnlyList<InviteDto>>> ListMine()
        => Ok(await _invites.ListMineAsync(GetAuthenticatedUserId()));

    [HttpPost("friend")]
    public async Task<ActionResult<InviteDto>> CreateFriendInvite([FromBody] CreateFriendInviteRequest body)
    {
        try
        {
            var lifetime = body.LifetimeDays is null ? (TimeSpan?)null : TimeSpan.FromDays(body.LifetimeDays.Value);
            return Ok(await _invites.CreateFriendInviteAsync(GetAuthenticatedUserId(), body.Email, lifetime));
        }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
    }

    [HttpPost("resource")]
    public async Task<ActionResult<InviteDto>> CreateResourceInvite([FromBody] CreateResourceInviteRequest body)
    {
        try
        {
            var role = RoleExtensions.ParseRole(body.Role);
            var lifetime = body.LifetimeDays is null ? (TimeSpan?)null : TimeSpan.FromDays(body.LifetimeDays.Value);
            return Ok(await _invites.CreateResourceInviteAsync(
                GetAuthenticatedUserId(), body.Email, body.ResourceId, role, lifetime));
        }
        catch (ResourceNotFoundException) { return NotFound(); }
        catch (AccessDeniedException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
    }

    /// <summary>
    /// Server-side trigger for new sign-ups: redeem every pending invite
    /// addressed to a given email in favour of the calling user. Idempotent.
    /// </summary>
    [HttpPost("redeem")]
    public async Task<ActionResult<int>> Redeem([FromBody] RedeemInvitesRequest body)
    {
        var count = await _invites.RedeemAllForUserAsync(GetAuthenticatedUserId(), body.Email);
        return Ok(new { Redeemed = count });
    }
}
