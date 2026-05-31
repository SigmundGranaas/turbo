using System.Security.Claims;
using System.Text.Json;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Fishing.domain.handler;
using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.controller;

[ApiController]
[Route("api/activities/fishing/{id}/observations")]
[Authorize]
public class FishingObservationsController : ControllerBase
{
    private const string KindKey = "fishing";

    private readonly IFishingActivityReader _reader;
    private readonly IActivityObservationStore _store;
    private readonly ILogger<FishingObservationsController> _logger;

    public FishingObservationsController(
        IFishingActivityReader reader,
        IActivityObservationStore store,
        ILogger<FishingObservationsController> logger)
    { _reader = reader; _store = store; _logger = logger; }

    private Guid GetUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    public async Task<ActionResult<FishingObservationResponse>> Create(
        Guid id, [FromBody] FishingObservationRequest request, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Fishing activity {id} not found"));

            var typed = new FishingObservation(
                caught: request.Caught,
                species: request.Species,
                lengthCm: request.LengthCm,
                weightKg: request.WeightKg,
                lure: request.Lure,
                waterClarity: request.WaterClarity,
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
                FishingObservationResponse.From(observation, typed));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error writing fishing observation for {ActivityId}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Observation save failed", ex.Message));
        }
    }

    [HttpGet]
    public async Task<ActionResult<FishingObservationListResponse>> List(
        Guid id, [FromQuery] DateTime? since, [FromQuery] int limit = 20, CancellationToken ct = default)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Fishing activity {id} not found"));
            var sinceCutoff = since is not null
                ? new DateTimeOffset(DateTime.SpecifyKind(since.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow - TimeSpan.FromDays(90);
            var rows = await _store.GetForActivityAsync(id, sinceCutoff, Math.Clamp(limit, 1, 100), ct);
            return Ok(new FishingObservationListResponse(
                rows.Select(r => FishingObservationResponse.From(r, ParseTyped(r.KindPayload))).ToList()));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpGet("{observationId}")]
    public async Task<ActionResult<FishingObservationResponse>> GetById(
        Guid id, Guid observationId, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Fishing activity {id} not found"));
            var obs = await _store.GetByIdAsync(observationId, ct);
            if (obs is null || obs.ActivityId != id)
                return NotFound(new ErrorResponse("Not found", $"Observation {observationId} not found"));
            return Ok(FishingObservationResponse.From(obs, ParseTyped(obs.KindPayload)));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    private static FishingObservation ParseTyped(JsonElement payload)
    {
        try
        {
            return payload.Deserialize<FishingObservation>()
                ?? new FishingObservation(false, null, null, null, null, null, null);
        }
        catch (JsonException)
        {
            return new FishingObservation(false, null, null, null, null, null, null);
        }
    }
}

public sealed class FishingObservationRequest
{
    public DateTime? ObservedAt { get; set; }
    public short? Rating { get; set; }
    public string? Comment { get; set; }
    public short? PhotoCount { get; set; }
    public bool Caught { get; set; }
    public string? Species { get; set; }
    public double? LengthCm { get; set; }
    public double? WeightKg { get; set; }
    public string? Lure { get; set; }
    public string? WaterClarity { get; set; }
    public string? Concerns { get; set; }
}

public sealed record FishingObservationResponse(
    Guid Id, Guid ActivityId, Guid UserId, DateTimeOffset ObservedAt,
    short? Rating, string? Comment, short PhotoCount, FishingObservation Details)
{
    public static FishingObservationResponse From(ActivityObservation r, FishingObservation typed) =>
        new(r.Id, r.ActivityId, r.UserId, r.ObservedAt, r.Rating, r.Comment, r.PhotoCount, typed);
}

public sealed record FishingObservationListResponse(IReadOnlyList<FishingObservationResponse> Items);
