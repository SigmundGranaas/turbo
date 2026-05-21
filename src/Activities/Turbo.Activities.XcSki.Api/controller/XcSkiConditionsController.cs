using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.XcSki.conditions;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.controller;

[ApiController]
[Route("api/activities/xc-ski")]
[Authorize]
public class XcSkiConditionsController : ControllerBase
{
    private readonly Turboapi.Activities.XcSki.domain.handler.IXcSkiActivityReader _reader;
    private readonly IXcSkiConditionsAdvisor _advisor;
    private readonly ILogger<XcSkiConditionsController> _logger;

    public XcSkiConditionsController(
        Turboapi.Activities.XcSki.domain.handler.IXcSkiActivityReader reader,
        IXcSkiConditionsAdvisor advisor,
        ILogger<XcSkiConditionsController> logger)
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
    [ProducesResponseType(typeof(XcSkiConditionsReport), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<XcSkiConditionsReport>> GetConditions(
        Guid id, [FromQuery] DateTime? at, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"XC ski activity {id} not found"));
            var instant = at.HasValue
                ? new DateTimeOffset(DateTime.SpecifyKind(at.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow;
            return Ok(await _advisor.AdviseAsync(activity, instant, ct));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error computing xc ski conditions for {Id}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Conditions unavailable", ex.Message));
        }
    }
}
