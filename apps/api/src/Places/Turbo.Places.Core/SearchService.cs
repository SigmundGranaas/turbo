using System.Text.Json;
using Turboapi.Places;

namespace Turboapi.Places.Core;

/// <summary>One ranked, positioned forward-search result.</summary>
public sealed record PlaceSearchResult(
    string Title,
    string? Description,
    string Icon,
    double Lat,
    double Lng);

/// <summary>
/// Forward search from our own stack: trigram/prefix retrieval from the store,
/// canonical ordering + icon mapping by place-core (exact > prefix > substring,
/// nearer wins within a class — "the Storvatnet near me" first).
/// </summary>
public sealed class SearchService
{
    private readonly IPlaceStore _store;
    private readonly int _retrievalLimit;

    public SearchService(IPlaceStore store, int retrievalLimit = 30)
    {
        _store = store;
        _retrievalLimit = retrievalLimit;
    }

    public async Task<IReadOnlyList<PlaceSearchResult>> SearchAsync(
        string query, double? nearLat, double? nearLng, int limit, CancellationToken ct = default)
    {
        if (string.IsNullOrWhiteSpace(query)) return Array.Empty<PlaceSearchResult>();

        var rows = await _store.SearchAsync(query, nearLat, nearLng, _retrievalLimit, ct);
        if (rows.Count == 0) return Array.Empty<PlaceSearchResult>();

        var candidates = rows
            .Select(r => new SearchCandidateDto
            {
                Name = r.Name,
                Kind = r.Kind,
                DistanceM = r.DistanceM,
                // Same subtitle the live Stedsnavn backend composed:
                // "navneobjekttype, kommune, fylke".
                Description = string.Join(", ",
                    new[] { r.Kind, r.KommuneName, r.FylkeName }
                        .Where(s => !string.IsNullOrEmpty(s))),
            })
            .ToList();

        var hitsJson = PlaceCore.SearchJson(
            query, JsonSerializer.Serialize(candidates, PlaceCoreJson.Options));
        var hits = JsonSerializer.Deserialize<List<SearchHitDto>>(hitsJson, PlaceCoreJson.Options)
            ?? new List<SearchHitDto>();

        return hits
            .Take(limit)
            .Select(h =>
            {
                var row = rows[h.Index];
                return new PlaceSearchResult(h.Title, h.Description, h.Icon, row.Lat, row.Lng);
            })
            .ToList();
    }
}
