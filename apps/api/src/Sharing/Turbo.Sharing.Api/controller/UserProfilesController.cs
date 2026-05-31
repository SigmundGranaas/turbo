using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Sharing.domain.service;

namespace Turboapi.Sharing.controller;

[ApiController]
[Route("api/sharing")]
[Authorize]
public class UserProfilesController : ControllerBase
{
    private readonly IUserProfileService _profiles;

    public UserProfilesController(IUserProfileService profiles) => _profiles = profiles;

    private Guid GetAuthenticatedUserId() =>
        Guid.Parse(User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not found in token"));

    /// <summary>
    /// Returns the calling user's friend code, generating one lazily on
    /// first read. Used by the client to show "Your friend code is X"
    /// in Settings → Sharing → Friends.
    /// </summary>
    [HttpGet("me/profile")]
    public async Task<ActionResult<UserProfileDto>> GetMyProfile()
        => Ok(await _profiles.EnsureProfileAsync(GetAuthenticatedUserId()));

    /// <summary>
    /// Looks up a user by their friend code. Returns 404 if no profile
    /// matches the code. The "turbo-" prefix is accepted but optional.
    /// </summary>
    [HttpGet("users/lookup")]
    public async Task<ActionResult<UserLookupResponse>> LookupByCode([FromQuery] string code)
    {
        var userId = await _profiles.LookupByCodeAsync(code);
        if (userId is null) return NotFound();
        return Ok(new UserLookupResponse(userId.Value));
    }
}

public sealed record UserLookupResponse(Guid UserId);
