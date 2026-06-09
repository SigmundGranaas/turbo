using Npgsql;
using Turboapi.Places.Core;

namespace Turboapi.Places.Infrastructure;

/// <summary>
/// PostGIS-backed <see cref="IPlaceStore"/> using Npgsql directly (raw SQL is
/// the robust choice for the spatial read path; the eventual HTTP module uses
/// EF Core for the rest). Schema matches
/// docs/architecture/2026-06-places-backend-plan.md §2, trimmed to the columns
/// the M1 slice exercises.
/// </summary>
public sealed class PgPlaceStore : IPlaceStore
{
    private readonly string _connectionString;

    public PgPlaceStore(string connectionString) => _connectionString = connectionString;

    public async Task EnsureSchemaAsync(CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var cmd = conn.CreateCommand();
        cmd.CommandText = """
            CREATE EXTENSION IF NOT EXISTS postgis;
            CREATE EXTENSION IF NOT EXISTS pg_trgm;
            CREATE SCHEMA IF NOT EXISTS places;
            CREATE TABLE IF NOT EXISTS places.places (
                source          text NOT NULL,
                source_id       text NOT NULL,
                feature_type    text NOT NULL,
                primary_name    text NOT NULL,
                name_fold       text NOT NULL,
                status          text NOT NULL DEFAULT 'aktiv',
                geom            geometry(Point,4326) NOT NULL,
                centroid        geography(Point,4326) NOT NULL,
                elevation_m     double precision,
                kommune_name    text,
                fylke_name      text,
                dataset_version text NOT NULL,
                updated_at      timestamptz NOT NULL DEFAULT now(),
                PRIMARY KEY (source, source_id)
            );
            -- Idempotent column adds for a table created by an earlier slice.
            ALTER TABLE places.places ADD COLUMN IF NOT EXISTS elevation_m  double precision;
            ALTER TABLE places.places ADD COLUMN IF NOT EXISTS kommune_name text;
            ALTER TABLE places.places ADD COLUMN IF NOT EXISTS fylke_name   text;
            CREATE INDEX IF NOT EXISTS places_centroid_gist ON places.places USING gist (centroid);
            CREATE INDEX IF NOT EXISTS places_geom_gist     ON places.places USING gist (geom);
            CREATE INDEX IF NOT EXISTS places_name_trgm     ON places.places USING gin (name_fold gin_trgm_ops);
            CREATE TABLE IF NOT EXISTS places.areas (
                source          text NOT NULL,
                source_id       text NOT NULL,
                area_type       text NOT NULL,   -- 'protected_area' | 'kommune'
                name            text NOT NULL,
                kind            text,            -- verneform | fylke name
                geom            geometry(Geometry,4326) NOT NULL,
                dataset_version text NOT NULL,
                updated_at      timestamptz NOT NULL DEFAULT now(),
                PRIMARY KEY (source, source_id)
            );
            CREATE INDEX IF NOT EXISTS areas_geom_gist ON places.areas USING gist (geom);
            CREATE INDEX IF NOT EXISTS areas_type      ON places.areas (area_type);
            """;
        await cmd.ExecuteNonQueryAsync(ct);
    }

