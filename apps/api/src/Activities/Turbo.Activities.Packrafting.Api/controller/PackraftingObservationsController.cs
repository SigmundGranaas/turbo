using System.Security.Claims;
using System.Text.Json;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.Packrafting.domain.handler;
using Turboapi.Activities.Packrafting.value;

namespace Turboapi.Activities.Packrafting.controller;

[ApiController]
[Route("api/activities/packrafting/{id}/observations")]
[Authorize]
public class PackraftingObservationsController : ControllerBase
{
    private const string KindKey = "packrafting";

    private readonly IPackraftingActivityReader _reader;
    private readonly IActivityObservationStore _store;
    private readonly ILogger<PackraftingObservationsController> _logger;

    public PackraftingObservationsController(
        IPackraftingActivityReader reader,
        IActivityObservationStore store,
        ILogger<PackraftingObservationsController> logger)
    { _reader = reader; _store = store; _logger = logger; }

    private Guid GetUserId()
    {
        var raw = User.FindFirst(ClaimTypes.NameIdentifier)?.Value
            ?? throw new UnauthorizedAccessException("User ID not in token");
        return Guid.Parse(raw);
    }

    [HttpPost]
    public async Task<ActionResult<PackraftingObservationResponse>> Create(
        Guid id, [FromBody] PackraftingObservationRequest request, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Packrafting activity {id} not found"));

            var typed = new PackraftingObservation(
                observedGrade: request.ObservedGrade,
                waterTempC: request.WaterTempC,
                flowCumecs: request.FlowCumecs,
                portagesTaken: request.PortagesTaken,
                hazardsNoted: request.HazardsNoted,
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
                PackraftingObservationResponse.From(observation, typed));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error writing packrafting observation for {ActivityId}", id);
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("Observation save failed", ex.Message));
        }
    }

    [HttpGet]
    public async Task<ActionResult<PackraftingObservationListResponse>> List(
        Guid id, [FromQuery] DateTime? since, [FromQuery] int limit = 20, CancellationToken ct = default)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Packrafting activity {id} not found"));
            var sinceCutoff = since is not null
                ? new DateTimeOffset(DateTime.SpecifyKind(since.Value, DateTimeKind.Utc))
                : DateTimeOffset.UtcNow - TimeSpan.FromDays(90);
            var rows = await _store.GetForActivityAsync(id, sinceCutoff, Math.Clamp(limit, 1, 100), ct);
            return Ok(new PackraftingObservationListResponse(
                rows.Select(r => PackraftingObservationResponse.From(r, ParseTyped(r.KindPayload))).ToList()));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpGet("{observationId}")]
    public async Task<ActionResult<PackraftingObservationResponse>> GetById(
        Guid id, Guid observationId, CancellationToken ct)
    {
        try
        {
            var userId = GetUserId();
            var activity = await _reader.GetByIdAsync(id, ct);
            if (activity is null || activity.Core.OwnerId != userId)
                return NotFound(new ErrorResponse("Not found", $"Packrafting activity {id} not found"));
            var obs = await _store.GetByIdAsync(observationId, ct);
            if (obs is null || obs.ActivityId != id)
                return NotFound(new ErrorResponse("Not found", $"Observation {observationId} not found"));
            return Ok(PackraftingObservationResponse.From(obs, ParseTyped(obs.KindPayload)));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    private static PackraftingObservation ParseTyped(JsonElement payload)
    {
        try
        {
            return payload.Deserialize<PackraftingObservation>()
                ?? new PackraftingObservation(null, null, null, null, null, null);
        }
        catch (JsonException)
        {
            return new PackraftingObservation(null, null, null, null, null, null);
        }
    }
}

public sealed class PackraftingObservationRequest
{
    public DateTime? ObservedAt { get; set; }
    public short? Rating { get; set; }
    public string? Comment { get; set; }
    public short? PhotoCount { get; set; }
    public string? ObservedGrade { get; set; }
    public double? WaterTempC { get; set; }
    public double? FlowCumecs { get; set; }
    public int? PortagesTaken { get; set; }
    public List<string>? HazardsNoted { get; set; }
    public string? Concerns { get; set; }
}

public sealed record PackraftingObservationResponse(
    Guid Id, Guid ActivityId, Guid UserId, DateTimeOffset ObservedAt,
    short? Rating, string? Comment, short PhotoCount, PackraftingObservation Details)
{
    public static PackraftingObservationResponse From(ActivityObservation r, PackraftingObservation typed) =>
        new(r.Id, r.ActivityId, r.UserId, r.ObservedAt, r.Rating, r.Comment, r.PhotoCount, typed);
}

public sealed record PackraftingObservationListResponse(IReadOnlyList<PackraftingObservationResponse> Items);
