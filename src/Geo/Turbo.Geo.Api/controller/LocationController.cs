using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Geo.controller.request;
using Turboapi.Geo.controller.response;
using Turboapi.Geo.domain.commands;
using Turboapi.Geo.domain.exception;
using Turboapi.Geo.domain.handler;
using Turboapi.Geo.domain.queries;
using Turboapi.Geo.domain.query;
using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.controller;

[ApiController]
[Route("api/geo/[controller]")]
[Authorize]
public class LocationsController : ControllerBase
{
    private const int MaxDeltaLimit = 500;

    private readonly CreateLocationHandler _createHandler;
    private readonly UpdateLocationHandler _updateHandler;
    private readonly DeleteLocationHandler _deleteHandler;
    private readonly GetLocationByIdHandler _locationQueryHandler;
    private readonly GetLocationsInExtentHandler _locationsQueryHandler;
    private readonly GetLocationsChangedSinceHandler _deltaHandler;
    private readonly ILogger<LocationsController> _logger;

    public LocationsController(
        CreateLocationHandler createHandler,
        UpdateLocationHandler updateHandler,
        DeleteLocationHandler deleteHandler,
        GetLocationByIdHandler idQuery,
        GetLocationsInExtentHandler locationsQuery,
        GetLocationsChangedSinceHandler deltaHandler,
        ILogger<LocationsController> logger)
    {
        _createHandler = createHandler;
        _locationQueryHandler = idQuery;
        _locationsQueryHandler = locationsQuery;
        _updateHandler = updateHandler;
        _deleteHandler = deleteHandler;
        _deltaHandler = deltaHandler;
        _logger = logger;
    }

    private Guid GetAuthenticatedUserId()
    {
        var userId = User.FindFirst(ClaimTypes.NameIdentifier)?.Value;
        if (userId == null)
        {
            throw new UnauthorizedAccessException("User ID not found in token");
        }
        return Guid.Parse(userId);
    }