    public async Task<int> UpsertAreasAsync(
        IReadOnlyCollection<Area> areas, string datasetVersion, CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var tx = await conn.BeginTransactionAsync(ct);

        await using var cmd = conn.CreateCommand();
        // ST_MakeValid: real-world coastline/boundary polygons routinely carry
        // self-intersections that would otherwise poison ST_Contains.
        cmd.CommandText = """
            INSERT INTO places.areas (source, source_id, area_type, name, kind, geom, dataset_version)
            VALUES (@source, @sid, @type, @name, @kind,
                    ST_MakeValid(ST_SetSRID(ST_GeomFromGeoJSON(@geojson), 4326)),
                    @ver)
            ON CONFLICT (source, source_id) DO UPDATE SET
                area_type       = EXCLUDED.area_type,
                name            = EXCLUDED.name,
                kind            = EXCLUDED.kind,
                geom            = EXCLUDED.geom,
                dataset_version = EXCLUDED.dataset_version,
                updated_at      = now();
            """;
        var pSource = cmd.Parameters.Add("source", NpgsqlTypes.NpgsqlDbType.Text);
        var pSid = cmd.Parameters.Add("sid", NpgsqlTypes.NpgsqlDbType.Text);
        var pType = cmd.Parameters.Add("type", NpgsqlTypes.NpgsqlDbType.Text);
        var pName = cmd.Parameters.Add("name", NpgsqlTypes.NpgsqlDbType.Text);
        var pKind = cmd.Parameters.Add("kind", NpgsqlTypes.NpgsqlDbType.Text);
        var pGeo = cmd.Parameters.Add("geojson", NpgsqlTypes.NpgsqlDbType.Text);
        var pVer = cmd.Parameters.Add("ver", NpgsqlTypes.NpgsqlDbType.Text);
        await cmd.PrepareAsync(ct);

        var n = 0;
        foreach (var a in areas)
        {
            pSource.Value = a.Source;
            pSid.Value = a.SourceId;
            pType.Value = a.AreaType;
            pName.Value = a.Name;
            pKind.Value = (object?)a.Kind ?? DBNull.Value;
            pGeo.Value = a.GeoJsonGeometry;
            pVer.Value = datasetVersion;
            n += await cmd.ExecuteNonQueryAsync(ct);
        }

        await tx.CommitAsync(ct);
        return n;
    }

    public async Task<Containment> ContainingAsync(double lat, double lng, CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var cmd = conn.CreateCommand();
        // One pass over the GIST index; smallest containing polygon per type
        // (a nature reserve inside a national park should win the title).
        cmd.CommandText = """
            SELECT DISTINCT ON (area_type) area_type, name, kind
            FROM places.areas
            WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint(@lng, @lat), 4326))
            ORDER BY area_type, ST_Area(geom) ASC;
            """;
        cmd.Parameters.AddWithValue("lng", lng);
        cmd.Parameters.AddWithValue("lat", lat);

        string? parkName = null, parkKind = null, kommune = null, fylke = null;
        await using var reader = await cmd.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
        {
            var type = reader.GetString(0);
            var name = reader.GetString(1);
            var kind = reader.IsDBNull(2) ? null : reader.GetString(2);
            switch (type)
            {
                case "protected_area":
                    parkName = name;
                    parkKind = kind;
                    break;
                case "kommune":
                    kommune = name;
                    fylke = kind;
                    break;
            }
        }
        return new Containment(parkName, parkKind, kommune, fylke);
    }

