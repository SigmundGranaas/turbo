namespace Turboapi.Places.Ingestion;

/// <summary>
/// EPSG:25833 (ETRS89 / UTM zone 33N) → WGS84 lat/lng. Norway uses zone 33
/// nationwide, so points reach ~15° off the 15°E central meridian (eastern
/// Finnmark); a truncated Snyder series loses metres there. This uses the
/// Krüger/Karney n-series (order 6), accurate to sub-millimetre across and
/// well beyond a zone. ETRS89 ≈ WGS84 to within centimetres, so the datum
/// shift is treated as identity (the GRS80 ellipsoid is used).
/// </summary>
public static class Utm33
{
    private const double A = 6_378_137.0;          // GRS80 semi-major axis
    private const double F = 1.0 / 298.257222101;  // GRS80 flattening
    private const double K0 = 0.9996;
    private const double FalseEasting = 500_000.0;
    private const double Lon0 = 15.0 * Math.PI / 180.0; // zone 33 central meridian

    // n-series constants, precomputed once.
    private static readonly double N = F / (2 - F);
    private static readonly double RectifyingRadius =
        A / (1 + N) * (1 + N * N / 4 + Math.Pow(N, 4) / 64 + Math.Pow(N, 6) / 256);

    // Inverse (TM → conformal) coefficients β1..β6.
    private static readonly double[] Beta = BuildBeta();

    // Conformal-latitude → geodetic-latitude coefficients δ1..δ6.
    private static readonly double[] Delta = BuildDelta();

    /// <summary>Returns (latitude, longitude) in WGS84 degrees.</summary>
    public static (double Lat, double Lng) ToWgs84(double easting, double northing)
    {
        var xi = northing / (K0 * RectifyingRadius);
        var eta = (easting - FalseEasting) / (K0 * RectifyingRadius);

        var xiPrime = xi;
        var etaPrime = eta;
        for (var j = 1; j <= 6; j++)
        {
            xiPrime -= Beta[j - 1] * Math.Sin(2 * j * xi) * Math.Cosh(2 * j * eta);
            etaPrime -= Beta[j - 1] * Math.Cos(2 * j * xi) * Math.Sinh(2 * j * eta);
        }

        var chi = Math.Asin(Math.Sin(xiPrime) / Math.Cosh(etaPrime)); // conformal latitude

        var phi = chi;
        for (var j = 1; j <= 6; j++)
        {
            phi += Delta[j - 1] * Math.Sin(2 * j * chi);
        }

        var lambda = Lon0 + Math.Atan2(Math.Sinh(etaPrime), Math.Cos(xiPrime));

        return (phi * 180.0 / Math.PI, lambda * 180.0 / Math.PI);
    }

    private static double[] BuildBeta()
    {
        double n = N, n2 = n * n, n3 = n2 * n, n4 = n3 * n, n5 = n4 * n, n6 = n5 * n;
        return
        [
            1.0 / 2 * n - 2.0 / 3 * n2 + 37.0 / 96 * n3 - 1.0 / 360 * n4 - 81.0 / 512 * n5 + 96199.0 / 604800 * n6,
            1.0 / 48 * n2 + 1.0 / 15 * n3 - 437.0 / 1440 * n4 + 46.0 / 105 * n5 - 1118711.0 / 3870720 * n6,
            17.0 / 480 * n3 - 37.0 / 840 * n4 - 209.0 / 4480 * n5 + 5569.0 / 90720 * n6,
            4397.0 / 161280 * n4 - 11.0 / 504 * n5 - 830251.0 / 7257600 * n6,
            4583.0 / 161280 * n5 - 108847.0 / 3991680 * n6,
            20648693.0 / 638668800 * n6,
        ];
    }

    private static double[] BuildDelta()
    {
        double n = N, n2 = n * n, n3 = n2 * n, n4 = n3 * n, n5 = n4 * n, n6 = n5 * n;
        return
        [
            2 * n - 2.0 / 3 * n2 - 2 * n3 + 116.0 / 45 * n4 + 26.0 / 45 * n5 - 2854.0 / 675 * n6,
            7.0 / 3 * n2 - 8.0 / 5 * n3 - 227.0 / 45 * n4 + 2704.0 / 315 * n5 + 2323.0 / 945 * n6,
            56.0 / 15 * n3 - 136.0 / 35 * n4 - 1262.0 / 105 * n5 + 73814.0 / 2835 * n6,
            4279.0 / 630 * n4 - 332.0 / 35 * n5 - 399572.0 / 14175 * n6,
            4174.0 / 315 * n5 - 144838.0 / 6237 * n6,
            601676.0 / 22680 * n6,
        ];
    }
}
