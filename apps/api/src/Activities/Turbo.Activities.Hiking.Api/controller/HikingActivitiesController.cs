using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Hiking.controller.request;
using Turboapi.Activities.Hiking.data;
using Turboapi.Activities.Hiking.domain.handler;

namespace Turboapi.Activities.Hiking.controller;

[ApiController]
[Route("api/activities/hiking")]
[Authorize]
public class HikingActivitiesController : ControllerBase
{
    private readonly CreateHikingActivityHandler _create;
    private readonly UpdateHikingActivityHandler _update;
    private readonly DeleteHikingActivityHandler _delete;
    private readonly HikingContext _db;
    private readonly ILogger<HikingActivitiesController> _logger;

    public HikingActivitiesController(
        CreateHikingActivityHandler create, UpdateHikingActivityHandler update,
        DeleteHikingActivityHandler delete, HikingContext db, ILogger<HikingActivitiesController> logger)
    {
        _create = create; _update = update; _delete = delete; _db = db; _logger = logger;
    }

    private Guid GetAuthenticatedUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    [ProducesResponseType(typeof(CreateHikingActivityResponse), StatusCodes.Status201Created)]
    public async Task<ActionResult<CreateHikingActivityResponse>> Create([FromBody] CreateHikingActivityRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var cmd = new CreateHikingActivityCommand(
                userId, request.Name, request.Description, request.RouteWkt, request.Details.ToValueObject());
            var id = await _create.Handle(cmd);
            return CreatedAtAction(nameof(GetById), new { id }, new CreateHikingActivityResponse(id));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid create request", ex.Message)); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating hiking activity");
            return BadRequest(new ErrorResponse("Failed to create", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(HikingActivityResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<HikingActivityResponse>> GetById(Guid id, CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var row = await _db.Activities.Include(a => a.WaterSources).AsNoTracking()
                .FirstOrDefaultAsync(a => a.Id == id && a.OwnerId == userId && a.DeletedAt == null, ct);
            if (row is null)
                return NotFound(new ErrorResponse("Not found", $"Hiking activity {id} not found"));
            Response.Headers.ETag = $"\"{row.Version}\"";
            return Ok(HikingActivityResponse.From(row));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ConcurrencyErrorResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> Update(Guid id, [FromBody] UpdateHikingActivityRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            await _update.Handle(new UpdateHikingActivityCommand(
                userId, id, request.Name, request.Description, request.RouteWkt, request.Details?.ToValueObject())
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException) { return NotFound(new ErrorResponse("Not found", $"Hiking activity {id} not found")); }
        catch (OptimisticConcurrencyException ex)
        {
            Response.Headers.ETag = $"\"{ex.ActualVersion}\"";
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConcurrencyErrorResponse(ex.ExpectedVersion, ex.ActualVersion));
        }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid update", ex.Message)); }
    }

    [HttpDelete("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ConcurrencyErrorResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> Delete(Guid id)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            await _delete.Handle(new DeleteHikingActivityCommand(userId, id)
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException) { return NotFound(new ErrorResponse("Not found", $"Hiking activity {id} not found")); }
        catch (OptimisticConcurrencyException ex)
        {
            Response.Headers.ETag = $"\"{ex.ActualVersion}\"";
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConcurrencyErrorResponse(ex.ExpectedVersion, ex.ActualVersion));
        }
    }
}
