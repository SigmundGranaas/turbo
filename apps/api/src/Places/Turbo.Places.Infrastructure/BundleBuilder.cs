using System.Globalization;
using System.Text.Json;
using Microsoft.Data.Sqlite;
using Npgsql;

namespace Turboapi.Places.Infrastructure;

/// <summary>
/// Builds an offline region bundle: a SQLite file (R*Tree over place points +
/// polygon rings for containment) sliced from the live PostGIS dataset, in the
/// exact schema the place-core embedded engine reads. The bundle carries the
/// same ruleset artifact the server runs, so an offline answer equals the
/// online one by construction.
/// </summary>
public sealed class BundleBuilder
{
    private readonly string _connectionString;

    public BundleBuilder(string connectionString) => _connectionString = connectionString;

    /// <summary>Write a bundle for the WGS84 bbox to <paramref name="outputPath"/>.</summary>
    public async Task BuildAsync(
        double minLng, double minLat, double maxLng, double maxLat,
        string rulesetJson, string datasetVersion, string outputPath,
        CancellationToken ct = default)
    {
        await using var pg = new NpgsqlConnection(_connectionString);
        await pg.OpenAsync(ct);

        var sqlitePath = new SqliteConnectionStringBuilder
        {
            DataSource = outputPath,
            Mode = SqliteOpenMode.ReadWriteCreate,
        }.ToString();
        await using var bundle = new SqliteConnection(sqlitePath);
        await bundle.OpenAsync(ct);

        await ExecuteAsync(bundle,
            """
            PRAGMA journal_mode=OFF;
            CREATE TABLE manifest(key TEXT, value TEXT);
            CREATE TABLE ruleset(json TEXT);
            CREATE TABLE places(id INTEGER PRIMARY KEY, name TEXT, name_fold TEXT, kind TEXT,
                lat REAL, lng REAL, status TEXT, elevation_m REAL, kommune TEXT, fylke TEXT);
            CREATE VIRTUAL TABLE places_rtree USING rtree(id, minLat, maxLat, minLng, maxLng);
            CREATE TABLE areas(id INTEGER PRIMARY KEY, area_type TEXT, name TEXT, kind TEXT, rings_json TEXT);
            CREATE VIRTUAL TABLE areas_rtree USING rtree(id, minLat, maxLat, minLng, maxLng);
            """, ct);

        await using (var tx = (SqliteTransaction)await bundle.BeginTransactionAsync(ct))
        {
            await WriteManifestAsync(bundle, tx, minLng, minLat, maxLng, maxLat, rulesetJson, datasetVersion, ct);
            await CopyPlacesAsync(pg, bundle, tx, minLng, minLat, maxLng, maxLat, ct);
            await CopyAreasAsync(pg, bundle, tx, minLng, minLat, maxLng, maxLat, ct);
            await tx.CommitAsync(ct);
        }
    }

    private static async Task WriteManifestAsync(
        SqliteConnection bundle, SqliteTransaction tx,
        double minLng, double minLat, double maxLng, double maxLat,
        string rulesetJson, string datasetVersion, CancellationToken ct)
    {
        using var doc = JsonDocument.Parse(rulesetJson);
        var rulesetVersion = doc.RootElement.TryGetProperty("version", out var v) ? v.GetString() : "unknown";

        var manifest = new (string Key, string Value)[]
        {
            ("format_version", "1"),
            ("dataset_version", datasetVersion),
            ("ruleset_version", rulesetVersion ?? "unknown"),
            ("bbox", string.Create(CultureInfo.InvariantCulture, $"{minLng},{minLat},{maxLng},{maxLat}")),
            ("attribution", "© Kartverket / Miljødirektoratet (NLOD)"),
        };
        foreach (var (key, value) in manifest)
        {
            using var cmd = bundle.CreateCommand();
            cmd.Transaction = tx;
            cmd.CommandText = "INSERT INTO manifest(key,value) VALUES ($k,$v)";
            cmd.Parameters.AddWithValue("$k", key);
            cmd.Parameters.AddWithValue("$v", value);
            await cmd.ExecuteNonQueryAsync(ct);
        }

        using var rs = bundle.CreateCommand();
        rs.Transaction = tx;
        rs.CommandText = "INSERT INTO ruleset(json) VALUES ($j)";
        rs.Parameters.AddWithValue("$j", rulesetJson);
        await rs.ExecuteNonQueryAsync(ct);
    }

