using NetTopologySuite.Geometries;

namespace Turboapi.Activities.data.model;

/// <summary>
/// A named geographic region polygon — Varsom avalanche region,
/// Mareano seabed cell, NVE REGINE watershed, etc. The geo-context
/// service does a PostGIS <c>ST_Contains</c> against this table to
/// fill in the corresponding ids on activity creation.
///
/// <see cref="Source"/> identifies which polygon set this row belongs
/// to (<c>"varsom_region"</c>, <c>"mareano_cell"</c>, …). Region ids
/// are stored as text so seeders for both numeric-id and string-id
/// sources (Varsom uses int, REGINE uses href strings) share one
/// table.
/// </summary>
public class GeoRegionEntity
{
    public long Id { get; set; }

    /// <summary>Polygon-set key. Indexed for the
    /// <c>WHERE source = ? AND ST_Contains(...)</c> lookup pattern.</summary>
    public required string Source { get; set; }

    /// <summary>Upstream id (numeric for Varsom, free-form for REGINE
    /// watersheds). Stored as text so a single column covers all sources.</summary>
    public required string RegionId { get; set; }

    public required string Name { get; set; }

    public required Geometry Geometry { get; set; }
}