    private static long? ParseIfMatch(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw)) return null;
        var trimmed = raw.Trim().Trim('"');
        return long.TryParse(trimmed, out var v) ? v : null;
    }

    [HttpPost]
    [ProducesResponseType(typeof(LocationResponse), StatusCodes.Status201Created)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    public async Task<ActionResult<LocationResponse>> Create([FromBody] CreateLocationRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();

            var displayInformation = new DisplayInformation(request.Display.Name, request.Display.Description ?? "",
                request.Display.Icon ?? "");

            var command = new CreateLocationCommand(
                userId,
                new Coordinates(request.Geometry.Longitude, request.Geometry.Latitude),
                displayInformation
            );

            var locationId = await _createHandler.Handle(command);

            var response = new LocationResponse
            {
                Id = locationId,
                Geometry = request.Geometry,
                Display = new DisplayData
                {
                    Name = request.Display.Name,
                    Description = request.Display.Description ?? "",
                    Icon = request.Display.Icon ?? ""
                },
                Version = 1,
            };
            return CreatedAtAction(nameof(GetById), new { id = locationId }, response);
        }
        catch (UnauthorizedAccessException)
        {
            return Forbid();
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating location");
            return BadRequest(new ErrorResponse("Failed to create location", ex.Message));
        }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(typeof(LocationResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConflictResponse), StatusCodes.Status412PreconditionFailed)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    public async Task<ActionResult<LocationResponse>> Update(
        Guid id,
        [FromBody] UpdateLocationRequest request,
        [FromHeader(Name = "If-Match")] string? ifMatch)
    {
        try
        {
            var userId = GetAuthenticatedUserId();

            Coordinates? newCoordinates = null;
            if (request.Geometry?.Longitude != null && request.Geometry?.Latitude != null)
            {
                newCoordinates = new Coordinates(request.Geometry.Longitude, request.Geometry.Latitude);
            }

            DisplayUpdate? domainDisplayChanges = null;
            if (request.Display != null)
            {
                domainDisplayChanges = new DisplayUpdate(
                    request.Display.Name,
                    request.Display.Description,
                    request.Display.Icon
                );
            }

            var locationUpdateParams = new LocationUpdateParameters(newCoordinates, domainDisplayChanges);
            var command = new UpdateLocationCommand(userId, id, locationUpdateParams, ParseIfMatch(ifMatch));

            var location = await _updateHandler.Handle(command);

            var response = new LocationResponse
            {
                Id = location.Id,
                Geometry = new GeometryData
                {
                    Longitude = location.Coordinates.Longitude,
                    Latitude = location.Coordinates.Latitude
                },
                Display = new DisplayData
                {
                    Name = location.Display.Name,
                    Description = location.Display.Description ?? "",
                    Icon = location.Display.Icon ?? ""
                }
            };
            return Ok(response);
        }
        catch (UnauthorizedAccessException)
        {
            return Forbid();
        }
        catch (LocationNotFoundException)
        {
            return NotFound(new ErrorResponse("Location not found", $"Location with ID {id} was not found"));
        }
        catch (OptimisticConcurrencyException occ)
        {
            var current = await _locationQueryHandler.Handle(new GetLocationByIdQuery(id, GetAuthenticatedUserId()));
            var body = current is null
                ? new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, null)
                : new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, LocationResponse.FromDto(current));
            return StatusCode(StatusCodes.Status412PreconditionFailed, body);
        }
        catch (ArgumentException ex)
        {
            _logger.LogWarning(ex, "Invalid update request for location {LocationId}: {ErrorMessage}", id, ex.Message);
            return BadRequest(new ErrorResponse("Invalid update request", ex.Message));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error updating location {LocationId}", id);
            return BadRequest(new ErrorResponse("Failed to update location", ex.Message));
        }
    }

    [HttpDelete("{id}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConflictResponse), StatusCodes.Status412PreconditionFailed)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    public async Task<IActionResult> Delete(
        Guid id,
        [FromHeader(Name = "If-Match")] string? ifMatch)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var command = new DeleteLocationCommand(userId, id, ParseIfMatch(ifMatch));

            await _deleteHandler.Handle(command);
            return NoContent();
        }
        catch (UnauthorizedAccessException)
        {
            return Forbid();
        }
        catch (LocationNotFoundException)
        {
            return NotFound(new ErrorResponse("Location not found", $"Location with ID {id} was not found"));
        }
        catch (OptimisticConcurrencyException occ)
        {
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, null));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error deleting location");
            return BadRequest(new ErrorResponse("Failed to delete location", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(LocationResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    public async Task<ActionResult<LocationResponse>> GetById(Guid id)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var location = await _locationQueryHandler.Handle(new GetLocationByIdQuery(id, userId));

            if (location == null)
                return NotFound(new ErrorResponse("Location not found", $"Location with ID {id} was not found"));

            if (location.version is { } v && v > 0)
                Response.Headers.ETag = $"\"{v}\"";

            return Ok(LocationResponse.FromDto(location));
        }
        catch (UnauthorizedAccessException)
        {
            return Forbid();
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error retrieving location");
            return BadRequest(new ErrorResponse("Failed to retrieve location", ex.Message));
        }
    }

    /// <summary>
    /// Returns either the user's full set of locations (when no
    /// <paramref name="minLon"/> / <paramref name="minLat"/> /
    /// <paramref name="maxLon"/> / <paramref name="maxLat"/> are provided
    /// and a <paramref name="since"/> cursor is supplied) as a delta, or
    /// runs the existing bounding-box query when extent parameters are
    /// present. The delta endpoint is additive and is used by the client
    /// for sync-on-login + sync-on-connectivity.
    /// </summary>
    [HttpGet]
    [ProducesResponseType(typeof(LocationsResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(LocationsDeltaResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    public async Task<IActionResult> List(
        [FromQuery] double? minLon,
        [FromQuery] double? minLat,
        [FromQuery] double? maxLon,
        [FromQuery] double? maxLat,
        [FromQuery] DateTime? since,
        [FromQuery] int? limit)
    {
        try
        {
            var userId = GetAuthenticatedUserId();

            if (minLon is not null && minLat is not null && maxLon is not null && maxLat is not null)
            {
                var locations = await _locationsQueryHandler.Handle(new GetLocationsInExtentQuery(
                    userId,
                    minLon.Value, minLat.Value, maxLon.Value, maxLat.Value
                ));
                return Ok(new LocationsResponse
                {
                    Items = locations.Select(LocationResponse.FromDto).ToList(),
                    Count = locations.Count()
                });
            }

            // Delta-sync path. since=null becomes "everything I have ever owned".
            var effectiveSince = since ?? DateTime.MinValue.ToUniversalTime();
            var effectiveLimit = limit is null ? MaxDeltaLimit : Math.Clamp(limit.Value, 1, MaxDeltaLimit);

            var delta = await _deltaHandler.Handle(
                new GetLocationsChangedSinceQuery(userId, effectiveSince, effectiveLimit));

            return Ok(new LocationsDeltaResponse
            {
                Items = delta.Items.Select(LocationResponse.FromDto).ToList(),
                Deleted = delta.Deleted
                    .Select(t => new TombstoneResponse(t.Id, t.DeletedAt, t.Version))
                    .ToList(),
                NextCursor = null,
                ServerTime = delta.ServerTime,
            });
        }
        catch (UnauthorizedAccessException)
        {
            return Forbid();
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error retrieving locations");
            return BadRequest(new ErrorResponse("Failed to retrieve locations", ex.Message));
        }
    }
}
