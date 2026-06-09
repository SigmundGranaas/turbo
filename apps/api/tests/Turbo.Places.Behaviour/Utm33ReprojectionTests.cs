using FluentAssertions;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P1a: the GDAL-free UTM33 → WGS84 reprojection. Norway uses EPSG:25833
/// (ETRS89 / UTM zone 33N) across the whole country — including eastern
/// Finnmark at ~30°E, 15° off the central meridian, where a truncated Snyder
/// series loses metres. The control points come straight from Kartverket's
/// transformation service (ws.geonorge.no/transformering), so this test is an
/// independent check, not self-referential. Gate: &lt; 0.5 m everywhere.
/// </summary>
public class Utm33ReprojectionTests
{
    // (label, lon, lat) ↔ (easting=x, northing=y), authoritative — Kartverket
    // /transformering/v1/transformer fra=4258 til=25833.
    public static readonly TheoryData<string, double, double, double, double> ControlPoints = new()
    {
        { "Galdhøpiggen", 8.31248, 61.63644, 146001.63931684673, 6851889.415514315 },
        { "Tromsø",       18.9551, 69.6488,  653416.32548282,     7731676.0560329845 },
        { "Lindesnes",    7.0476,  57.9828,  30378.842477760918,  6454498.112122297 },
        { "Kirkenes",     30.0454, 69.7270,  1076703.3334049513,  7806967.153011698 },
    };

    [Theory]
    [MemberData(nameof(ControlPoints))]
    public void Inverse_transform_matches_Kartverket_within_half_a_metre(
        string label, double expectedLon, double expectedLat, double easting, double northing)
    {
        var (lat, lng) = Utm33.ToWgs84(easting, northing);

        var error = HaversineMetres(lat, lng, expectedLat, expectedLon);
        error.Should().BeLessThan(0.5, $"{label} must reproject within 0.5 m (got {error:F3} m)");
    }

    private static double HaversineMetres(double lat1, double lng1, double lat2, double lng2)
    {
        const double r = 6_371_000.0;
        double Rad(double d) => d * Math.PI / 180.0;
        var dLat = Rad(lat2 - lat1);
        var dLng = Rad(lng2 - lng1);
        var a = Math.Sin(dLat / 2) * Math.Sin(dLat / 2)
                + Math.Cos(Rad(lat1)) * Math.Cos(Rad(lat2)) * Math.Sin(dLng / 2) * Math.Sin(dLng / 2);
        return 2 * r * Math.Asin(Math.Sqrt(a));
    }
}
