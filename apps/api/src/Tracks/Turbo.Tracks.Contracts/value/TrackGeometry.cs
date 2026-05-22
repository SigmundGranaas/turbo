using System.Text.Json.Serialization;
using NetTopologySuite.Geometries;

namespace Turboapi.Tracks.domain.value;

/// <summary>
/// Immutable polyline geometry: the ordered list of (longitude, latitude)
/// points the user recorded, and an optional per-vertex elevation array.
/// Elevations, when present, MUST be the same length as <see cref="Points"/>;
/// the aggregate enforces this on construction.
/// </summary>
public record TrackGeometry
{
    [JsonPropertyName("points")]
    public IReadOnlyList<GeoPoint> Points { get; init; }

    [JsonPropertyName("elevations")]
    public IReadOnlyList<double>? Elevations { get; init; }

    [JsonConstructor]
    public TrackGeometry(IReadOnlyList<GeoPoint> points, IReadOnlyList<double>? elevations = null)
    {
        Points = points ?? Array.Empty<GeoPoint>();
        Elevations = elevations;
    }

    public TrackGeometry()
    {
        Points = Array.Empty<GeoPoint>();
        Elevations = null;
    }

    public LineString ToLineString(GeometryFactory factory)
    {
        if (Points.Count < 2)
        {
            throw new ArgumentException(
                "A track geometry must contain at least two points to form a LINESTRING",
                nameof(Points));
        }
        var coords = new Coordinate[Points.Count];
        for (var i = 0; i < Points.Count; i++)
            coords[i] = new Coordinate(Points[i].Longitude, Points[i].Latitude);
        var line = factory.CreateLineString(coords);
        line.SRID = 4326;
        return line;
    }

    public static TrackGeometry FromLineString(LineString line, IReadOnlyList<double>? elevations = null)
    {
        var points = new GeoPoint[line.NumPoints];
        for (var i = 0; i < line.NumPoints; i++)
        {
            var c = line.GetCoordinateN(i);
            points[i] = new GeoPoint(c.X, c.Y);
        }
        return new TrackGeometry(points, elevations);
    }
}

public record GeoPoint
{
    [JsonPropertyName("longitude")]
    public double Longitude { get; init; }

    [JsonPropertyName("latitude")]
    public double Latitude { get; init; }

    [JsonConstructor]
    public GeoPoint(double longitude, double latitude)
    {
        Longitude = longitude;
        Latitude = latitude;
    }

    public GeoPoint() { }
}
