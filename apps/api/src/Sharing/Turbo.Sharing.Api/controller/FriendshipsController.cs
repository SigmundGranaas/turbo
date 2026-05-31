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
[Route("api/sharing/friendships")]
[Authorize]
public class FriendshipsController : ControllerBase
{
    private readonly IFriendshipService _friendships;
    private readonly ILogger<FriendshipsController> _logger;

    public FriendshipsController(IFriendshipService friendships, ILogger<FriendshipsController> logger)
    {
        _friendships = friendships;
        _logger = logger;
    }

    private Guid GetAuthenticatedUserId() =>
        Guid.Parse(User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not found in token"));

    [HttpGet]
    public async Task<ActionResult<IReadOnlyList<FriendshipDto>>> List([FromQuery] string? status)
    {
        var userId = GetAuthenticatedUserId();
        FriendshipStatus? parsed = string.IsNullOrWhiteSpace(status)
            ? null
            : FriendshipStatusExtensions.ParseFriendshipStatus(status);
        return Ok(await _friendships.ListAsync(userId, parsed));
    }

    [HttpPost("request")]
    public async Task<ActionResult<FriendshipDto>> RequestFriendship([FromBody] FriendshipActionRequest body)
    {
        try
        {
            return Ok(await _friendships.RequestAsync(GetAuthenticatedUserId(), body.OtherUserId));
        }
        catch (FriendshipAlreadyExistsException ex)
        {
            return Conflict(new ErrorResponse("Already exists", ex.Message));
        }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid request", ex.Message));
        }
    }

    [HttpPost("accept")]
    public async Task<ActionResult<FriendshipDto>> Accept([FromBody] FriendshipActionRequest body)
    {
        try
        {
            return Ok(await _friendships.AcceptAsync(GetAuthenticatedUserId(), body.OtherUserId));
        }
        catch (InvalidOperationException ex)
        {
            return BadRequest(new ErrorResponse("Cannot accept", ex.Message));
        }
    }

    [HttpPost("block")]
    public async Task<IActionResult> Block([FromBody] FriendshipActionRequest body)
    {
        await _friendships.BlockAsync(GetAuthenticatedUserId(), body.OtherUserId);
        return NoContent();
    }

    [HttpDelete("{otherUserId}")]
    public async Task<IActionResult> Remove(Guid otherUserId)
    {
        await _friendships.RemoveAsync(GetAuthenticatedUserId(), otherUserId);
        return NoContent();
    }
}
