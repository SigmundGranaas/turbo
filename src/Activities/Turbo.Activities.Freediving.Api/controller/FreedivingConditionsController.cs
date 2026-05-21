using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.Freediving.conditions;
using Turboapi.Activities.Freediving.domain.handler;
using Turboapi.Activities.Freediving.value;

namespace Turboapi.Activities.Freediving.controller;

[ApiController]
[Route("api/activities/freediving")]
[Authorize]
public class FreedivingConditionsController : ControllerBase
{
    private readonly IFreedivingActivityReader _reader;
    private readonly IFreedivingConditionsAdvisor _advisor;
    private readonly ILogger<FreedivingConditionsController> _logger;

    public FreedivingConditionsController(
        IFreedivingActivityReader reader,
        IFreedivingConditionsAdvisor advisor,
        ILogger<FreedivingConditionsController> logger)
    {
        _reader = reader;
        _advisor = advisor;
        _logger = logger;
    }

    private Guid GetUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpGet("{id}/conditions")]
    [ProducesResponseType(typeof(FreedivingConditionsReport), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<FreedivingConditionsReport>> GetConditions(
        Guid id, [FromQuery] DateTime? at, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Freediving activity {id} not found"));
            var instant = at.HasValue
                ? new DateTimeOffset(DateTime.SpecifyKind(at.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow;
            return Ok(await _advisor.AdviseAsync(activity, instant, ct));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing freediving conditions for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Conditions unavailable", ex.Message));
        }
    }
}
