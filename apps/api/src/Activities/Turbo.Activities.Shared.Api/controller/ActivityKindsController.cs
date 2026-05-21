using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.controller;

/// <summary>
/// Discovery endpoint. Returns the set of activity kinds enabled in this
/// deployment, with rendering and capability hints. Clients hit this on
/// startup to know which kind UIs to surface in the create-picker.
/// </summary>
[ApiController]
[Route("api/activities/kinds")]
[Authorize]
public class ActivityKindsController : ControllerBase
{
    private readonly IActivityKindCatalog _catalog;

    public ActivityKindsController(IActivityKindCatalog catalog) => _catalog = catalog;

    [HttpGet]
    [ProducesResponseType(typeof(ActivityKindsResponse), StatusCodes.Status200OK)]
    public ActionResult<ActivityKindsResponse> List()
    {
        var items = _catalog.All().Select(d => new ActivityKindItem(
            d.Key,
            d.DisplayName,
            d.IconKey,
            d.ColorHex,
            d.AllowedGeometries.Select(g => g.ToString()).ToList(),
            d.ConditionsAvailable)).ToList();
        return Ok(new ActivityKindsResponse(items));
    }
}

public sealed record ActivityKindsResponse(IReadOnlyList<ActivityKindItem> Items);

public sealed record ActivityKindItem(
    string Key,
    string DisplayName,
    string IconKey,
    string ColorHex,
    IReadOnlyList<string> AllowedGeometries,
    bool ConditionsAvailable);
