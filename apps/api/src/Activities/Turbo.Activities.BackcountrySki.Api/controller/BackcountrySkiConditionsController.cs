using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.BackcountrySki.conditions;
using Turboapi.Activities.BackcountrySki.domain.handler;
using Turboapi.Activities.BackcountrySki.value;

namespace Turboapi.Activities.BackcountrySki.controller;

[ApiController]
[Route("api/activities/backcountry-ski")]
[Authorize]
public class BackcountrySkiConditionsController : ControllerBase
{
    private readonly IBackcountrySkiActivityReader _reader;
    private readonly IBackcountrySkiConditionsAdvisor _advisor;
    private readonly ILogger<BackcountrySkiConditionsController> _logger;

    public BackcountrySkiConditionsController(
        IBackcountrySkiActivityReader reader,
        IBackcountrySkiConditionsAdvisor advisor,
        ILogger<BackcountrySkiConditionsController> logger)
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
}
