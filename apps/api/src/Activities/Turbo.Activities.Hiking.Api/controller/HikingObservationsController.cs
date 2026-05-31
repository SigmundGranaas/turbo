using System.Security.Claims;
using System.Text.Json;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Hiking.domain.handler;
using Turboapi.Activities.Hiking.value;

namespace Turboapi.Activities.Hiking.controller;

[ApiController]
[Route("api/activities/hiking/{id}/observations")]
[Authorize]
public class HikingObservationsController : ControllerBase
{
    private const string KindKey = "hiking";

    private readonly IHikingActivityReader _reader;
    private readonly IActivityObservationStore _store;
    private readonly ILogger<HikingObservationsController> _logger;

    public HikingObservationsController(
        IHikingActivityReader reader,
        IActivityObservationStore store,
        ILogger<HikingObservationsController> logger)
    { _reader = reader; _store = store; _logger = logger; }

    private Guid GetUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    public async Task<ActionResult<HikingObservationResponse>> Create(
        Guid id, [FromBody] HikingObservationRequest request, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Hiking activity {id} not found"));

            var typed = new HikingObservation(
                trailCondition: request.TrailCondition,
                snowAt: request.SnowAt,
                waterSourcesFlowing: request.WaterSourcesFlowing,
                markingState: request.MarkingState,
                concerns: request.Concerns);
            var observation = new ActivityObservation(
                Id: Guid.NewGuid(),
                ActivityId: id,
                UserId: userId,
                ObservedAt: request.ObservedAt?.ToUniversalTime() ?? DateTimeOffset.UtcNow,
                Kind: KindKey,
                Rating: request.Rating,
                Comment: request.Comment,
                KindPayload: JsonSerializer.SerializeToElement(typed),
                PhotoCount: request.PhotoCount ?? 0,
                CreatedAt: DateTime.UtcNow);
            await _store.WriteAsync(observation, ct);
            return CreatedAtAction(nameof(GetById),
                new { id, observationId = observation.Id },
                HikingObservationResponse.From(observation, typed));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error writing hiking observation for {ActivityId}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Observation save failed", ex.Message));
        }
    }

    [HttpGet]
    public async Task<ActionResult<HikingObservationListResponse>> List(
        Guid id, [FromQuery] DateTime? since, [FromQuery] int limit = 20, CancellationToken ct = default)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Hiking activity {id} not found"));
            var sinceCutoff = since is not null
                ? new DateTimeOffset(DateTime.SpecifyKind(since.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow - TimeSpan.FromDays(90);
            var rows = await _store.GetForActivityAsync(id, sinceCutoff, Math.Clamp(limit, 1, 100), ct);
            return Ok(new HikingObservationListResponse(
                rows.Select(r => HikingObservationResponse.From(r, ParseTyped(r.KindPayload))).ToList()));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpGet("{observationId}")]
    public async Task<ActionResult<HikingObservationResponse>> GetById(
        Guid id, Guid observationId, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Hiking activity {id} not found"));
            var obs = await _store.GetByIdAsync(observationId, ct);
            if (obs is null || obs.ActivityId != id)
                return NotFound(new ErrorResponse("Not found", $"Observation {observationId} not found"));
            return Ok(HikingObservationResponse.From(obs, ParseTyped(obs.KindPayload)));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    private static HikingObservation ParseTyped(JsonElement payload)
    {
        try
        {
            return payload.Deserialize<HikingObservation>()
                ?? new HikingObservation(null, null, null, null, null);
        }
        catch (JsonException)
        {
            return new HikingObservation(null, null, null, null, null);
        }
    }
}

public sealed class HikingObservationRequest
{
    public DateTime? ObservedAt { get; set; }
    public short? Rating { get; set; }
    public string? Comment { get; set; }
    public short? PhotoCount { get; set; }
    public string? TrailCondition { get; set; }
    public double? SnowAt { get; set; }
    public bool? WaterSourcesFlowing { get; set; }
    public string? MarkingState { get; set; }
    public string? Concerns { get; set; }
}

public sealed record HikingObservationResponse(
    Guid Id, Guid ActivityId, Guid UserId, DateTimeOffset ObservedAt,
    short? Rating, string? Comment, short PhotoCount, HikingObservation Details)
{
    public static HikingObservationResponse From(ActivityObservation r, HikingObservation typed) =>
        new(r.Id, r.ActivityId, r.UserId, r.ObservedAt, r.Rating, r.Comment, r.PhotoCount, typed);
}

public sealed record HikingObservationListResponse(IReadOnlyList<HikingObservationResponse> Items);
