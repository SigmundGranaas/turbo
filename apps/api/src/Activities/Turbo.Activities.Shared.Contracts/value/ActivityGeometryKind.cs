namespace Turboapi.Activities.value;

/// <summary>
/// Coarse-grained geometry shape an activity occupies. Kinds declare which
/// of these they accept (e.g. fishing = Point, hiking = LineString, sea
/// fishing = Polygon). Mapped to PostGIS geometry types one-to-one.
/// </summary>
public enum ActivityGeometryKind
{
    Point = 0,
    LineString = 1,
    Polygon = 2,
}
