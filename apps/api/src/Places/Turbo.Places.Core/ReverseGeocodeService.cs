using System.Text.Json;

namespace Turboapi.Places.Core;

/// <summary>
/// Reverse-geocodes a coordinate entirely from our own stack: gather nearest
/// toponym candidates + polygon containment (protected area, kommune) from the
/// store, hand them to place-core, get back a ranked
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
        // Both queries are independent — run them concurrently, mirroring the
        // live app's parallel fan-out (only against our own store).
        var nearestTask = _store.NearestAsync(lat, lng, _radiusM, _limit, ct);
        var containingTask = _store.ContainingAsync(lat, lng, ct);
        await Task.WhenAll(nearestTask, containingTask);
        var candidates = nearestTask.Result;
        var containment = containingTask.Result;

        // Kommune comes from true polygon containment; the nearest feature's
        // stored kommune is the fallback when no kommune polygon is loaded.
        var nearest = candidates.FirstOrDefault();
        var kommune = containment.KommuneName ?? nearest?.KommuneName;
        var fylke = containment.KommuneName is not null ? containment.FylkeName : nearest?.FylkeName;

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
            ProtectedArea = containment.ProtectedAreaName is { } park
                ? new ProtectedAreaDto { Name = park, Kind = containment.ProtectedAreaKind }
                : null,
            Kommune = kommune is not null
                ? new KommuneDto { Name = kommune, Fylke = fylke }
                : null,
            // Elevation is precomputed per feature, not per arbitrary point —
            // use the nearest feature's value (right when a tight feature
            // wins; absent in pure wilderness, where the field stays null).
            ElevationM = nearest?.ElevationM,
        };

        var resultJson = PlaceCore.ReverseJson(JsonSerializer.Serialize(input, PlaceCoreJson.Options));
        return resultJson == "null"
            ? null
            : JsonSerializer.Deserialize<LocationDescriptionDto>(resultJson, PlaceCoreJson.Options);
    }
}
