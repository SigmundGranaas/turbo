using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Tracks.controller.request;
using Turboapi.Tracks.controller.response;
using Turboapi.Tracks.domain.commands;
using Turboapi.Tracks.domain.exception;
using Turboapi.Tracks.domain.handler;
using Turboapi.Tracks.domain.queries;
using Turboapi.Tracks.domain.query;
using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.controller;

[ApiController]
[Route("api/tracks/[controller]")]
[Authorize]
public class TracksController : ControllerBase
{
    private const int MaxDeltaLimit = 500;

    private readonly CreateTrackHandler _createHandler;
    private readonly UpdateTrackHandler _updateHandler;
    private readonly DeleteTrackHandler _deleteHandler;
    private readonly GetTrackByIdHandler _byIdHandler;
    private readonly GetUserTracksHandler _userTracksHandler;
    private readonly GetTracksChangedSinceHandler _deltaHandler;
    private readonly ILogger<TracksController> _logger;

    public TracksController(
        CreateTrackHandler createHandler,
        UpdateTrackHandler updateHandler,
        DeleteTrackHandler deleteHandler,
        GetTrackByIdHandler byIdHandler,
        GetUserTracksHandler userTracksHandler,
        GetTracksChangedSinceHandler deltaHandler,
        ILogger<TracksController> logger)
    {
        _createHandler = createHandler;
        _updateHandler = updateHandler;
        _deleteHandler = deleteHandler;
        _byIdHandler = byIdHandler;
        _userTracksHandler = userTracksHandler;
        _deltaHandler = deltaHandler;
        _logger = logger;
    }

    private Guid GetAuthenticatedUserId()
    {
        var userId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not found in token");
        return Guid.Parse(userId);
    }

    private static long? ParseIfMatch(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw)) return null;
        var trimmed = raw.Trim().Trim('"');
        return long.TryParse(trimmed, out var v) ? v : null;
    }

    [HttpPost]
    [ProducesResponseType(typeof(TrackResponse), StatusCodes.Status201Created)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    public async Task<ActionResult<TrackResponse>> Create([FromBody] CreateTrackRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var command = new CreateTrackCommand(
                userId,
                request.Metadata.ToValueObject(),
                request.Geometry.ToValueObject(),
                request.Stats.ToValueObject());
            var trackId = await _createHandler.Handle(command);

            // The read-model projection is async (outbox → transport →
            // subscriber). Echo the request shape back; clients should follow
            // the Location header (or use the delta endpoint) for the
            // server-stamped sync fields.
            var response = new TrackResponse
            {
                Id = trackId,
                Geometry = request.Geometry,
                Metadata = new MetadataDto
                {
                    Name = request.Metadata.Name,
                    Description = request.Metadata.Description,
                    ColorHex = request.Metadata.ColorHex,
                    IconKey = request.Metadata.IconKey,
                    LineStyleKey = request.Metadata.LineStyleKey,
                    Smoothing = request.Metadata.Smoothing,
                },
                Stats = request.Stats,
                Version = 1,
            };
            return CreatedAtAction(nameof(GetById), new { id = trackId }, response);
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating track");
            return BadRequest(new ErrorResponse("Failed to create track", ex.Message));
        }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(typeof(TrackResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConflictResponse), StatusCodes.Status412PreconditionFailed)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    public async Task<ActionResult<TrackResponse>> Update(
        Guid id,
        [FromBody] UpdateTrackRequest request,
        [FromHeader(Name = "If-Match")] string? ifMatch)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var updates = new TrackUpdateParameters(
                request.Geometry?.ToValueObject(),
                request.Metadata?.ToValueObject(),
                request.Stats?.ToValueObject());
            var command = new UpdateTrackCommand(userId, id, updates, ParseIfMatch(ifMatch));
            var aggregate = await _updateHandler.Handle(command);

            var data = new TrackData(
                aggregate.Id, aggregate.OwnerId,
                aggregate.Metadata, aggregate.Geometry, aggregate.Stats,
                CreatedAt: default, UpdatedAt: default, DeletedAt: null, Version: 0);
            return Ok(TrackResponse.FromDto(data));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedException) { return Forbid(); }
        catch (TrackNotFoundException)
        {
            return NotFound(new ErrorResponse("Track not found", $"Track with ID {id} was not found"));
        }
        catch (OptimisticConcurrencyException occ)
        {
            var current = await _byIdHandler.Handle(new GetTrackByIdQuery(id, GetAuthenticatedUserId()));
            var body = current is null
                ? new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, default!)
                : new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, TrackResponse.FromDto(current));
            return StatusCode(StatusCodes.Status412PreconditionFailed, body);
        }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid update request", ex.Message));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error updating track {TrackId}", id);
            return BadRequest(new ErrorResponse("Failed to update track", ex.Message));
        }
    }

    [HttpDelete("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConflictResponse), StatusCodes.Status412PreconditionFailed)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    public async Task<IActionResult> Delete(
        Guid id,
        [FromHeader(Name = "If-Match")] string? ifMatch)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            await _deleteHandler.Handle(new DeleteTrackCommand(userId, id, ParseIfMatch(ifMatch)));
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedException) { return Forbid(); }
        catch (TrackNotFoundException)
        {
            return NotFound(new ErrorResponse("Track not found", $"Track with ID {id} was not found"));
        }
        catch (OptimisticConcurrencyException occ)
        {
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, default!));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error deleting track {TrackId}", id);
            return BadRequest(new ErrorResponse("Failed to delete track", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(TrackResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<TrackResponse>> GetById(Guid id)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var track = await _byIdHandler.Handle(new GetTrackByIdQuery(id, userId));
            if (track is null)
                return NotFound(new ErrorResponse("Track not found", $"Track with ID {id} was not found"));
            if (track.Version > 0)
                Response.Headers.ETag = $"\"{track.Version}\"";
            return Ok(TrackResponse.FromDto(track));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    /// <summary>
    /// Delta sync. If <paramref name="since"/> is omitted or null returns the
    /// entire current set for the caller (the initial-sync case); otherwise
    /// returns rows changed strictly after <paramref name="since"/>, plus
    /// tombstones (rows with deleted_at greater than the cutoff). Clients use
    /// the returned <c>serverTime</c> as the next <c>since</c>.
    /// </summary>
    [HttpGet]
    [ProducesResponseType(typeof(TracksDeltaResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<TracksDeltaResponse>> GetChanged(
        [FromQuery] DateTime? since,
        [FromQuery] int? limit)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var effectiveSince = since ?? DateTime.MinValue.ToUniversalTime();
            var effectiveLimit = limit is null ? MaxDeltaLimit : Math.Clamp(limit.Value, 1, MaxDeltaLimit);

            var result = await _deltaHandler.Handle(
                new GetTracksChangedSinceQuery(userId, effectiveSince, effectiveLimit));

            return Ok(new TracksDeltaResponse
            {
                Items = result.Items.Select(TrackResponse.FromDto).ToList(),
                Deleted = result.Deleted
                    .Select(t => new TombstoneResponse(t.Id, t.DeletedAt, t.Version))
                    .ToList(),
                NextCursor = null,
                ServerTime = result.ServerTime,
            });
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }
}
