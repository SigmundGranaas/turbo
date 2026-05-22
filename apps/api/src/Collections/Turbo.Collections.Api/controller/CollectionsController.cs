using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Collections.controller.request;
using Turboapi.Collections.controller.response;
using Turboapi.Collections.domain.commands;
using Turboapi.Collections.domain.exception;
using Turboapi.Collections.domain.handler;
using Turboapi.Collections.domain.queries;
using Turboapi.Collections.domain.query;
using Turboapi.Collections.domain.value;

namespace Turboapi.Collections.controller;

[ApiController]
[Route("api/collections/[controller]")]
[Authorize]
public class CollectionsController : ControllerBase
{
    private const int MaxDeltaLimit = 500;

    private readonly CreateCollectionHandler _createHandler;
    private readonly UpdateCollectionHandler _updateHandler;
    private readonly DeleteCollectionHandler _deleteHandler;
    private readonly AddItemToCollectionHandler _addItemHandler;
    private readonly RemoveItemFromCollectionHandler _removeItemHandler;
    private readonly GetCollectionByIdHandler _byIdHandler;
    private readonly GetCollectionsChangedSinceHandler _deltaHandler;
    private readonly ILogger<CollectionsController> _logger;

    public CollectionsController(
        CreateCollectionHandler createHandler,
        UpdateCollectionHandler updateHandler,
        DeleteCollectionHandler deleteHandler,
        AddItemToCollectionHandler addItemHandler,
        RemoveItemFromCollectionHandler removeItemHandler,
        GetCollectionByIdHandler byIdHandler,
        GetCollectionsChangedSinceHandler deltaHandler,
        ILogger<CollectionsController> logger)
    {
        _createHandler = createHandler;
        _updateHandler = updateHandler;
        _deleteHandler = deleteHandler;
        _addItemHandler = addItemHandler;
        _removeItemHandler = removeItemHandler;
        _byIdHandler = byIdHandler;
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
    [ProducesResponseType(typeof(CollectionResponse), StatusCodes.Status201Created)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    public async Task<ActionResult<CollectionResponse>> Create([FromBody] CreateCollectionRequest request)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var command = new CreateCollectionCommand(userId, request.ToValueObject());
            var collectionId = await _createHandler.Handle(command);

            var response = new CollectionResponse
            {
                Id = collectionId,
                Name = request.Name,
                Description = request.Description,
                ColorHex = request.ColorHex,
                IconKey = request.IconKey,
                SortOrder = request.SortOrder,
                SavedFilter = request.SavedFilter,
                Items = Array.Empty<ItemRefDto>(),
                Version = 1,
            };
            return CreatedAtAction(nameof(GetById), new { id = collectionId }, response);
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid create request", ex.Message));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error creating collection");
            return BadRequest(new ErrorResponse("Failed to create collection", ex.Message));
        }
    }

    [HttpPut("{id}")]
    [ProducesResponseType(typeof(CollectionResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConflictResponse), StatusCodes.Status412PreconditionFailed)]
    [ProducesResponseType(StatusCodes.Status401Unauthorized)]
    public async Task<ActionResult<CollectionResponse>> Update(
        Guid id,
        [FromBody] UpdateCollectionRequest request,
        [FromHeader(Name = "If-Match")] string? ifMatch)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var command = new UpdateCollectionCommand(userId, id, request.ToValueObject(), ParseIfMatch(ifMatch));
            var aggregate = await _updateHandler.Handle(command);

            var data = new CollectionData(
                aggregate.Id, aggregate.OwnerId, aggregate.Metadata,
                aggregate.Items.ToList(),
                CreatedAt: default, UpdatedAt: default, DeletedAt: null, Version: 0);
            return Ok(CollectionResponse.FromDto(data));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedException) { return Forbid(); }
        catch (CollectionNotFoundException)
        {
            return NotFound(new ErrorResponse("Collection not found", $"Collection with ID {id} was not found"));
        }
        catch (OptimisticConcurrencyException occ)
        {
            var current = await _byIdHandler.Handle(new GetCollectionByIdQuery(id, GetAuthenticatedUserId()));
            var body = current is null
                ? new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, null)
                : new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, CollectionResponse.FromDto(current));
            return StatusCode(StatusCodes.Status412PreconditionFailed, body);
        }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid update request", ex.Message));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error updating collection {CollectionId}", id);
            return BadRequest(new ErrorResponse("Failed to update collection", ex.Message));
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
            await _deleteHandler.Handle(new DeleteCollectionCommand(userId, id, ParseIfMatch(ifMatch)));
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedException) { return Forbid(); }
        catch (CollectionNotFoundException)
        {
            return NotFound(new ErrorResponse("Collection not found", $"Collection with ID {id} was not found"));
        }
        catch (OptimisticConcurrencyException occ)
        {
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, null));
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error deleting collection {CollectionId}", id);
            return BadRequest(new ErrorResponse("Failed to delete collection", ex.Message));
        }
    }

    [HttpGet("{id}")]
    [ProducesResponseType(typeof(CollectionResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    public async Task<ActionResult<CollectionResponse>> GetById(Guid id)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var collection = await _byIdHandler.Handle(new GetCollectionByIdQuery(id, userId));
            if (collection is null)
                return NotFound(new ErrorResponse("Collection not found", $"Collection with ID {id} was not found"));
            if (collection.Version > 0)
                Response.Headers.ETag = $"\"{collection.Version}\"";
            return Ok(CollectionResponse.FromDto(collection));
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    /// <summary>
    /// Delta sync. Without <c>since</c> returns the entire current set;
    /// otherwise returns rows changed strictly after <c>since</c> plus
    /// tombstones. Clients use the returned <c>serverTime</c> as the next
    /// <c>since</c>.
    /// </summary>
    [HttpGet]
    [ProducesResponseType(typeof(CollectionsDeltaResponse), StatusCodes.Status200OK)]
    public async Task<ActionResult<CollectionsDeltaResponse>> GetChanged(
        [FromQuery] DateTime? since,
        [FromQuery] int? limit)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var effectiveSince = since ?? DateTime.MinValue.ToUniversalTime();
            var effectiveLimit = limit is null ? MaxDeltaLimit : Math.Clamp(limit.Value, 1, MaxDeltaLimit);

            var result = await _deltaHandler.Handle(
                new GetCollectionsChangedSinceQuery(userId, effectiveSince, effectiveLimit));

            return Ok(new CollectionsDeltaResponse
            {
                Items = result.Items.Select(CollectionResponse.FromDto).ToList(),
                Deleted = result.Deleted
                    .Select(t => new TombstoneResponse(t.Id, t.DeletedAt, t.Version))
                    .ToList(),
                NextCursor = null,
                ServerTime = result.ServerTime,
            });
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
    }

    [HttpPost("{id}/items")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConflictResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> AddItem(
        Guid id,
        [FromBody] AddItemRequest request,
        [FromHeader(Name = "If-Match")] string? ifMatch)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var item = new CollectionItemRef(request.Type, request.Uuid);
            await _addItemHandler.Handle(
                new AddItemToCollectionCommand(userId, id, item, ParseIfMatch(ifMatch)));
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedException) { return Forbid(); }
        catch (CollectionNotFoundException)
        {
            return NotFound(new ErrorResponse("Collection not found", $"Collection with ID {id} was not found"));
        }
        catch (OptimisticConcurrencyException occ)
        {
            var current = await _byIdHandler.Handle(new GetCollectionByIdQuery(id, GetAuthenticatedUserId()));
            var body = current is null
                ? new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, null)
                : new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, CollectionResponse.FromDto(current));
            return StatusCode(StatusCodes.Status412PreconditionFailed, body);
        }
        catch (ArgumentException ex)
        {
            return BadRequest(new ErrorResponse("Invalid add-item request", ex.Message));
        }
    }

    [HttpDelete("{id}/items/{type}/{itemUuid}")]
    [ProducesResponseType(StatusCodes.Status204NoContent)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status404NotFound)]
    [ProducesResponseType(typeof(ConflictResponse), StatusCodes.Status412PreconditionFailed)]
    public async Task<IActionResult> RemoveItem(
        Guid id,
        string type,
        string itemUuid,
        [FromHeader(Name = "If-Match")] string? ifMatch)
    {
        try
        {
            var userId = GetAuthenticatedUserId();
            var item = new CollectionItemRef(type, itemUuid);
            await _removeItemHandler.Handle(
                new RemoveItemFromCollectionCommand(userId, id, item, ParseIfMatch(ifMatch)));
            return NoContent();
        }
        catch (UnauthorizedAccessException) { return Forbid(); }
        catch (UnauthorizedException) { return Forbid(); }
        catch (CollectionNotFoundException)
        {
            return NotFound(new ErrorResponse("Collection not found", $"Collection with ID {id} was not found"));
        }
        catch (OptimisticConcurrencyException occ)
        {
            return StatusCode(StatusCodes.Status412PreconditionFailed,
                new ConflictResponse("Version mismatch", occ.Message, occ.ActualVersion, null));
        }
    }
}
