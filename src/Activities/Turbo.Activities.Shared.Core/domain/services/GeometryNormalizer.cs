using NetTopologySuite.Geometries;
using Turboapi.Activities.value;

namespace Turboapi.Activities.domain.services;

public sealed class GeometryNormalizer : IGeometryNormalizer
{
    private const int Srid = 4326;

    public Geometry Normalize(Geometry input, ActivityGeometryKind expected)
    {
        ArgumentNullException.ThrowIfNull(input);

        if (input.SRID != Srid)
            input.SRID = Srid;

        switch (expected)
        {
            case ActivityGeometryKind.Point:
                if (input is not Point p)
                    throw new ArgumentException($"Expected Point geometry, got {input.GeometryType}");
                EnsureCoordinateInRange(p.Coordinate);
                return p;

            case ActivityGeometryKind.LineString:
                if (input is not LineString ls)
                    throw new ArgumentException($"Expected LineString geometry, got {input.GeometryType}");
                if (ls.NumPoints < 2)
                    throw new ArgumentException("LineString must contain at least two points");
                foreach (var c in ls.Coordinates) EnsureCoordinateInRange(c);
                return ls;

            case ActivityGeometryKind.Polygon:
                if (input is not Polygon pg)
                    throw new ArgumentException($"Expected Polygon geometry, got {input.GeometryType}");
                if (pg.Shell.NumPoints < 4)
                    throw new ArgumentException("Polygon shell must contain at least four points (including closing vertex)");
                foreach (var c in pg.Coordinates) EnsureCoordinateInRange(c);
                return pg;

            default:
                throw new ArgumentOutOfRangeException(nameof(expected), expected, "Unknown geometry kind");
        }
    }

    private static void EnsureCoordinateInRange(Coordinate c)
    {
        if (double.IsNaN(c.X) || double.IsNaN(c.Y))
            throw new ArgumentException("Coordinate contains NaN");
        if (c.X < -180.0 || c.X > 180.0)
            throw new ArgumentException($"Longitude {c.X} out of [-180,180]");
        if (c.Y < -90.0 || c.Y > 90.0)
            throw new ArgumentException($"Latitude {c.Y} out of [-90,90]");
    }
}
