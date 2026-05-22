using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.BackcountrySki.controller.request;
using Turboapi.Activities.BackcountrySki.data;
using Turboapi.Activities.BackcountrySki.domain.handler;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.BackcountrySki.controller;

[ApiController]
[Route("api/activities/backcountry-ski")]
[Authorize]
public class BackcountrySkiActivitiesController : ControllerBase
{
    private readonly CreateBackcountrySkiActivityHandler _create;
    private readonly UpdateBackcountrySkiActivityHandler _update;
    private readonly DeleteBackcountrySkiActivityHandler _delete;
    private readonly BackcountrySkiContext _db;
    private readonly ILogger<BackcountrySkiActivitiesController> _logger;

    public BackcountrySkiActivitiesController(
        CreateBackcountrySkiActivityHandler create,
        UpdateBackcountrySkiActivityHandler update,
        DeleteBackcountrySkiActivityHandler delete,
        BackcountrySkiContext db,
        ILogger<BackcountrySkiActivitiesController> logger)
    {
        _create = create;
        _update = update;
        _delete = delete;
        _db = db;
        _logger = logger;
    }

    private Guid GetAuthenticatedUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    [ProducesResponseType(typeof(CreateBackcountrySkiActivityResponse), StatusCodes.Status201Created)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    public async Task<ActionResult<CreateBackcountrySkiActivityResponse>> Create(
        [FromBody] CreateBackcountrySkiActivityRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var cmd = new CreateBackcountrySkiActivityCommand(
                userId, request.Name, request.Description, request.RouteWkt,
                request.Details.ToValueObject());
            var id = await _create.Handle(cmd);
            return CreatedAtAction(nameof(GetById), new { id }, new CreateBackcountrySkiActivityResponse(id));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid create request", ex.Message));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating backcountry ski activity");
            return BadRequest(new ErrorResponse("Failed to create", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(BackcountrySkiActivityResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<BackcountrySkiActivityResponse>> GetById(Guid id, CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var row = await _db.Activities
                .Include(a => a.AspectMix)
                .Include(a => a.Legs)
                .AsNoTracking()
                .FirstOrDefaultAsync(a => a.Id == id && a.OwnerId == userId && a.DeletedAt == null, ct);
            if (row is null)
                return NotFound(new ErrorResponse("Not found", $"Backcountry ski activity {id} not found"));
            Response.Headers.ETag = $"\"{row.Version}\"";
            return Ok(BackcountrySkiActivityResponse.From(row));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConcurrencyErrorResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> Update(Guid id, [FromBody] UpdateBackcountrySkiActivityRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var cmd = new UpdateBackcountrySkiActivityCommand(
                userId, id,
                request.Name, request.Description,
                request.RouteWkt,
                request.Details?.ToValueObject())
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            };
            await _update.Handle(cmd);
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException)
        {
            return NotFound(new ErrorResponse("Not found", $"Backcountry ski activity {id} not found"));
        }
        catch (OptimisticConcurrencyException ex)
        {
            Response.Headers.ETag = $"\"{ex.ActualVersion}\"";
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConcurrencyErrorResponse(ex.ExpectedVersion, ex.ActualVersion));
        }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid update request", ex.Message));
        }
    }

    [HttpDelete("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConcurrencyErrorResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> Delete(Guid id)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            await _delete.Handle(new DeleteBackcountrySkiActivityCommand(userId, id)
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException)
        {
            return NotFound(new ErrorResponse("Not found", $"Backcountry ski activity {id} not found"));
        }
        catch (OptimisticConcurrencyException ex)
        {
            Response.Headers.ETag = $"\"{ex.ActualVersion}\"";
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConcurrencyErrorResponse(ex.ExpectedVersion, ex.ActualVersion));
        }
    }
}
