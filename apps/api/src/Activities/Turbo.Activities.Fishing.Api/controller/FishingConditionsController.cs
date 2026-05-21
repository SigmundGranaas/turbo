using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.Fishing.conditions;
using Turboapi.Activities.Fishing.domain.handler;
using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.controller;

/// <summary>
/// Conditions endpoint for a single fishing activity. Reads the typed
/// fishing aggregate, hands it to <see cref="IFishingConditionsAdvisor"/>,
/// and returns the composed typed report. The kind-specific path
/// (<c>/api/activities/fishing/{id}/conditions</c>) matches the typed
/// CRUD shape — there is no generic conditions endpoint.
/// </summary>
[ApiController]
[Route("api/activities/fishing")]
[Authorize]
public class FishingConditionsController : ControllerBase
{
    private readonly IFishingActivityReader _reader;
    private readonly IFishingConditionsAdvisor _advisor;
    private readonly ILogger<FishingConditionsController> _logger;

    public FishingConditionsController(
        IFishingActivityReader reader,
        IFishingConditionsAdvisor advisor,
        ILogger<FishingConditionsController> logger)
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
    [ProducesResponseType(typeof(FishingConditionsReport), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<FishingConditionsReport>> GetConditions(
        Guid id,
        [FromQuery] DateTime? at,
        CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Fishing activity {id} not found"));

            var instant = at.HasValue
                ? new DateTimeOffset(DateTime.SpecifyKind(at.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow;

            var report = await _advisor.AdviseAsync(activity, instant, ct);
            return Ok(report);
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing fishing conditions for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Conditions unavailable", ex.Message));
        }
    }
}
