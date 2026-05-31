using NetTopologySuite.Geometries;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.services;

/// <summary>
/// Placeholder <see cref="IActivityGeoContextService"/> for the foundations
/// landing. Returns a minimal geo context — geometry length only, no
/// derived aspect/slope/region/station data. Real implementation arrives
/// in Phase 2 (pilot) once Kartverket DEM + Varsom region polygons are
/// wired in. Orchestrators gracefully degrade to weather-only signals
/// when the context is sparse.
/// </summary>
public sealed class StubActivityGeoContextService : IActivityGeoContextService
{
    public Task<ActivityGeoContext> ComputeAndStoreAsync(
        Guid activityId, Geometry geometry, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(geometry);
        return Task.FromResult(BuildMinimal(activityId, geometry));
    }

    public Task<ActivityGeoContext?> GetAsync(Guid activityId, CancellationToken cancellationToken)
        => Task.FromResult<ActivityGeoContext?>(null);

    public Task<ActivityGeoContext> ComputeTransientAsync(Geometry geometry, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(geometry);
        return Task.FromResult(BuildMinimal(Guid.Empty, geometry));
    }

    private static ActivityGeoContext BuildMinimal(Guid activityId, Geometry geometry)
    {
        var lengthM = geometry is LineString ls ? ApproxLengthM(ls) : 0.0;
        return new ActivityGeoContext(
            activityId: activityId,
            version: 0,
            elevationMinM: 0,
            elevationMaxM: 0,
            ascentM: 0,
            descentM: 0,
            lengthM: lengthM,
            aspectMix: Array.Empty<AspectShare>(),
            slopeHistogram: Array.Empty<SlopeBin>(),
            varsomRegionId: null,
            varsomRegionName: null,
            mareanoCellId: null,
            watershedHrefId: null,
            nveStations: Array.Empty<NearestStation>(),
            treelineCrossings: null,
            aboveTreelineFractionM: null,
            touchesCoastline: false,
            distanceToCoastM: null,
            computedAt: DateTime.UtcNow);
    }

    /// <summary>Haversine-summed length in meters for an EPSG:4326
    /// linestring. Good enough for a stub — the real geo-context service
    /// uses NTS' geographic projection.</summary>
    private static double ApproxLengthM(LineString ls)
    {
        const double earthRadiusM = 6_371_000;
        double total = 0;
        var coords = ls.Coordinates;
        for (var i = 1; i < coords.Length; i++)
        {
            var lat1 = coords[i - 1].Y * Math.PI / 180.0;
            var lat2 = coords[i].Y * Math.PI / 180.0;
            var dLat = lat2 - lat1;
            var dLon = (coords[i].X - coords[i - 1].X) * Math.PI / 180.0;
            var a = Math.Sin(dLat / 2) * Math.Sin(dLat / 2)
                    + Math.Cos(lat1) * Math.Cos(lat2) * Math.Sin(dLon / 2) * Math.Sin(dLon / 2);
            var c = 2 * Math.Atan2(Math.Sqrt(a), Math.Sqrt(1 - a));
            total += earthRadiusM * c;
        }
        return total;
    }
}
