using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Freediving.conditions;
using Turboapi.Activities.Freediving.domain.handler;
using Turboapi.Activities.Freediving.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Freediving.controller;

[ApiController]
[Route("api/activities/freediving")]
[Authorize]
public class FreedivingConditionsController : ControllerBase
{
    private readonly IFreedivingActivityReader _reader;
    private readonly IFreedivingConditionsAdvisor _advisor;
    private readonly FreedivingOrchestrator _orchestrator;
    private readonly ILogger<FreedivingConditionsController> _logger;

    public FreedivingConditionsController(
        IFreedivingActivityReader reader,
        IFreedivingConditionsAdvisor advisor,
        FreedivingOrchestrator orchestrator,
        ILogger<FreedivingConditionsController> logger)
    {
        _reader = reader;
        _advisor = advisor;
        _orchestrator = orchestrator;
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

    /// <summary>v2 orchestrator endpoint. Returns the structured
    /// <see cref="ActivityAnalysis"/> with named drivers (visibility
    /// estimate, surface chop, sea-temp proxy, tide phase), warnings
    /// (HAB, storm runoff), and the freediving kind-slice with viz
    /// forecast + tide info.</summary>
    [HttpGet("{id}/analysis")]
    [ProducesResponseType(typeof(ActivityAnalysis), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<ActivityAnalysis>> GetAnalysis(
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
            var qctx = QueryContext.ForAnalysis(instant, userId: userId);
            return Ok(await _orchestrator.RunAsync(activity, id, qctx, ct));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing freediving analysis for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Analysis unavailable", ex.Message));
        }
    }
}
