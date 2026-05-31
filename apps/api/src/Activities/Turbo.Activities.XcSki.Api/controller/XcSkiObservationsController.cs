using System.Security.Claims;
using System.Text.Json;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.XcSki.domain.handler;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.controller;

/// <summary>
/// User-contributed post-visit observation endpoints for XC ski trails.
/// Writes a row to <c>activities.activity_observations</c> with a typed
/// <see cref="XcSkiObservation"/> jsonb payload; reads recent observations
/// for the trail. The orchestrator's <c>nearby_obs</c> driver picks these
/// up on the next analysis fetch — no explicit invalidation needed
/// because <c>fetchAnalysisCached</c> hits the network first.
/// </summary>
[ApiController]
[Route("api/activities/xc-ski/{id}/observations")]
[Authorize]
public class XcSkiObservationsController : ControllerBase
{
    private const string KindKey = "xc_ski";

    private readonly IXcSkiActivityReader _reader;
    private readonly IActivityObservationStore _store;
    private readonly ILogger<XcSkiObservationsController> _logger;

    public XcSkiObservationsController(
        IXcSkiActivityReader reader,
        IActivityObservationStore store,
        ILogger<XcSkiObservationsController> logger)
    {
        _reader = reader;
        _store = store;
        _logger = logger;
    }

    private Guid GetUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    [ProducesResponseType(typeof(XcSkiObservationResponse), StatusCodes.Status201Created)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<XcSkiObservationResponse>> Create(
        Guid id, [FromBody] XcSkiObservationRequest request, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"XC ski activity {id} not found"));

            var typed = new XcSkiObservation(
                trackCondition: request.TrackCondition,
                snowQuality: request.SnowQuality,
                freshGroomingVisible: request.FreshGroomingVisible,
                concerns: request.Concerns);
            var payload = JsonSerializer.SerializeToElement(typed);

            var observation = new ActivityObservation(
                Id: Guid.NewGuid(),
                ActivityId: id,
                UserId: userId,
                ObservedAt: request.ObservedAt?.ToUniversalTime() ?? DateTimeOffset.UtcNow,
                Kind: KindKey,
                Rating: request.Rating,
                Comment: request.Comment,
                KindPayload: payload,
                PhotoCount: request.PhotoCount ?? 0,
                CreatedAt: DateTime.UtcNow);
            await _store.WriteAsync(observation, ct);
            return CreatedAtAction(nameof(GetById),
                new { id, observationId = observation.Id },
                XcSkiObservationResponse.From(observation, typed));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid observation", ex.Message)); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error writing XC ski observation for {ActivityId}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Observation save failed", ex.Message));
        }
    }

    [HttpGet]
    [ProducesResponseType(typeof(XcSkiObservationListResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<XcSkiObservationListResponse>> List(
        Guid id,
        [FromQuery] DateTime? since,
        [FromQuery] int limit = 20,
        CancellationToken ct = default)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"XC ski activity {id} not found"));

            var sinceCutoff = since is not null
                ? new DateTimeOffset(DateTime.SpecifyKind(since.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow - TimeSpan.FromDays(90);
            var rows = await _store.GetForActivityAsync(id, sinceCutoff, Math.Clamp(limit, 1, 100), ct);
            var items = rows
                .Select(r => XcSkiObservationResponse.From(r, ParseTyped(r.KindPayload)))
                .ToList();
            return Ok(new XcSkiObservationListResponse(items));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpGet("{observationId}")]
    [ProducesResponseType(typeof(XcSkiObservationResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<XcSkiObservationResponse>> GetById(
        Guid id, Guid observationId, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"XC ski activity {id} not found"));
            var obs = await _store.GetByIdAsync(observationId, ct);
            if (obs is null || obs.ActivityId != id)
                return NotFound(new ErrorResponse("Not found", $"Observation {observationId} not found"));
            return Ok(XcSkiObservationResponse.From(obs, ParseTyped(obs.KindPayload)));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    private static XcSkiObservation ParseTyped(JsonElement payload)
    {
        try
        {
            return payload.Deserialize<XcSkiObservation>()
                ?? new XcSkiObservation(null, null, null, null);
        }
        catch (JsonException)
        {
            return new XcSkiObservation(null, null, null, null);
        }
    }
}

public sealed class XcSkiObservationRequest
{
    public DateTime? ObservedAt { get; set; }
    public short? Rating { get; set; }
    public string? Comment { get; set; }
    public short? PhotoCount { get; set; }

    // Kind-specific extras.
    public string? TrackCondition { get; set; }
    public string? SnowQuality { get; set; }
    public bool? FreshGroomingVisible { get; set; }
    public string? Concerns { get; set; }
}

public sealed record XcSkiObservationResponse(
    Guid Id,
    Guid ActivityId,
    Guid UserId,
    DateTimeOffset ObservedAt,
    short? Rating,
    string? Comment,
    short PhotoCount,
    XcSkiObservation Details)
{
    public static XcSkiObservationResponse From(ActivityObservation r, XcSkiObservation typed) =>
        new(r.Id, r.ActivityId, r.UserId, r.ObservedAt, r.Rating, r.Comment, r.PhotoCount, typed);
}

public sealed record XcSkiObservationListResponse(IReadOnlyList<XcSkiObservationResponse> Items);
