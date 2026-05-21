using System.Text.Json.Serialization;
using NetTopologySuite.Geometries;

namespace Turboapi.Geo.domain.value;

/// <summary>
/// Immutable coordinates record
/// </summary>
/// <summary>
/// Immutable coordinates record
/// </summary>
public record Coordinates
{
    [JsonPropertyName("longitude")]
    public double Longitude { get; init; }

    [JsonPropertyName("latitude")]
    public double Latitude { get; init; }

    [JsonConstructor]
    public Coordinates(double longitude, double latitude)
    {
        Longitude = longitude;
        Latitude = latitude;
    }

    // Parameterless constructor if needed by other frameworks or as a fallback.
    // System.Text.Json with [JsonConstructor] on the parameterized one should prefer that.
    public Coordinates()
    {
    }

    // Conversion helpers
    public Point ToPoint(GeometryFactory factory) =>
        factory.CreatePoint(new Coordinate(Longitude, Latitude));

    public static Coordinates FromPoint(Point point) =>
        new(point.X, point.Y);
}