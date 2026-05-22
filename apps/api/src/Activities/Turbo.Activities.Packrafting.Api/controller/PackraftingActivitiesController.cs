using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using NetTopologySuite.IO;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Packrafting.data;
using Turboapi.Activities.Packrafting.data.model;
using Turboapi.Activities.Packrafting.domain.handler;
using Turboapi.Activities.Packrafting.value;

namespace Turboapi.Activities.Packrafting.controller;

[ApiController]
[Route("api/activities/packrafting")]
[Authorize]
public class PackraftingActivitiesController : ControllerBase
{
    private readonly CreatePackraftingActivityHandler _create;
    private readonly UpdatePackraftingActivityHandler _update;
    private readonly DeletePackraftingActivityHandler _delete;
    private readonly PackraftingContext _db;
    private readonly ILogger<PackraftingActivitiesController> _logger;

    public PackraftingActivitiesController(
        CreatePackraftingActivityHandler create, UpdatePackraftingActivityHandler update,
        DeletePackraftingActivityHandler delete, PackraftingContext db, ILogger<PackraftingActivitiesController> logger)
    { _create = create; _update = update; _delete = delete; _db = db; _logger = logger; }

    private Guid GetUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    [ProducesResponseType(typeof(CreatePackraftingResponse), StatusCodes.Status201Created)]
    public async Task<ActionResult<CreatePackraftingResponse>> Create([FromBody] CreatePackraftingRequest request)
    {
        try
        {
            var userId = GetUserId();
            var id = await _create.Handle(new CreatePackraftingActivityCommand(
                userId, request.Name, request.Description, request.RouteWkt, request.Details.ToValueObject()));
            return CreatedAtAction(nameof(GetById), new { id }, new CreatePackraftingResponse(id));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid create", ex.Message)); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating packrafting activity");
            return BadRequest(new ErrorResponse("Failed to create", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(PackraftingActivityResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<PackraftingActivityResponse>> GetById(Guid id, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var row = await _db.Activities.Include(a => a.Segments).AsNoTracking()
                .FirstOrDefaultAsync(a => a.Id == id && a.OwnerId == userId && a.DeletedAt == null, ct);
            if (row is null) return NotFound(new ErrorResponse("Not found", $"Packrafting activity {id} not found"));
            Response.Headers.ETag = $"\"{row.Version}\"";
            return Ok(PackraftingActivityResponse.From(row));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ConcurrencyErrorResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> Update(Guid id, [FromBody] UpdatePackraftingRequest request)
    {
        try
        {
            var userId = GetUserId();
            await _update.Handle(new UpdatePackraftingActivityCommand(
                userId, id, request.Name, request.Description, request.RouteWkt, request.Details?.ToValueObject())
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException) { return NotFound(new ErrorResponse("Not found", $"Packrafting activity {id} not found")); }
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
            await _delete.Handle(new DeletePackraftingActivityCommand(userId, id)
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException) { return NotFound(new ErrorResponse("Not found", $"Packrafting activity {id} not found")); }
        catch (OptimisticConcurrencyException ex)
        {
            Response.Headers.ETag = $"\"{ex.ActualVersion}\"";
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConcurrencyErrorResponse(ex.ExpectedVersion, ex.ActualVersion));
        }
    }
}

public sealed class CreatePackraftingRequest
{
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public string RouteWkt { get; set; } = string.Empty;
    public PackraftingDetailsDto Details { get; set; } = new();
}

public sealed class UpdatePackraftingRequest
{
    public string? Name { get; set; }
    public string? Description { get; set; }
    public string? RouteWkt { get; set; }
    public PackraftingDetailsDto? Details { get; set; }
}

public sealed class PackraftingDetailsDto
{
    public int DistanceMeters { get; set; }
    public int PaddleDistanceMeters { get; set; }
    public int PortageDistanceMeters { get; set; }
    public WaterGrade MaxGrade { get; set; }
    public WaterGrade TypicalGrade { get; set; }
    public double PutInLat { get; set; }
    public double PutInLon { get; set; }
    public double TakeOutLat { get; set; }
    public double TakeOutLon { get; set; }
    public string? NveStationCode { get; set; }
    public float? MinFlowCumecs { get; set; }
    public float? MaxFlowCumecs { get; set; }
    public List<SegmentDto> Segments { get; set; } = new();

    public PackraftingDetails ToValueObject() => new(
        DistanceMeters, PaddleDistanceMeters, PortageDistanceMeters,
        MaxGrade, TypicalGrade,
        PutInLat, PutInLon, TakeOutLat, TakeOutLon,
        NveStationCode, MinFlowCumecs, MaxFlowCumecs,
        Segments.Select(s => new RouteSegment(s.Kind, s.Grade, s.DistanceMeters, s.PolylineWkt, s.Notes)).ToList());
}

public sealed class SegmentDto
{
    public SegmentKind Kind { get; set; }
    public WaterGrade? Grade { get; set; }
    public int DistanceMeters { get; set; }
    public string PolylineWkt { get; set; } = string.Empty;
    public string? Notes { get; set; }
}

public sealed record CreatePackraftingResponse(Guid Id);

public sealed class PackraftingActivityResponse
{
    public Guid Id { get; set; }
    public Guid OwnerId { get; set; }
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public string RouteWkt { get; set; } = string.Empty;
    public PackraftingDetailsDto Details { get; set; } = new();
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public long Version { get; set; }

    public static PackraftingActivityResponse From(PackraftingActivityEntity e)
    {
        var writer = new WKTWriter();
        return new PackraftingActivityResponse
        {
            Id = e.Id, OwnerId = e.OwnerId, Name = e.Name, Description = e.Description,
            RouteWkt = writer.Write(e.Route),
            Details = new PackraftingDetailsDto
            {
                DistanceMeters = e.DistanceMeters,
                PaddleDistanceMeters = e.PaddleDistanceMeters,
                PortageDistanceMeters = e.PortageDistanceMeters,
                MaxGrade = (WaterGrade)e.MaxGrade,
                TypicalGrade = (WaterGrade)e.TypicalGrade,
                PutInLat = e.PutInLat, PutInLon = e.PutInLon,
                TakeOutLat = e.TakeOutLat, TakeOutLon = e.TakeOutLon,
                NveStationCode = e.NveStationCode,
                MinFlowCumecs = e.MinFlowCumecs,
                MaxFlowCumecs = e.MaxFlowCumecs,
                Segments = e.Segments.OrderBy(s => s.Ordinal)
                    .Select(s => new SegmentDto
                    {
                        Kind = (SegmentKind)s.Kind,
                        Grade = s.Grade is { } g ? (WaterGrade)g : null,
                        DistanceMeters = s.DistanceMeters,
                        PolylineWkt = writer.Write(s.Geometry),
                        Notes = s.Notes,
                    }).ToList(),
            },
            CreatedAt = e.CreatedAt, UpdatedAt = e.UpdatedAt, Version = e.Version,
        };
    }
}

public sealed record ErrorResponse(string Title, string Detail);