    private static async Task CopyPlacesAsync(
        NpgsqlConnection pg, SqliteConnection bundle, SqliteTransaction tx,
        double minLng, double minLat, double maxLng, double maxLat, CancellationToken ct)
    {
        await using var read = pg.CreateCommand();
        read.CommandText = """
            SELECT primary_name, name_fold, feature_type, ST_Y(geom), ST_X(geom),
                   status, elevation_m, kommune_name, fylke_name
            FROM places.places
            WHERE geom && ST_MakeEnvelope(@minLng, @minLat, @maxLng, @maxLat, 4326)
            """;
        read.Parameters.AddWithValue("minLng", minLng);
        read.Parameters.AddWithValue("minLat", minLat);
        read.Parameters.AddWithValue("maxLng", maxLng);
        read.Parameters.AddWithValue("maxLat", maxLat);

        using var insP = bundle.CreateCommand();
        insP.Transaction = tx;
        insP.CommandText = """
            INSERT INTO places(id,name,name_fold,kind,lat,lng,status,elevation_m,kommune,fylke)
            VALUES ($id,$name,$fold,$kind,$lat,$lng,$status,$elev,$kommune,$fylke)
            """;
        using var insR = bundle.CreateCommand();
        insR.Transaction = tx;
        insR.CommandText =
            "INSERT INTO places_rtree(id,minLat,maxLat,minLng,maxLng) VALUES ($id,$lat,$lat,$lng,$lng)";

        var id = 0L;
        await using var r = await read.ExecuteReaderAsync(ct);
        while (await r.ReadAsync(ct))
        {
            id++;
            var lat = r.GetDouble(3);
            var lng = r.GetDouble(4);
            insP.Parameters.Clear();
            insP.Parameters.AddWithValue("$id", id);
            insP.Parameters.AddWithValue("$name", r.GetString(0));
            insP.Parameters.AddWithValue("$fold", r.GetString(1));
            insP.Parameters.AddWithValue("$kind", r.GetString(2));
            insP.Parameters.AddWithValue("$lat", lat);
            insP.Parameters.AddWithValue("$lng", lng);
            insP.Parameters.AddWithValue("$status", r.GetString(5));
            insP.Parameters.AddWithValue("$elev", r.IsDBNull(6) ? DBNull.Value : r.GetDouble(6));
            insP.Parameters.AddWithValue("$kommune", r.IsDBNull(7) ? DBNull.Value : r.GetString(7));
            insP.Parameters.AddWithValue("$fylke", r.IsDBNull(8) ? DBNull.Value : r.GetString(8));
            await insP.ExecuteNonQueryAsync(ct);

            insR.Parameters.Clear();
            insR.Parameters.AddWithValue("$id", id);
            insR.Parameters.AddWithValue("$lat", lat);
            insR.Parameters.AddWithValue("$lng", lng);
            await insR.ExecuteNonQueryAsync(ct);
        }
    }

