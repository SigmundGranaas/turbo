using System.Text.Json;

namespace Turboapi.Places.Core;

/// <summary>
/// Reverse-geocodes a coordinate entirely from our own stack: gather nearest
/// candidates from the store, hand them to place-core, get back a ranked
/// <c>LocationDescription</c>. No third-party API at query time.
/// </summary>
public sealed class ReverseGeocodeService
{
    private readonly IPlaceStore _store;
    private readonly double _radiusM;
    private readonly int _limit;

    public ReverseGeocodeService(IPlaceStore store, double radiusM = 1000, int limit = 25)
    {
        _store = store;
        _radiusM = radiusM;
        _limit = limit;
    }

    public async Task<LocationDescriptionDto?> DescribeAsync(double lat, double lng, CancellationToken ct = default)
    {
        var candidates = await _store.NearestAsync(lat, lng, _radiusM, _limit, ct);

        // The nearest feature's stored enrichment stands in for the queried
        // point: place-core merges kommune/fylke + elevation onto whichever
        // source wins, and uses kommune as the title fallback when no feature
        // qualifies. (M2b will use admin-polygon containment for the latter.)
        var nearest = candidates.FirstOrDefault();

        var input = new ReverseInputDto
        {
            Toponyms = candidates
                .Select(c => new CandidateDto
                {
                    Name = c.Name,
                    Kind = c.Kind,
                    DistanceM = c.DistanceM,
                    Status = c.Status,
                })
                .ToList(),
            Kommune = nearest?.KommuneName is { } kommune
                ? new KommuneDto { Name = kommune, Fylke = nearest.FylkeName }
                : null,
            ElevationM = nearest?.ElevationM,
        };

        var resultJson = PlaceCore.ReverseJson(JsonSerializer.Serialize(input, PlaceCoreJson.Options));
        return resultJson == "null"
            ? null
            : JsonSerializer.Deserialize<LocationDescriptionDto>(resultJson, PlaceCoreJson.Options);
    }
}
