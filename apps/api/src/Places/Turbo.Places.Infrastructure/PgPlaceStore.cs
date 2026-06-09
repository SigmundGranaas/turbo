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
                dataset_version text NOT NULL,
                updated_at      timestamptz NOT NULL DEFAULT now(),
                PRIMARY KEY (source, source_id)
            );
            CREATE INDEX IF NOT EXISTS places_centroid_gist ON places.places USING gist (centroid);
            CREATE INDEX IF NOT EXISTS places_geom_gist     ON places.places USING gist (geom);
            """;
        await cmd.ExecuteNonQueryAsync(ct);
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
                (source, source_id, feature_type, primary_name, name_fold, status, geom, centroid, dataset_version)
            VALUES
                (@source, @sid, @ft, @name, @fold, @status,
                 ST_SetSRID(ST_MakePoint(@lng, @lat), 4326),
                 ST_SetSRID(ST_MakePoint(@lng, @lat), 4326)::geography,
                 @ver)
            ON CONFLICT (source, source_id) DO UPDATE SET
                feature_type    = EXCLUDED.feature_type,
                primary_name    = EXCLUDED.primary_name,
                name_fold       = EXCLUDED.name_fold,
                status          = EXCLUDED.status,
                geom            = EXCLUDED.geom,
                centroid        = EXCLUDED.centroid,
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
                   ST_Distance(centroid, ST_SetSRID(ST_MakePoint(@lng, @lat), 4326)::geography) AS d
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
                DistanceM: reader.GetDouble(3)));
        }
        return results;
    }
}
