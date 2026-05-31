using System.Security.Claims;
using System.Text.Json;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.BackcountrySki.domain.handler;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.BackcountrySki.controller;

[ApiController]
[Route("api/activities/backcountry-ski/{id}/observations")]
[Authorize]
public class BackcountrySkiObservationsController : ControllerBase
{
    private const string KindKey = "backcountry_ski";

    private readonly IBackcountrySkiActivityReader _reader;
    private readonly IActivityObservationStore _store;
    private readonly ILogger<BackcountrySkiObservationsController> _logger;

    public BackcountrySkiObservationsController(
        IBackcountrySkiActivityReader reader,
        IActivityObservationStore store,
        ILogger<BackcountrySkiObservationsController> logger)
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
    [ProducesResponseType(typeof(BackcountrySkiObservationResponse), StatusCodes.Status201Created)]
    public async Task<ActionResult<BackcountrySkiObservationResponse>> Create(
        Guid id, [FromBody] BackcountrySkiObservationRequest request, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Backcountry ski activity {id} not found"));

            var typed = new BackcountrySkiObservation(
                snowConditionSummary: request.SnowConditionSummary,
                breakableCrust: request.BreakableCrust,
                observedDangerLevel: request.ObservedDangerLevel,
                signsOfInstability: request.SignsOfInstability,
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
                BackcountrySkiObservationResponse.From(observation, typed));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex) { return BadRequest(new ErrorResponse("Invalid observation", ex.Message)); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error writing backcountry ski observation for {ActivityId}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Observation save failed", ex.Message));
        }
    }

    [HttpGet]
    [ProducesResponseType(typeof(BackcountrySkiObservationListResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<BackcountrySkiObservationListResponse>> List(
        Guid id, [FromQuery] DateTime? since, [FromQuery] int limit = 20,
        CancellationToken ct = default)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Backcountry ski activity {id} not found"));
            var sinceCutoff = since is not null
                ? new DateTimeOffset(DateTime.SpecifyKind(since.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow - TimeSpan.FromDays(90);
            var rows = await _store.GetForActivityAsync(id, sinceCutoff, Math.Clamp(limit, 1, 100), ct);
            return Ok(new BackcountrySkiObservationListResponse(
                rows.Select(r => BackcountrySkiObservationResponse.From(r, ParseTyped(r.KindPayload))).ToList()));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpGet("{observationId}")]
    public async Task<ActionResult<BackcountrySkiObservationResponse>> GetById(
        Guid id, Guid observationId, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Backcountry ski activity {id} not found"));
            var obs = await _store.GetByIdAsync(observationId, ct);
            if (obs is null || obs.ActivityId != id)
                return NotFound(new ErrorResponse("Not found", $"Observation {observationId} not found"));
            return Ok(BackcountrySkiObservationResponse.From(obs, ParseTyped(obs.KindPayload)));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    private static BackcountrySkiObservation ParseTyped(JsonElement payload)
    {
        try
        {
            return payload.Deserialize<BackcountrySkiObservation>()
                ?? new BackcountrySkiObservation(null, null, null, null, null);
        }
        catch (JsonException)
        {
            return new BackcountrySkiObservation(null, null, null, null, null);
        }
    }
}

public sealed class BackcountrySkiObservationRequest
{
    public DateTime? ObservedAt { get; set; }
    public short? Rating { get; set; }
    public string? Comment { get; set; }
    public short? PhotoCount { get; set; }
    public string? SnowConditionSummary { get; set; }
    public bool? BreakableCrust { get; set; }
    public short? ObservedDangerLevel { get; set; }
    public List<string>? SignsOfInstability { get; set; }
    public string? Concerns { get; set; }
}

public sealed record BackcountrySkiObservationResponse(
    Guid Id, Guid ActivityId, Guid UserId, DateTimeOffset ObservedAt,
    short? Rating, string? Comment, short PhotoCount,
    BackcountrySkiObservation Details)
{
    public static BackcountrySkiObservationResponse From(ActivityObservation r, BackcountrySkiObservation typed) =>
        new(r.Id, r.ActivityId, r.UserId, r.ObservedAt, r.Rating, r.Comment, r.PhotoCount, typed);
}

public sealed record BackcountrySkiObservationListResponse(
    IReadOnlyList<BackcountrySkiObservationResponse> Items);
