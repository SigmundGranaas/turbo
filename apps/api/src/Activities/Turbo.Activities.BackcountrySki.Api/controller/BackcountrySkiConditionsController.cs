using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.BackcountrySki.conditions;
using Turboapi.Activities.BackcountrySki.domain.handler;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities.BackcountrySki.controller;

[ApiController]
[Route("api/activities/backcountry-ski")]
[Authorize]
public class BackcountrySkiConditionsController : ControllerBase
{
    private readonly IBackcountrySkiActivityReader _reader;
    private readonly IBackcountrySkiConditionsAdvisor _advisor;
    private readonly BackcountrySkiOrchestrator _orchestrator;
    private readonly ILogger<BackcountrySkiConditionsController> _logger;

    public BackcountrySkiConditionsController(
        IBackcountrySkiActivityReader reader,
        IBackcountrySkiConditionsAdvisor advisor,
        BackcountrySkiOrchestrator orchestrator,
        ILogger<BackcountrySkiConditionsController> logger)
    {
        _reader = reader;
        _advisor = advisor;
        _orchestrator = orchestrator;
        _logger = logger;
    }

    private Guid GetAuthenticatedUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpGet("{id}/conditions")]
    [ProducesResponseType(typeof(BackcountrySkiConditionsReport), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<BackcountrySkiConditionsReport>> GetConditions(
        Guid id, [FromQuery] DateTime? at, CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Backcountry ski activity {id} not found"));

            var instant = at.HasValue
                ? new DateTimeOffset(DateTime.SpecifyKind(at.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow;
            var report = await _advisor.AdviseAsync(activity, instant, ct);
            return Ok(report);
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing backcountry ski conditions for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Conditions unavailable", ex.Message));
        }
    }

    /// <summary>
    /// v2 orchestrator endpoint. Returns the structured
    /// <see cref="ActivityAnalysis"/> with named drivers, warnings,
    /// suggested windows, and the per-aspect wind-loading slice in
    /// <c>kindSlices["backcountry_ski"]</c>. Coexists with
    /// <c>/conditions</c> during migration.
    /// </summary>
    [HttpGet("{id}/analysis")]
    [ProducesResponseType(typeof(ActivityAnalysis), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<ActivityAnalysis>> GetAnalysis(
        Guid id, [FromQuery] DateTime? at, CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Backcountry ski activity {id} not found"));

            var instant = at.HasValue
                ? new DateTimeOffset(DateTime.SpecifyKind(at.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow;
            var queryContext = QueryContext.ForAnalysis(instant, userId: userId);
            var analysis = await _orchestrator.RunAsync(activity, id, queryContext, ct);
            return Ok(analysis);
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing backcountry ski analysis for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Analysis unavailable", ex.Message));
        }
    }
}
