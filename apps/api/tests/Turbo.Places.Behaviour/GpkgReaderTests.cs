using FluentAssertions;
using Microsoft.Data.Sqlite;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P1a: read Geonorge GeoPackage (which is SQLite) without GDAL. A GPKG
/// geometry blob is a small "GP" header (byte-order, envelope, srs_id) wrapping
/// standard WKB; we strip the header and hand the WKB to NetTopologySuite.
/// Blobs here are hand-assembled to the GeoPackage spec (not via a GDAL writer),
/// so the parser is checked against the spec, not against itself.
/// </summary>
public class GpkgReaderTests
{
    // Galdhøpiggen in EPSG:25833 (from the Kartverket control point).
    private const double E = 146001.63931684673, Nrt = 6851889.415514315;

    /// <summary>GeoPackageBinary for a 2D point: LE header, no envelope,
    /// srs_id, then LE WKB point. Hand-built per GeoPackage §2.1.3.</summary>
    private static byte[] Gpb(double x, double y, int srsId = 25833)
    {
        using var ms = new MemoryStream();
        ms.WriteByte((byte)'G');
        ms.WriteByte((byte)'P');
        ms.WriteByte(0);     // version
        ms.WriteByte(0x01);  // flags: little-endian header, envelope code 0 (none)
        ms.Write(BitConverter.GetBytes(srsId));        // srs_id (LE int32)
        ms.WriteByte(0x01);                            // WKB byte order: LE
        ms.Write(BitConverter.GetBytes((uint)1));      // WKB type: Point
        ms.Write(BitConverter.GetBytes(x));            // X (LE double)
        ms.Write(BitConverter.GetBytes(y));            // Y (LE double)
        return ms.ToArray();
    }

    [Fact]
    public void Parses_a_GeoPackage_point_blob_to_its_source_coordinate()
    {
        var geom = GpkgGeometry.Parse(Gpb(E, Nrt));

        geom.OgcGeometryType.Should().Be(NetTopologySuite.Geometries.OgcGeometryType.Point);
        geom.Coordinate.X.Should().BeApproximately(E, 1e-6);
        geom.Coordinate.Y.Should().BeApproximately(Nrt, 1e-6);
    }

    [Fact]
    public void Reads_a_GeoPackage_feature_table_and_reprojects_to_WGS84()
    {
        var path = Path.Combine(Path.GetTempPath(), $"mini-{Guid.NewGuid():n}.gpkg");
        try
        {
            WriteMiniGpkg(path,
                (Gpb(E, Nrt), "Galdhøpiggen", "Fjelltopp"),
                (Gpb(653416.32548282, 7731676.0560329845), "Tromsø", "By"));

            var features = new GpkgReader()
                .ReadFeatures(path, "ssr_navn", "geom", ["navn", "type"])
                .ToList();

            features.Should().HaveCount(2);

            var g = features.Single(f => f.Attributes["navn"] == "Galdhøpiggen");
            var (lat, lng) = Utm33.ToWgs84(g.Geometry.Coordinate.X, g.Geometry.Coordinate.Y);
            lat.Should().BeApproximately(61.63644, 1e-4);
            lng.Should().BeApproximately(8.31248, 1e-4);
            g.Attributes["type"].Should().Be("Fjelltopp");

            var t = features.Single(f => f.Attributes["navn"] == "Tromsø");
            var (tlat, tlng) = Utm33.ToWgs84(t.Geometry.Coordinate.X, t.Geometry.Coordinate.Y);
            tlat.Should().BeApproximately(69.6488, 1e-4);
            tlng.Should().BeApproximately(18.9551, 1e-4);
        }
        finally { File.Delete(path); }
    }

    /// <summary>Builds a minimal but valid GeoPackage feature table by hand
    /// (raw SQLite + GPB blobs) — independent of the production reader.</summary>
    private static void WriteMiniGpkg(string path, params (byte[] Geom, string Navn, string Type)[] rows)
    {
        using var conn = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = path,
            Mode = SqliteOpenMode.ReadWriteCreate,
        }.ToString());
        conn.Open();
        using (var ddl = conn.CreateCommand())
        {
            ddl.CommandText =
                "CREATE TABLE ssr_navn (fid INTEGER PRIMARY KEY, geom BLOB, navn TEXT, type TEXT);";
            ddl.ExecuteNonQuery();
        }
        foreach (var (geom, navn, type) in rows)
        {
            using var ins = conn.CreateCommand();
            ins.CommandText = "INSERT INTO ssr_navn (geom, navn, type) VALUES ($g, $n, $t)";
            ins.Parameters.AddWithValue("$g", geom);
            ins.Parameters.AddWithValue("$n", navn);
            ins.Parameters.AddWithValue("$t", type);
            ins.ExecuteNonQuery();
        }
        SqliteConnection.ClearAllPools(); // release the file handle on Windows/CI
    }
}
