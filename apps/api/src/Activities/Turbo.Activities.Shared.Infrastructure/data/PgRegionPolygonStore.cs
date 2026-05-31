using Microsoft.EntityFrameworkCore;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain.services;

namespace Turboapi.Activities.data;

/// <summary>
/// PostGIS-backed region polygon lookup. Uses NetTopologySuite's
/// <c>Contains</c> predicate which Npgsql translates to PostGIS
/// <c>ST_Contains</c>. The geometry column on <c>geo_regions</c> has a
/// GIST index so the query is cheap even with country-scale polygon
/// sets (Varsom is ~24 polygons, REGINE watersheds are a few thousand).
/// </summary>
public sealed class PgRegionPolygonStore : IRegionPolygonStore
{
    private readonly ActivitySummariesContext _db;

    public PgRegionPolygonStore(ActivitySummariesContext db) => _db = db;

    public async Task<RegionMatch?> FindContainingAsync(
        string source, Point point, CancellationToken cancellationToken)
    {
        var row = await _db.GeoRegions
            .AsNoTracking()
            .Where(r => r.Source == source && r.Geometry.Contains(point))
            .Select(r => new { r.Source, r.RegionId, r.Name })
            .FirstOrDefaultAsync(cancellationToken);
        return row is null ? null : new RegionMatch(row.Source, row.RegionId, row.Name);
    }
}
