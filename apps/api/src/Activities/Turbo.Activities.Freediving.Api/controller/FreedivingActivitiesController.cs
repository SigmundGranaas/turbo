using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Freediving.data;
using Turboapi.Activities.Freediving.data.model;
using Turboapi.Activities.Freediving.domain.handler;
using Turboapi.Activities.Freediving.value;

namespace Turboapi.Activities.Freediving.controller;

[ApiController]
[Route("api/activities/freediving")]
[Authorize]
public class FreedivingActivitiesController : ControllerBase
{
    private readonly CreateFreedivingActivityHandler _create;
    private readonly UpdateFreedivingActivityHandler _update;
    private readonly DeleteFreedivingActivityHandler _delete;
    private readonly FreedivingContext _db;
    private readonly ILogger<FreedivingActivitiesController> _logger;

    public FreedivingActivitiesController(
        CreateFreedivingActivityHandler create, UpdateFreedivingActivityHandler update,
        DeleteFreedivingActivityHandler delete, FreedivingContext db, ILogger<FreedivingActivitiesController> logger)
    { _create = create; _update = update; _delete = delete; _db = db; _logger = logger; }

    private Guid GetUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    [ProducesResponseType(typeof(CreateFreedivingResponse), StatusCodes.Status201Created)]
    public async Task<ActionResult<CreateFreedivingResponse>> Create([FromBody] CreateFreedivingRequest request)
    {
        try
        {
            var userId = GetUserId();
            var id = await _create.Handle(new CreateFreedivingActivityCommand(
                userId, request.Name, request.Description, request.Longitude, request.Latitude, request.Details.ToValueObject()));
            return CreatedAtAction(nameof(GetById), new { id }, new CreateFreedivingResponse(id));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid create", ex.Message)); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating freediving activity");
            return BadRequest(new ErrorResponse("Failed to create", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(FreedivingActivityResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<FreedivingActivityResponse>> GetById(Guid id, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var row = await _db.Activities.Include(a => a.TargetSpecies).AsNoTracking()
                .FirstOrDefaultAsync(a => a.Id == id && a.OwnerId == userId && a.DeletedAt == null, ct);
            if (row is null) return NotFound(new ErrorResponse("Not found", $"Freediving activity {id} not found"));
            Response.Headers.ETag = $"\"{row.Version}\"";
            return Ok(FreedivingActivityResponse.From(row));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ConcurrencyErrorResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> Update(Guid id, [FromBody] UpdateFreedivingRequest request)
    {
        try
        {
            var userId = GetUserId();
            await _update.Handle(new UpdateFreedivingActivityCommand(
                userId, id, request.Name, request.Description, request.Longitude, request.Latitude, request.Details?.ToValueObject())
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException) { return NotFound(new ErrorResponse("Not found", $"Freediving activity {id} not found")); }
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
            var userId = GetUserId();
            await _delete.Handle(new DeleteFreedivingActivityCommand(userId, id)
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException) { return NotFound(new ErrorResponse("Not found", $"Freediving activity {id} not found")); }
        catch (OptimisticConcurrencyException ex)
        {
            Response.Headers.ETag = $"\"{ex.ActualVersion}\"";
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConcurrencyErrorResponse(ex.ExpectedVersion, ex.ActualVersion));
        }
    }
}

public sealed class CreateFreedivingRequest
{
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public double Longitude { get; set; }
    public double Latitude { get; set; }
    public FreedivingDetailsDto Details { get; set; } = new();
}

public sealed class UpdateFreedivingRequest
{
    public string? Name { get; set; }
    public string? Description { get; set; }
    public double? Longitude { get; set; }
    public double? Latitude { get; set; }
    public FreedivingDetailsDto? Details { get; set; }
}

public sealed class FreedivingDetailsDto
{
    public WaterBody WaterBody { get; set; }
    public BottomType BottomType { get; set; }
    public float MaxDepthMeters { get; set; }
    public float? TypicalVisibilityMeters { get; set; }
    public bool HarpoonAllowed { get; set; }
    public bool ShoreEntry { get; set; }
    public string? AccessNotes { get; set; }
    public List<TargetSpeciesDto> TargetSpecies { get; set; } = new();

    public FreedivingDetails ToValueObject() => new(
        WaterBody, BottomType, MaxDepthMeters, TypicalVisibilityMeters,
        HarpoonAllowed, ShoreEntry, AccessNotes,
        TargetSpecies.Select(t => new TargetSpecies(t.SpeciesCode, t.Notes)).ToList());
}

public sealed class TargetSpeciesDto
{
    public string SpeciesCode { get; set; } = string.Empty;
    public string? Notes { get; set; }
}

public sealed record CreateFreedivingResponse(Guid Id);

public sealed class FreedivingActivityResponse
{
    public Guid Id { get; set; }
    public Guid OwnerId { get; set; }
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public double Longitude { get; set; }
    public double Latitude { get; set; }
    public FreedivingDetailsDto Details { get; set; } = new();
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public long Version { get; set; }

    public static FreedivingActivityResponse From(FreedivingActivityEntity e) => new()
    {
        Id = e.Id, OwnerId = e.OwnerId, Name = e.Name, Description = e.Description,
        Longitude = e.Geometry.X, Latitude = e.Geometry.Y,
        Details = new FreedivingDetailsDto
        {
            WaterBody = (WaterBody)e.WaterBody,
            BottomType = (BottomType)e.BottomType,
            MaxDepthMeters = e.MaxDepthMeters,
            TypicalVisibilityMeters = e.TypicalVisibilityMeters,
            HarpoonAllowed = e.HarpoonAllowed,
            ShoreEntry = e.ShoreEntry,
            AccessNotes = e.AccessNotes,
            TargetSpecies = e.TargetSpecies
                .Select(t => new TargetSpeciesDto { SpeciesCode = t.SpeciesCode, Notes = t.Notes })
                .ToList(),
        },
        CreatedAt = e.CreatedAt, UpdatedAt = e.UpdatedAt, Version = e.Version,
    };
}

public sealed record ErrorResponse(string Title, string Detail);
