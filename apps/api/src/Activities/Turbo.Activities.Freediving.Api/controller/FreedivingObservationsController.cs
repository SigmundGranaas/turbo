using System.Security.Claims;
using System.Text.Json;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Freediving.domain.handler;
using Turboapi.Activities.Freediving.value;

namespace Turboapi.Activities.Freediving.controller;

/// <summary>
/// Freediving observation endpoints. The <c>visibilityMeters</c> field
/// is the headline payload — calibrates the orchestrator's computed
/// visibility estimate over time. Recent observations on a spot also
/// feed the <c>nearby_obs</c> driver directly.
/// </summary>
[ApiController]
[Route("api/activities/freediving/{id}/observations")]
[Authorize]
public class FreedivingObservationsController : ControllerBase
{
    private const string KindKey = "freediving";

    private readonly IFreedivingActivityReader _reader;
    private readonly IActivityObservationStore _store;
    private readonly ILogger<FreedivingObservationsController> _logger;

    public FreedivingObservationsController(
        IFreedivingActivityReader reader,
        IActivityObservationStore store,
        ILogger<FreedivingObservationsController> logger)
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
    [ProducesResponseType(typeof(FreedivingObservationResponse), StatusCodes.Status201Created)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<FreedivingObservationResponse>> Create(
        Guid id, [FromBody] FreedivingObservationRequest request, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Freediving activity {id} not found"));

            var typed = new FreedivingObservation(
                visibilityMeters: request.VisibilityMeters,
                waterTempC: request.WaterTempC,
                currentStrength: request.CurrentStrength,
                speciesSeen: request.SpeciesSeen,
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
                FreedivingObservationResponse.From(observation, typed));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid observation", ex.Message)); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error writing freediving observation for {ActivityId}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Observation save failed", ex.Message));
        }
    }

    [HttpGet]
    [ProducesResponseType(typeof(FreedivingObservationListResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<FreedivingObservationListResponse>> List(
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
                return NotFound(new ErrorResponse("Not found", $"Freediving activity {id} not found"));

            var sinceCutoff = since is not null
                ? new DateTimeOffset(DateTime.SpecifyKind(since.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow - TimeSpan.FromDays(90);
            var rows = await _store.GetForActivityAsync(id, sinceCutoff, Math.Clamp(limit, 1, 100), ct);
            return Ok(new FreedivingObservationListResponse(
                rows.Select(r => FreedivingObservationResponse.From(r, ParseTyped(r.KindPayload))).ToList()));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpGet("{observationId}")]
    [ProducesResponseType(typeof(FreedivingObservationResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<FreedivingObservationResponse>> GetById(
        Guid id, Guid observationId, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Freediving activity {id} not found"));
            var obs = await _store.GetByIdAsync(observationId, ct);
            if (obs is null || obs.ActivityId != id)
                return NotFound(new ErrorResponse("Not found", $"Observation {observationId} not found"));
            return Ok(FreedivingObservationResponse.From(obs, ParseTyped(obs.KindPayload)));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    private static FreedivingObservation ParseTyped(JsonElement payload)
    {
        try
        {
            return payload.Deserialize<FreedivingObservation>()
                ?? new FreedivingObservation(null, null, null, null, null);
        }
        catch (JsonException)
        {
            return new FreedivingObservation(null, null, null, null, null);
        }
    }
}

public sealed class FreedivingObservationRequest
{
    public DateTime? ObservedAt { get; set; }
    public short? Rating { get; set; }
    public string? Comment { get; set; }
    public short? PhotoCount { get; set; }
    public double? VisibilityMeters { get; set; }
    public double? WaterTempC { get; set; }
    public string? CurrentStrength { get; set; }
    public List<string>? SpeciesSeen { get; set; }
    public string? Concerns { get; set; }
}

public sealed record FreedivingObservationResponse(
    Guid Id,
    Guid ActivityId,
    Guid UserId,
    DateTimeOffset ObservedAt,
    short? Rating,
    string? Comment,
    short PhotoCount,
    FreedivingObservation Details)
{
    public static FreedivingObservationResponse From(ActivityObservation r, FreedivingObservation typed) =>
        new(r.Id, r.ActivityId, r.UserId, r.ObservedAt, r.Rating, r.Comment, r.PhotoCount, typed);
}

public sealed record FreedivingObservationListResponse(IReadOnlyList<FreedivingObservationResponse> Items);
