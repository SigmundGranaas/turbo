using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Packrafting.conditions;
using Turboapi.Activities.Packrafting.domain.handler;
using Turboapi.Activities.Packrafting.value;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Packrafting.controller;

[ApiController]
[Route("api/activities/packrafting")]
[Authorize]
public class PackraftingConditionsController : ControllerBase
{
    private readonly IPackraftingActivityReader _reader;
    private readonly IPackraftingConditionsAdvisor _advisor;
    private readonly PackraftingOrchestrator _orchestrator;
    private readonly ILogger<PackraftingConditionsController> _logger;

    public PackraftingConditionsController(
        IPackraftingActivityReader reader,
        IPackraftingConditionsAdvisor advisor,
        PackraftingOrchestrator orchestrator,
        ILogger<PackraftingConditionsController> logger)
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
    [ProducesResponseType(typeof(PackraftingConditionsReport), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<PackraftingConditionsReport>> GetConditions(
        Guid id, [FromQuery] DateTime? at, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Packrafting activity {id} not found"));
            var instant = at.HasValue
                ? new DateTimeOffset(DateTime.SpecifyKind(at.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow;
            return Ok(await _advisor.AdviseAsync(activity, instant, ct));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing packrafting conditions for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Conditions unavailable", ex.Message));
        }
    }

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
                return NotFound(new ErrorResponse("Not found", $"Packrafting activity {id} not found"));
            var instant = at.HasValue
                ? new DateTimeOffset(DateTime.SpecifyKind(at.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow;
            var qctx = QueryContext.ForAnalysis(instant, userId: userId);
            return Ok(await _orchestrator.RunAsync(activity, id, qctx, ct));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing packrafting analysis for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Analysis unavailable", ex.Message));
        }
    }
}
