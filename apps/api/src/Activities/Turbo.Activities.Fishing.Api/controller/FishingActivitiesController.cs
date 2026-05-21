using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Fishing.controller.request;
using Turboapi.Activities.Fishing.data;
using Turboapi.Activities.Fishing.domain.handler;

namespace Turboapi.Activities.Fishing.controller;

[ApiController]
[Route("api/activities/fishing")]
[Authorize]
public class FishingActivitiesController : ControllerBase
{
    private readonly CreateFishingActivityHandler _create;
    private readonly UpdateFishingActivityHandler _update;
    private readonly DeleteFishingActivityHandler _delete;
    private readonly FishingContext _db;
    private readonly ILogger<FishingActivitiesController> _logger;

    public FishingActivitiesController(
        CreateFishingActivityHandler create,
        UpdateFishingActivityHandler update,
        DeleteFishingActivityHandler delete,
        FishingContext db,
        ILogger<FishingActivitiesController> logger)
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
    [ProducesResponseType(typeof(CreateFishingActivityResponse), StatusCodes.Status201Created)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    public async Task<ActionResult<CreateFishingActivityResponse>> Create(
        [FromBody] CreateFishingActivityRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var cmd = new CreateFishingActivityCommand(
                userId,
                request.Name,
                request.Description,
                request.Longitude,
                request.Latitude,
                request.Details.ToValueObject());
            var id = await _create.Handle(cmd);
            return CreatedAtAction(nameof(GetById), new { id }, new CreateFishingActivityResponse(id));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid create request", ex.Message));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating fishing activity");
            return BadRequest(new ErrorResponse("Failed to create fishing activity", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(FishingActivityResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<FishingActivityResponse>> GetById(Guid id, CancellationToken ct)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var row = await _db.Activities
                .Include(a => a.TargetSpecies)
                .Include(a => a.DepthSamples)
                .AsNoTracking()
                .FirstOrDefaultAsync(a => a.Id == id && a.OwnerId == userId && a.DeletedAt == null, ct);
            if (row is null)
                return NotFound(new ErrorResponse("Not found", $"Fishing activity {id} not found"));
            Response.Headers.ETag = $"\"{row.Version}\"";
            return Ok(FishingActivityResponse.From(row));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConcurrencyErrorResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> Update(Guid id, [FromBody] UpdateFishingActivityRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var cmd = new UpdateFishingActivityCommand(
                userId, id,
                request.Name, request.Description,
                request.Longitude, request.Latitude,
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
            return NotFound(new ErrorResponse("Not found", $"Fishing activity {id} not found"));
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
            await _delete.Handle(new DeleteFishingActivityCommand(userId, id)
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException)
        {
            return NotFound(new ErrorResponse("Not found", $"Fishing activity {id} not found"));
        }
        catch (OptimisticConcurrencyException ex)
        {
            Response.Headers.ETag = $"\"{ex.ActualVersion}\"";
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConcurrencyErrorResponse(ex.ExpectedVersion, ex.ActualVersion));
        }
    }
}

public sealed record ErrorResponse(string Title, string Detail);