    public async Task<int> UpsertAsync(
        IReadOnlyCollection<Place> places, string datasetVersion, CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var tx = await conn.BeginTransactionAsync(ct);

        await using var cmd = conn.CreateCommand();
        cmd.CommandText = """
            INSERT INTO places.places
                (source, source_id, feature_type, primary_name, name_fold, status, geom, centroid,
                 elevation_m, kommune_name, fylke_name, dataset_version)
            VALUES
                (@source, @sid, @ft, @name, @fold, @status,
                 ST_SetSRID(ST_MakePoint(@lng, @lat), 4326),
                 ST_SetSRID(ST_MakePoint(@lng, @lat), 4326)::geography,
                 @elev, @kommune, @fylke, @ver)
            ON CONFLICT (source, source_id) DO UPDATE SET
                feature_type    = EXCLUDED.feature_type,
                primary_name    = EXCLUDED.primary_name,
                name_fold       = EXCLUDED.name_fold,
                status          = EXCLUDED.status,
                geom            = EXCLUDED.geom,
                centroid        = EXCLUDED.centroid,
                elevation_m     = EXCLUDED.elevation_m,
                kommune_name    = EXCLUDED.kommune_name,
                fylke_name      = EXCLUDED.fylke_name,
                dataset_version = EXCLUDED.dataset_version,
                updated_at      = now();
            """;
        var pSource = cmd.Parameters.Add("source", NpgsqlTypes.NpgsqlDbType.Text);
        var pSid = cmd.Parameters.Add("sid", NpgsqlTypes.NpgsqlDbType.Text);
        var pFt = cmd.Parameters.Add("ft", NpgsqlTypes.NpgsqlDbType.Text);
        var pName = cmd.Parameters.Add("name", NpgsqlTypes.NpgsqlDbType.Text);
        var pFold = cmd.Parameters.Add("fold", NpgsqlTypes.NpgsqlDbType.Text);
        var pStatus = cmd.Parameters.Add("status", NpgsqlTypes.NpgsqlDbType.Text);
        var pLng = cmd.Parameters.Add("lng", NpgsqlTypes.NpgsqlDbType.Double);
        var pLat = cmd.Parameters.Add("lat", NpgsqlTypes.NpgsqlDbType.Double);
        var pElev = cmd.Parameters.Add("elev", NpgsqlTypes.NpgsqlDbType.Double);
        var pKommune = cmd.Parameters.Add("kommune", NpgsqlTypes.NpgsqlDbType.Text);
        var pFylke = cmd.Parameters.Add("fylke", NpgsqlTypes.NpgsqlDbType.Text);
        var pVer = cmd.Parameters.Add("ver", NpgsqlTypes.NpgsqlDbType.Text);
        await cmd.PrepareAsync(ct);

        var n = 0;
        foreach (var p in places)
        {
            pSource.Value = p.Source;
            pSid.Value = p.SourceId;
            pFt.Value = p.FeatureType;
            pName.Value = p.PrimaryName;
            pFold.Value = p.PrimaryName.ToLowerInvariant();
            pStatus.Value = p.Status;
            pLng.Value = p.Lng;
            pLat.Value = p.Lat;
            pElev.Value = (object?)p.ElevationM ?? DBNull.Value;
            pKommune.Value = (object?)p.KommuneName ?? DBNull.Value;
            pFylke.Value = (object?)p.FylkeName ?? DBNull.Value;
            pVer.Value = datasetVersion;
            n += await cmd.ExecuteNonQueryAsync(ct);
        }

        await tx.CommitAsync(ct);
        return n;
    }

    public async Task<IReadOnlyList<ReverseCandidate>> NearestAsync(
        double lat, double lng, double radiusM, int limit, CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var cmd = conn.CreateCommand();
        cmd.CommandText = """
            SELECT primary_name, feature_type, status,
                   ST_Distance(centroid, ST_SetSRID(ST_MakePoint(@lng, @lat), 4326)::geography) AS d,
                   elevation_m, kommune_name, fylke_name
            FROM places.places
            WHERE ST_DWithin(centroid, ST_SetSRID(ST_MakePoint(@lng, @lat), 4326)::geography, @radius)
            ORDER BY centroid <-> ST_SetSRID(ST_MakePoint(@lng, @lat), 4326)::geography
            LIMIT @limit;
            """;
        cmd.Parameters.AddWithValue("lng", lng);
        cmd.Parameters.AddWithValue("lat", lat);
        cmd.Parameters.AddWithValue("radius", radiusM);
        cmd.Parameters.AddWithValue("limit", limit);

        var results = new List<ReverseCandidate>();
        await using var reader = await cmd.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
        {
            results.Add(new ReverseCandidate(
                Name: reader.GetString(0),
                Kind: reader.GetString(1),
                Status: reader.GetString(2),
                DistanceM: reader.GetDouble(3),
                ElevationM: reader.IsDBNull(4) ? null : reader.GetDouble(4),
                KommuneName: reader.IsDBNull(5) ? null : reader.GetString(5),
                FylkeName: reader.IsDBNull(6) ? null : reader.GetString(6)));
        }
        return results;
    }
}
