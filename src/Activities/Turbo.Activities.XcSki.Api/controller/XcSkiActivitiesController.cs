using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;
using NetTopologySuite.IO;
using Turboapi.Activities.domain.exception;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.XcSki.data;
using Turboapi.Activities.XcSki.data.model;
using Turboapi.Activities.XcSki.domain.handler;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.controller;

[ApiController]
[Route("api/activities/xc-ski")]
[Authorize]
public class XcSkiActivitiesController : ControllerBase
{
    private readonly CreateXcSkiActivityHandler _create;
    private readonly UpdateXcSkiActivityHandler _update;
    private readonly DeleteXcSkiActivityHandler _delete;
    private readonly XcSkiContext _db;
    private readonly ILogger<XcSkiActivitiesController> _logger;

    public XcSkiActivitiesController(
        CreateXcSkiActivityHandler create, UpdateXcSkiActivityHandler update,
        DeleteXcSkiActivityHandler delete, XcSkiContext db, ILogger<XcSkiActivitiesController> logger)
    { _create = create; _update = update; _delete = delete; _db = db; _logger = logger; }

    private Guid GetUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    [ProducesResponseType(typeof(CreateXcSkiResponse), StatusCodes.Status201Created)]
    public async Task<ActionResult<CreateXcSkiResponse>> Create([FromBody] CreateXcSkiRequest request)
    {
        try
        {
            var userId = GetUserId();
            var id = await _create.Handle(new CreateXcSkiActivityCommand(
                userId, request.Name, request.Description, request.RouteWkt, request.Details.ToValueObject()));
            return CreatedAtAction(nameof(GetById), new { id }, new CreateXcSkiResponse(id));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid create", ex.Message)); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating XC ski activity");
            return BadRequest(new ErrorResponse("Failed to create", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(XcSkiActivityResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<XcSkiActivityResponse>> GetById(Guid id, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var row = await _db.Activities.AsNoTracking()
                .FirstOrDefaultAsync(a => a.Id == id && a.OwnerId == userId && a.DeletedAt == null, ct);
            if (row is null) return NotFound(new ErrorResponse("Not found", $"XC ski activity {id} not found"));
            Response.Headers.ETag = $"\"{row.Version}\"";
            return Ok(XcSkiActivityResponse.From(row));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ConcurrencyErrorResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> Update(Guid id, [FromBody] UpdateXcSkiRequest request)
    {
        try
        {
            var userId = GetUserId();
            await _update.Handle(new UpdateXcSkiActivityCommand(
                userId, id, request.Name, request.Description, request.RouteWkt, request.Details?.ToValueObject())
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException) { return NotFound(new ErrorResponse("Not found", $"XC ski activity {id} not found")); }
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
            await _delete.Handle(new DeleteXcSkiActivityCommand(userId, id)
            {
                IfMatchVersion = IfMatchHeader.Parse(Request.Headers.IfMatch.ToString()),
            });
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedActivityException) { return Forbid(); }
        catch (ActivityNotFoundException) { return NotFound(new ErrorResponse("Not found", $"XC ski activity {id} not found")); }
        catch (OptimisticConcurrencyException ex)
        {
            Response.Headers.ETag = $"\"{ex.ActualVersion}\"";
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConcurrencyErrorResponse(ex.ExpectedVersion, ex.ActualVersion));
        }
    }
}

public sealed class CreateXcSkiRequest
{
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public string RouteWkt { get; set; } = string.Empty;
    public XcSkiDetailsDto Details { get; set; } = new();
}

public sealed class UpdateXcSkiRequest
{
    public string? Name { get; set; }
    public string? Description { get; set; }
    public string? RouteWkt { get; set; }
    public XcSkiDetailsDto? Details { get; set; }
}

public sealed class XcSkiDetailsDto
{
    public int DistanceMeters { get; set; }
    public int AscentMeters { get; set; }
    public int DescentMeters { get; set; }
    public XcSkiTechnique Technique { get; set; }
    public GroomingStatus GroomingStatus { get; set; }
    public bool IsLit { get; set; }
    public bool RequiresSeasonPass { get; set; }
    public string? GroomingFeedKey { get; set; }

    public XcSkiDetails ToValueObject() => new(
        DistanceMeters, AscentMeters, DescentMeters,
        Technique, GroomingStatus, IsLit, RequiresSeasonPass, GroomingFeedKey);
}

public sealed record CreateXcSkiResponse(Guid Id);

public sealed class XcSkiActivityResponse
{
    public Guid Id { get; set; }
    public Guid OwnerId { get; set; }
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }
    public string RouteWkt { get; set; } = string.Empty;
    public XcSkiDetailsDto Details { get; set; } = new();
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public long Version { get; set; }

    public static XcSkiActivityResponse From(XcSkiActivityEntity e) => new()
    {
        Id = e.Id, OwnerId = e.OwnerId, Name = e.Name, Description = e.Description,
        RouteWkt = new WKTWriter().Write(e.Route),
        Details = new XcSkiDetailsDto
        {
            DistanceMeters = e.DistanceMeters,
            AscentMeters = e.AscentMeters,
            DescentMeters = e.DescentMeters,
            Technique = (XcSkiTechnique)e.Technique,
            GroomingStatus = (GroomingStatus)e.GroomingStatus,
            IsLit = e.IsLit,
            RequiresSeasonPass = e.RequiresSeasonPass,
            GroomingFeedKey = e.GroomingFeedKey,
        },
        CreatedAt = e.CreatedAt, UpdatedAt = e.UpdatedAt, Version = e.Version,
    };
}

public sealed record ErrorResponse(string Title, string Detail);
