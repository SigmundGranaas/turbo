using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.Hiking.conditions;
using Turboapi.Activities.Hiking.domain.handler;
using Turboapi.Activities.Hiking.value;

namespace Turboapi.Activities.Hiking.controller;

[ApiController]
[Route("api/activities/hiking")]
[Authorize]
public class HikingConditionsController : ControllerBase
{
    private readonly IHikingActivityReader _reader;
    private readonly IHikingConditionsAdvisor _advisor;
    private readonly ILogger<HikingConditionsController> _logger;

    public HikingConditionsController(
        IHikingActivityReader reader,
        IHikingConditionsAdvisor advisor,
        ILogger<HikingConditionsController> logger)
    {
        _reader = reader;
        _advisor = advisor;
        _logger = logger;
    }

    private Guid GetAuthenticatedUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpGet("{id}/conditions")]
    [ProducesResponseType(typeof(HikingConditionsReport), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<HikingConditionsReport>> GetConditions(
        Guid id, [FromQuery] DateTime? at, CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Hiking activity {id} not found"));
            var instant = at.HasValue
                ? new DateTimeOffset(DateTime.SpecifyKind(at.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow;
            return Ok(await _advisor.AdviseAsync(activity, instant, ct));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing hiking conditions for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Conditions unavailable", ex.Message));
        }
    }
}
