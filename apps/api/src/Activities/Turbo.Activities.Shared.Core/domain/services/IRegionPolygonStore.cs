using NetTopologySuite.Geometries;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// Looks up which named region a geometry sits inside. Backed by a
/// PostGIS <c>ST_Contains</c> query against the <c>geo_regions</c>
/// table. The geo-context service calls this on every activity
/// create/update to populate <see cref="ActivityGeoContext.VarsomRegionId"/>
/// (and Mareano / watershed id when those polygon sets are loaded).
/// </summary>
public interface IRegionPolygonStore
{
    /// <summary>Find the region from <paramref name="source"/> that
    /// contains <paramref name="point"/>. Returns null when no polygon
    /// covers the point — the orchestrator handles a null region by
    /// degrading the avalanche-driver confidence and surfacing the
    /// "no bulletin" rationale.</summary>
    Task<RegionMatch?> FindContainingAsync(
        string source,
        Point point,
        CancellationToken cancellationToken);
}

public sealed record RegionMatch(string Source, string RegionId, string Name);