    private static async Task CopyAreasAsync(
        NpgsqlConnection pg, SqliteConnection bundle, SqliteTransaction tx,
        double minLng, double minLat, double maxLng, double maxLat, CancellationToken ct)
    {
        await using var read = pg.CreateCommand();
        read.CommandText = """
            SELECT area_type, name, kind, ST_AsGeoJSON(geom)
            FROM places.areas
            WHERE geom && ST_MakeEnvelope(@minLng, @minLat, @maxLng, @maxLat, 4326)
            """;
        read.Parameters.AddWithValue("minLng", minLng);
        read.Parameters.AddWithValue("minLat", minLat);
        read.Parameters.AddWithValue("maxLng", maxLng);
        read.Parameters.AddWithValue("maxLat", maxLat);

        var id = 0L;
        await using var r = await read.ExecuteReaderAsync(ct);
        var areas = new List<(string Type, string Name, string? Kind, string GeoJson)>();
        while (await r.ReadAsync(ct))
        {
            areas.Add((r.GetString(0), r.GetString(1), r.IsDBNull(2) ? null : r.GetString(2), r.GetString(3)));
        }

        foreach (var (type, name, kind, geoJson) in areas)
        {
            // Expand MultiPolygon to one row per polygon so the engine's
            // single-polygon containment test stays simple.
            foreach (var (rings, bbox) in PolygonsOf(geoJson))
            {
                id++;
                using var insA = bundle.CreateCommand();
                insA.Transaction = tx;
                insA.CommandText =
                    "INSERT INTO areas(id,area_type,name,kind,rings_json) VALUES ($id,$type,$name,$kind,$rings)";
                insA.Parameters.AddWithValue("$id", id);
                insA.Parameters.AddWithValue("$type", type);
                insA.Parameters.AddWithValue("$name", name);
                insA.Parameters.AddWithValue("$kind", (object?)kind ?? DBNull.Value);
                insA.Parameters.AddWithValue("$rings", rings);
                await insA.ExecuteNonQueryAsync(ct);

                using var insR = bundle.CreateCommand();
                insR.Transaction = tx;
                insR.CommandText =
                    "INSERT INTO areas_rtree(id,minLat,maxLat,minLng,maxLng) VALUES ($id,$minLat,$maxLat,$minLng,$maxLng)";
                insR.Parameters.AddWithValue("$id", id);
                insR.Parameters.AddWithValue("$minLat", bbox.MinLat);
                insR.Parameters.AddWithValue("$maxLat", bbox.MaxLat);
                insR.Parameters.AddWithValue("$minLng", bbox.MinLng);
                insR.Parameters.AddWithValue("$maxLng", bbox.MaxLng);
                await insR.ExecuteNonQueryAsync(ct);
            }
        }
    }

    /// <summary>Yields each polygon's rings (as the engine's rings_json
    /// <c>[[[lng,lat],…],…]</c>) + its bbox, from a GeoJSON Polygon or
    /// MultiPolygon.</summary>
    private static IEnumerable<(string RingsJson, (double MinLng, double MinLat, double MaxLng, double MaxLat) Bbox)>
        PolygonsOf(string geoJson)
    {
        using var doc = JsonDocument.Parse(geoJson);
        var root = doc.RootElement;
        var type = root.GetProperty("type").GetString();
        var coords = root.GetProperty("coordinates");

        if (type == "Polygon")
        {
            yield return RingsAndBbox(coords);
        }
        else if (type == "MultiPolygon")
        {
            foreach (var polygon in coords.EnumerateArray())
                yield return RingsAndBbox(polygon);
        }
    }

    private static (string, (double, double, double, double)) RingsAndBbox(JsonElement polygonRings)
    {
        double minLng = double.MaxValue, minLat = double.MaxValue, maxLng = double.MinValue, maxLat = double.MinValue;
        foreach (var ring in polygonRings.EnumerateArray())
        foreach (var pt in ring.EnumerateArray())
        {
            var lng = pt[0].GetDouble();
            var lat = pt[1].GetDouble();
            minLng = Math.Min(minLng, lng); maxLng = Math.Max(maxLng, lng);
            minLat = Math.Min(minLat, lat); maxLat = Math.Max(maxLat, lat);
        }
        // The GeoJSON coordinates array IS already [[[lng,lat],…],…].
        return (polygonRings.GetRawText(), (minLng, minLat, maxLng, maxLat));
    }

    private static async Task ExecuteAsync(SqliteConnection conn, string sql, CancellationToken ct)
    {
        using var cmd = conn.CreateCommand();
        cmd.CommandText = sql;
        await cmd.ExecuteNonQueryAsync(ct);
    }
}
