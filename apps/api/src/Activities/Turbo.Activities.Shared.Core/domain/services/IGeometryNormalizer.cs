using NetTopologySuite.Geometries;
using Turboapi.Activities.value;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// Validates and canonicalises geometry inbound from a kind's controller.
/// Enforces:
///   * SRID is EPSG:4326,
///   * coordinate bounds are sane,
///   * the geometry type is in the kind's <see cref="ActivityGeometryKind"/>
///     allowlist,
///   * (for LineString/Polygon) at least the minimum number of distinct
///     vertices for the shape.
///
/// A separate interface (rather than a base-class hook on the aggregate)
/// keeps the policy out of the kind itself — composition over inheritance.
/// </summary>
public interface IGeometryNormalizer
{
    Geometry Normalize(Geometry input, ActivityGeometryKind expected);
}
