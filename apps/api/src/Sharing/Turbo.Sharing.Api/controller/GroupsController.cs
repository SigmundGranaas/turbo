using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Sharing.controller.request;
using Turboapi.Sharing.controller.response;
using Turboapi.Sharing.domain.service;

namespace Turboapi.Sharing.controller;

[ApiController]
[Route("api/sharing/groups")]
[Authorize]
public class GroupsController : ControllerBase
{
    private readonly IGroupService _groups;

    public GroupsController(IGroupService groups) => _groups = groups;

    private Guid GetAuthenticatedUserId() =>
        Guid.Parse(User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not found in token"));

    [HttpPost]
    public async Task<ActionResult<GroupDto>> Create([FromBody] CreateGroupRequest body)
    {
        try
        {
            var dto = await _groups.CreateAsync(GetAuthenticatedUserId(), body.Name);
            return CreatedAtAction(nameof(GetById), new { id = dto.Id }, dto);
        }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid request", ex.Message));
        }
    }

    [HttpGet]
    public async Task<ActionResult<IReadOnlyList<GroupDto>>> ListMine()
        => Ok(await _groups.ListMineAsync(GetAuthenticatedUserId()));

    [HttpGet("{id}")]
    public async Task<ActionResult<GroupDto>> GetById(Guid id)
    {
        var dto = await _groups.GetAsync(GetAuthenticatedUserId(), id);
        return dto is null ? NotFound() : Ok(dto);
    }

    [HttpDelete("{id}")]
    public async Task<IActionResult> Delete(Guid id)
    {
        try
        {
            await _groups.DeleteAsync(GetAuthenticatedUserId(), id);
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpPut("{id}/name")]
    public async Task<IActionResult> Rename(Guid id, [FromBody] RenameGroupRequest body)
    {
        try
        {
            await _groups.RenameAsync(GetAuthenticatedUserId(), id, body.Name);
            return NoContent();
        }
        catch (InvalidOperationException ex) { return BadRequest(new ErrorResponse("Cannot rename", ex.Message)); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid request", ex.Message)); }
    }

    [HttpPost("{id}/members")]
    public async Task<IActionResult> AddMember(Guid id, [FromBody] GroupMemberRequest body)
    {
        try
        {
            await _groups.AddMemberAsync(GetAuthenticatedUserId(), id, body.UserId);
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (InvalidOperationException ex) { return NotFound(new ErrorResponse("Group not found", ex.Message)); }
    }

    [HttpDelete("{id}/members/{userId}")]
    public async Task<IActionResult> RemoveMember(Guid id, Guid userId)
    {
        try
        {
            await _groups.RemoveMemberAsync(GetAuthenticatedUserId(), id, userId);
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (InvalidOperationException ex) { return NotFound(new ErrorResponse("Group not found", ex.Message)); }
    }
}
