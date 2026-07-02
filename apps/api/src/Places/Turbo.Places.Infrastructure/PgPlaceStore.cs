using System.Text;
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
    /// <summary>Upper bound for the national atomic swap (see <see cref="SwapAsync"/>).</summary>
    private const int SwapCommandTimeoutSeconds = 600;

    /// <summary>Trigram threshold for the fuzzy fallback (see
    /// <see cref="FuzzySearchAsync"/>). Raised from pg_trgm's 0.3 default so the
    /// GIN index itself stays selective — at 0.3 a common stem returns ~40k index
    /// hits and similarity() is computed on thousands; 0.45 still admits a 1–2
    /// char typo (galdhopiggen→galdhøpiggen ≈ 0.47–0.63) while cutting that ~10×.</summary>
    private const double FuzzySimilarityThreshold = 0.45;

    private readonly string _connectionString;
    private readonly IReadOnlyDictionary<string, double> _kindBonusMeters;
    private readonly double _defaultBonusMeters;

    /// <param name="kindBonusMeters">Lowercased feature-type → metres-of-
    /// head-start prominence bonus, derived from the place-core ruleset. Blends
    /// into retrieval ordering so a prominent kind (a <c>by</c>) enters the
    /// candidate set ahead of obscure toponyms with the same prefix — the final
    /// rank is still place-core's. Null/empty disables the DB-side prior (ordering
    /// falls back to distance + name length), keeping existing tests unchanged.</param>
    public PgPlaceStore(
        string connectionString,
        IReadOnlyDictionary<string, double>? kindBonusMeters = null,
        double defaultBonusMeters = 0.0)
    {
        _connectionString = connectionString;
        _kindBonusMeters = kindBonusMeters ?? new Dictionary<string, double>();
        _defaultBonusMeters = defaultBonusMeters;
    }

    /// <summary>Diacritic-folded ASCII form (æ→ae, ø→o, å→a, ä→a, ö→o) for the
    /// fallback prefix arm. Kept byte-identical to the SQL <c>replace</c> chain in
    /// <see cref="EnsureSchemaAsync"/> that backfills <c>name_ascii</c>, so the
    /// stored column and the query fold agree.</summary>
    internal static string FoldAscii(string fold)
    {
        var sb = new StringBuilder(fold.Length);
        foreach (var ch in fold)
        {
            sb.Append(ch switch
            {
                'æ' => "ae",
                'ø' => "o",
                'å' => "a",
                'ä' => "a",
                'ö' => "o",
                _ => ch.ToString(),
            });
        }
        return sb.ToString();
    }

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
                name_ascii      text,
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
            -- Diacritic-folded ASCII name for the fallback prefix arm (an ASCII
            -- query "alesund" should still find "Ålesund"). Populated by ingest
            -- (PgPlaceStore.FoldAscii); deliberately NOT backfilled here — a
            -- ~1M-row UPDATE at boot would spike WAL/bloat and block the startup
            -- probe on the small node. The fallback is simply dormant for rows
            -- from before this deploy until the next ingest rewrites them; the
            -- diacritic-preserving prefix + trigram arms cover them meanwhile.
            ALTER TABLE places.places ADD COLUMN IF NOT EXISTS name_ascii   text;
            CREATE INDEX IF NOT EXISTS places_centroid_gist ON places.places USING gist (centroid);
            CREATE INDEX IF NOT EXISTS places_geom_gist     ON places.places USING gist (geom);
            CREATE INDEX IF NOT EXISTS places_name_trgm     ON places.places USING gin (name_fold gin_trgm_ops);
            -- Btree range scan for the autocomplete-dominant prefix path (a GIN
            -- trigram % scan over-matches common stems — e.g. "storvatn" hits
            -- thousands — so prefix gets its own cheap index; see SearchAsync).
            CREATE INDEX IF NOT EXISTS places_name_prefix
                ON places.places (name_fold text_pattern_ops);
            CREATE INDEX IF NOT EXISTS places_name_ascii_prefix
                ON places.places (name_ascii text_pattern_ops);
            -- Prominence prior for retrieval ordering: lowercased feature_type →
            -- metres-of-head-start. Populated from the place-core ruleset after
            -- this DDL (see EnsureSchemaAsync), so it is a single source of truth
            -- shared with the native reranker.
            CREATE TABLE IF NOT EXISTS places.kind_prominence (
                kind    text PRIMARY KEY,
                bonus_m double precision NOT NULL
            );
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
            CREATE TABLE IF NOT EXISTS places.dataset (
                version        text PRIMARY KEY,
                status         text NOT NULL,           -- 'active' | 'superseded'
                published_at   timestamptz NOT NULL DEFAULT now(),
                source_version text                     -- upstream freshness marker (Geonorge DateUpdated)
            );
            ALTER TABLE places.dataset ADD COLUMN IF NOT EXISTS source_version text;
            CREATE INDEX IF NOT EXISTS dataset_active ON places.dataset (status, published_at DESC);
            -- Staging mirrors live columns; only a unique index for ON CONFLICT
            -- (no GIST/GIN — they'd just slow the bulk load). A national dataset
            -- lands here, then SwapAsync replaces live atomically (sweeping
            -- features absent from the new version).
            CREATE TABLE IF NOT EXISTS places.places_staging (LIKE places.places INCLUDING DEFAULTS);
            -- LIKE only copies columns at creation time, so a staging table from
            -- an earlier schema won't have columns added to places.places since.
            -- Keep it in lock-step (the ingest inserts name_ascii; SwapAsync moves
            -- rows staging→live by explicit column list, name-matched).
            ALTER TABLE places.places_staging ADD COLUMN IF NOT EXISTS name_ascii   text;
            ALTER TABLE places.places_staging ADD COLUMN IF NOT EXISTS elevation_m  double precision;
            ALTER TABLE places.places_staging ADD COLUMN IF NOT EXISTS kommune_name text;
            ALTER TABLE places.places_staging ADD COLUMN IF NOT EXISTS fylke_name   text;
            CREATE UNIQUE INDEX IF NOT EXISTS places_staging_pk
                ON places.places_staging (source, source_id);
            CREATE TABLE IF NOT EXISTS places.areas_staging (LIKE places.areas INCLUDING DEFAULTS);
            CREATE UNIQUE INDEX IF NOT EXISTS areas_staging_pk
                ON places.areas_staging (source, source_id);
            """;
        await cmd.ExecuteNonQueryAsync(ct);

        // Repopulate the prominence lookup from the ruleset-derived map (a truncate
        // + reinsert so a ruleset change is picked up on the next boot). Tiny table
        // (~tens of rows); keyed by lowercased feature_type to match the join.
        await using var tx = await conn.BeginTransactionAsync(ct);
        await using (var clear = conn.CreateCommand())
        {
            clear.Transaction = tx;
            clear.CommandText = "TRUNCATE places.kind_prominence;";
            await clear.ExecuteNonQueryAsync(ct);
        }
        if (_kindBonusMeters.Count > 0)
        {
            await using var ins = conn.CreateCommand();
            ins.Transaction = tx;
            ins.CommandText = """
                INSERT INTO places.kind_prominence (kind, bonus_m) VALUES (@k, @b)
                ON CONFLICT (kind) DO UPDATE SET bonus_m = EXCLUDED.bonus_m;
                """;
            var pk = ins.Parameters.Add("k", NpgsqlTypes.NpgsqlDbType.Text);
            var pb = ins.Parameters.Add("b", NpgsqlTypes.NpgsqlDbType.Double);
            await ins.PrepareAsync(ct);
            foreach (var (kind, bonus) in _kindBonusMeters)
            {
                pk.Value = kind.ToLowerInvariant();
                pb.Value = bonus;
                await ins.ExecuteNonQueryAsync(ct);
            }
        }
        await tx.CommitAsync(ct);
    }

    /// <summary>
    /// Atomically promote a staged dataset version to live: replace all live
    /// places + areas from staging (sweeping features absent in the new
    /// version), clear staging, and mark the version active — all in one
    /// transaction. DELETE+INSERT (not TRUNCATE) so concurrent readers keep
    /// serving the prior version under MVCC, with no blocking and no partial
    /// reads, until the commit flips everything at once.
    /// </summary>
    public async Task SwapAsync(string version, string? sourceVersion = null, CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var tx = await conn.BeginTransactionAsync(ct);
        await using var cmd = conn.CreateCommand();
        // The atomic DELETE+INSERT moves the whole dataset (1M+ rows nationally),
        // which legitimately runs minutes — well past Npgsql's 30 s default. Give
        // it a generous bound rather than 0 (unlimited), so a genuinely stuck
        // swap still fails instead of hanging forever.
        cmd.CommandTimeout = SwapCommandTimeoutSeconds;
        // Explicit column lists (name-matched, not SELECT *): staging columns are
        // ALTER-appended over time, so positional SELECT * would misalign once
        // live and staging diverge in physical order.
        cmd.CommandText = """
            DELETE FROM places.places;
            INSERT INTO places.places
                (source, source_id, feature_type, primary_name, name_fold, name_ascii, status,
                 geom, centroid, elevation_m, kommune_name, fylke_name, dataset_version, updated_at)
            SELECT source, source_id, feature_type, primary_name, name_fold, name_ascii, status,
                   geom, centroid, elevation_m, kommune_name, fylke_name, dataset_version, updated_at
            FROM places.places_staging WHERE dataset_version = @v;
            DELETE FROM places.areas;
            INSERT INTO places.areas
                (source, source_id, area_type, name, kind, geom, dataset_version, updated_at)
            SELECT source, source_id, area_type, name, kind, geom, dataset_version, updated_at
            FROM places.areas_staging WHERE dataset_version = @v;
            DELETE FROM places.places_staging WHERE dataset_version = @v;
            DELETE FROM places.areas_staging  WHERE dataset_version = @v;
            UPDATE places.dataset SET status = 'superseded' WHERE status = 'active';
            INSERT INTO places.dataset (version, status, published_at, source_version)
            VALUES (@v, 'active', now(), @sv)
            ON CONFLICT (version) DO UPDATE SET
                status = 'active', published_at = now(), source_version = EXCLUDED.source_version;
            """;
        cmd.Parameters.AddWithValue("v", version);
        cmd.Parameters.AddWithValue("sv", (object?)sourceVersion ?? DBNull.Value);
        await cmd.ExecuteNonQueryAsync(ct);
        await tx.CommitAsync(ct);
    }

    public Task<int> UpsertAreasAsync(
        IReadOnlyCollection<Area> areas, string datasetVersion, CancellationToken ct = default)
        => UpsertAreasIntoAsync("places.areas", areas, datasetVersion, ct);

    /// <summary>Stage areas ahead of an atomic <see cref="SwapAsync"/>.</summary>
    public Task<int> StageAreasAsync(
        IReadOnlyCollection<Area> areas, string datasetVersion, CancellationToken ct = default)
        => UpsertAreasIntoAsync("places.areas_staging", areas, datasetVersion, ct);

    private async Task<int> UpsertAreasIntoAsync(
        string table, IReadOnlyCollection<Area> areas, string datasetVersion, CancellationToken ct)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var tx = await conn.BeginTransactionAsync(ct);

        await using var cmd = conn.CreateCommand();
        // ST_MakeValid: real-world coastline/boundary polygons routinely carry
        // self-intersections that would otherwise poison ST_Contains.
        cmd.CommandText = $"""
            INSERT INTO {table} (source, source_id, area_type, name, kind, geom, dataset_version)
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

    public async Task<IReadOnlyList<SearchRow>> SearchAsync(
        string query, double? nearLat, double? nearLng, int limit, CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);

        var fold = query.Trim().ToLowerInvariant();

        // Prefix-first retrieval. place-core ranks exact > prefix > substring, so
        // when there are already `limit` prefix matches no fuzzy match can reach
        // the shown results — the trigram % arm would be pure waste. Prefix is a
        // cheap btree range scan (vs. % scanning thousands of common-stem rows),
        // ordered by (distance − prominence) so "the Storvatnet near me" AND a
        // prominent city (a `by` beating an obscure toponym of the same prefix)
        // are among the slots even when there are thousands of prefix matches.
        var rows = await PrefixLikeSearchAsync(
            conn, "name_fold", EscapeLike(fold) + "%", excludeFoldPrefix: null,
            nearLat, nearLng, limit, ct);

        // Trigram fuzzy only to fill remaining slots (typo tolerance), with a
        // raised similarity threshold so a generic stem can't scan thousands.
        if (rows.Count < limit)
            rows.AddRange(await FuzzySearchAsync(conn, fold, nearLat, nearLng, limit - rows.Count, ct));

        // Diacritic-insensitive fallback: an ASCII query ("alesund") should still
        // find the diacritic name ("Ålesund"). name_fold deliberately keeps æ/ø/å
        // for precision, so this is a *secondary* prefix arm over name_ascii,
        // excluding rows the primary fold arm already returned. Only when slots
        // remain — the common all-latin query fills from the primary arm first.
        if (rows.Count < limit)
        {
            var asciiFold = FoldAscii(fold);
            rows.AddRange(await PrefixLikeSearchAsync(
                conn, "name_ascii", EscapeLike(asciiFold) + "%",
                excludeFoldPrefix: EscapeLike(fold) + "%",
                nearLat, nearLng, limit - rows.Count, ct));
        }

        rows.AddRange(await SearchAreasAsync(conn, fold, nearLat, nearLng, limit, ct));
        return rows;
    }

    /// <summary>Prefix match (btree <c>text_pattern_ops</c>) over
    /// <paramref name="column"/> (<c>name_fold</c> or <c>name_ascii</c>), bounded
    /// two ways so a broad prefix can't run a multi-second scan:
    /// <list type="bullet">
    /// <item>With a map centre — nearest-first via the GiST KNN operator
    /// (<c>centroid &lt;-&gt; point</c>), which walks the index in distance order
    /// and stops at <c>LIMIT</c>, rather than computing <c>ST_Distance</c> for
    /// every match and sorting (the old "stor @centre" tail). Proximity dominates
    /// with a centre; place-core still applies the prominence prior to the
    /// returned nearby set.</item>
    /// <item>Without a centre — ordered by prominence then shorter names, over a
    /// hard candidate cap so the sort is bounded even for a huge prefix. So a
    /// prominent city (a <c>by</c>) still beats obscure same-prefix toponyms.</item>
    /// </list>
    /// <paramref name="excludeFoldPrefix"/> (the ASCII arm) drops rows the fold arm
    /// already returned. Retrieval only — place-core re-ranks.</summary>
    private async Task<List<SearchRow>> PrefixLikeSearchAsync(
        NpgsqlConnection conn, string column, string likeValue, string? excludeFoldPrefix,
        double? nearLat, double? nearLng, int limit, CancellationToken ct)
    {
        var exclude = excludeFoldPrefix is null ? "" : "AND p.name_fold NOT LIKE @exclude";
        await using var cmd = conn.CreateCommand();
        if (nearLat.HasValue && nearLng.HasValue)
        {
            cmd.CommandText = $"""
                SELECT p.primary_name, p.feature_type,
                       ST_Y(p.geom) AS lat, ST_X(p.geom) AS lng,
                       p.kommune_name, p.fylke_name,
                       ST_Distance(p.centroid, ST_SetSRID(ST_MakePoint(@nlng, @nlat), 4326)::geography) AS d
                FROM places.places p
                WHERE p.{column} LIKE @like {exclude}
                ORDER BY p.centroid <-> ST_SetSRID(ST_MakePoint(@nlng, @nlat), 4326)::geography
                LIMIT @limit;
                """;
            cmd.Parameters.AddWithValue("nlat", nearLat.Value);
            cmd.Parameters.AddWithValue("nlng", nearLng.Value);
        }
        else
        {
            cmd.CommandText = $"""
                SELECT name, feature_type, lat, lng, kommune_name, fylke_name,
                       NULL::double precision AS d FROM (
                    SELECT p.primary_name AS name, p.feature_type,
                           ST_Y(p.geom) AS lat, ST_X(p.geom) AS lng,
                           p.kommune_name, p.fylke_name,
                           COALESCE(kp.bonus_m, @defBonus) AS bonus,
                           char_length(p.name_fold) AS nlen
                    FROM places.places p
                    LEFT JOIN places.kind_prominence kp ON kp.kind = lower(p.feature_type)
                    WHERE p.{column} LIKE @like {exclude}
                    LIMIT @cap
                ) t
                ORDER BY bonus DESC, nlen ASC
                LIMIT @limit;
                """;
            cmd.Parameters.AddWithValue("defBonus", _defaultBonusMeters);
            cmd.Parameters.AddWithValue("cap", NoCentreCandidateCap);
        }
        cmd.Parameters.AddWithValue("like", likeValue);
        if (excludeFoldPrefix is not null)
            cmd.Parameters.AddWithValue("exclude", excludeFoldPrefix);
        cmd.Parameters.AddWithValue("limit", limit);
        return await ReadPlaceRowsAsync(cmd, ct);
    }

    /// <summary>How many prefix matches the no-centre arm ranks by prominence. A
    /// hard cap bounds the sort for a broad prefix; well above the retrieval
    /// limit so it doesn't distort ordering for realistic prefixes.</summary>
    private const int NoCentreCandidateCap = 2000;

    /// <summary>Trigram-similarity fallback for typos. Raises the trigram
    /// threshold (SET LOCAL, scoped to a transaction so it can't leak onto the
    /// pooled connection) so the GIN index returns a tight candidate set instead
    /// of every loose match; distance is only a tiebreak.</summary>
    private async Task<List<SearchRow>> FuzzySearchAsync(
        NpgsqlConnection conn, string fold, double? nearLat, double? nearLng, int limit, CancellationToken ct)
    {
        await using var tx = await conn.BeginTransactionAsync(ct);
        await using (var set = conn.CreateCommand())
        {
            set.Transaction = tx;
            set.CommandText =
                $"SET LOCAL pg_trgm.similarity_threshold = {FuzzySimilarityThreshold.ToString(System.Globalization.CultureInfo.InvariantCulture)}";
            await set.ExecuteNonQueryAsync(ct);
        }

        await using var cmd = conn.CreateCommand();
        cmd.Transaction = tx;
        cmd.CommandText = """
            SELECT primary_name, feature_type,
                   ST_Y(geom) AS lat, ST_X(geom) AS lng,
                   kommune_name, fylke_name,
                   CASE WHEN @hasNear
                        THEN ST_Distance(centroid, ST_SetSRID(ST_MakePoint(@nlng, @nlat), 4326)::geography)
                   END AS d
            FROM places.places
            WHERE name_fold % @q AND name_fold NOT LIKE @prefix
            ORDER BY similarity(name_fold, @q) DESC, d ASC NULLS LAST
            LIMIT @limit;
            """;
        cmd.Parameters.AddWithValue("q", fold);
        cmd.Parameters.AddWithValue("prefix", EscapeLike(fold) + "%");
        cmd.Parameters.AddWithValue("hasNear", nearLat.HasValue && nearLng.HasValue);
        cmd.Parameters.AddWithValue("nlat", nearLat ?? 0.0);
        cmd.Parameters.AddWithValue("nlng", nearLng ?? 0.0);
        cmd.Parameters.AddWithValue("limit", limit);
        var rows = await ReadPlaceRowsAsync(cmd, ct);
        await tx.CommitAsync(ct);
        return rows;
    }

    private static async Task<List<SearchRow>> ReadPlaceRowsAsync(NpgsqlCommand cmd, CancellationToken ct)
    {
        var rows = new List<SearchRow>();
        await using var reader = await cmd.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
        {
            rows.Add(new SearchRow(
                Name: reader.GetString(0),
                Kind: reader.GetString(1),
                Lat: reader.GetDouble(2),
                Lng: reader.GetDouble(3),
                KommuneName: reader.IsDBNull(4) ? null : reader.GetString(4),
                FylkeName: reader.IsDBNull(5) ? null : reader.GetString(5),
                DistanceM: reader.IsDBNull(6) ? null : reader.GetDouble(6)));
        }
        return rows;
    }

    /// <summary>Name matches over the (small) areas table so parks and kommuner
    /// are searchable, not just toponyms. Result kind/description are shaped for
    /// the icon + subtitle: protected areas carry their verneform; kommuner read
    /// "Kommune" + fylke.</summary>
    private static async Task<List<SearchRow>> SearchAreasAsync(
        NpgsqlConnection conn, string fold, double? nearLat, double? nearLng, int limit, CancellationToken ct)
    {
        await using var cmd = conn.CreateCommand();
        cmd.CommandText = """
            SELECT name,
                   CASE WHEN area_type = 'kommune' THEN 'Kommune' ELSE COALESCE(kind, area_type) END AS kind,
                   ST_Y(ST_PointOnSurface(geom)) AS lat, ST_X(ST_PointOnSurface(geom)) AS lng,
                   CASE WHEN area_type = 'kommune' THEN kind END AS fylke,
                   CASE WHEN @hasNear
                        THEN ST_Distance(geom::geography, ST_SetSRID(ST_MakePoint(@nlng, @nlat), 4326)::geography)
                   END AS d
            FROM places.areas
            WHERE lower(name) % @q OR lower(name) LIKE @prefix
            ORDER BY GREATEST(similarity(lower(name), @q),
                              CASE WHEN lower(name) LIKE @prefix THEN 0.95 ELSE 0 END) DESC,
                     d ASC NULLS LAST
            LIMIT @limit;
            """;
        cmd.Parameters.AddWithValue("q", fold);
        cmd.Parameters.AddWithValue("prefix", EscapeLike(fold) + "%");
        cmd.Parameters.AddWithValue("hasNear", nearLat.HasValue && nearLng.HasValue);
        cmd.Parameters.AddWithValue("nlat", nearLat ?? 0.0);
        cmd.Parameters.AddWithValue("nlng", nearLng ?? 0.0);
        cmd.Parameters.AddWithValue("limit", limit);

        var rows = new List<SearchRow>();
        await using var reader = await cmd.ExecuteReaderAsync(ct);
        while (await reader.ReadAsync(ct))
        {
            rows.Add(new SearchRow(
                Name: reader.GetString(0),
                Kind: reader.GetString(1),
                Lat: reader.GetDouble(2),
                Lng: reader.GetDouble(3),
                KommuneName: null,
                FylkeName: reader.IsDBNull(4) ? null : reader.GetString(4),
                DistanceM: reader.IsDBNull(5) ? null : reader.GetDouble(5)));
        }
        return rows;
    }

    public async Task<(long Places, long Areas, string? DatasetVersion)> StatsAsync(
        CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var cmd = conn.CreateCommand();
        // Counts are real (ops endpoint, low traffic); the version is the
        // authoritative active publication from places.dataset, NOT a scan of
        // row versions — so /health and the ETag always agree.
        cmd.CommandText = """
            SELECT (SELECT count(*) FROM places.places),
                   (SELECT count(*) FROM places.areas),
                   (SELECT version FROM places.dataset WHERE status = 'active'
                    ORDER BY published_at DESC LIMIT 1);
            """;
        await using var reader = await cmd.ExecuteReaderAsync(ct);
        await reader.ReadAsync(ct);
        return (reader.GetInt64(0), reader.GetInt64(1),
                reader.IsDBNull(2) ? null : reader.GetString(2));
    }

    public async Task<string?> GetActiveDatasetVersionAsync(CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var cmd = conn.CreateCommand();
        cmd.CommandText = """
            SELECT version FROM places.dataset WHERE status = 'active'
            ORDER BY published_at DESC LIMIT 1;
            """;
        return await cmd.ExecuteScalarAsync(ct) as string;
    }

    /// <summary>The upstream freshness marker of the active dataset (Geonorge
    /// <c>DateUpdated</c>), or null if none is published/recorded. The ingest
    /// compares this against the live upstream marker to skip an unchanged
    /// re-ingest before ordering or downloading anything.</summary>
    public async Task<string?> GetActiveSourceVersionAsync(CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var cmd = conn.CreateCommand();
        cmd.CommandText = """
            SELECT source_version FROM places.dataset WHERE status = 'active'
            ORDER BY published_at DESC LIMIT 1;
            """;
        return await cmd.ExecuteScalarAsync(ct) as string;
    }

    public async Task PublishDatasetVersionAsync(
        string version, string? sourceVersion = null, CancellationToken ct = default)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var tx = await conn.BeginTransactionAsync(ct);
        await using var cmd = conn.CreateCommand();
        cmd.CommandText = """
            UPDATE places.dataset SET status = 'superseded' WHERE status = 'active';
            INSERT INTO places.dataset (version, status, published_at, source_version)
            VALUES (@v, 'active', now(), @sv)
            ON CONFLICT (version) DO UPDATE SET
                status = 'active', published_at = now(), source_version = EXCLUDED.source_version;
            """;
        cmd.Parameters.AddWithValue("v", version);
        cmd.Parameters.AddWithValue("sv", (object?)sourceVersion ?? DBNull.Value);
        await cmd.ExecuteNonQueryAsync(ct);
        await tx.CommitAsync(ct);
    }

    private static string EscapeLike(string s) =>
        s.Replace(@"\", @"\\").Replace("%", @"\%").Replace("_", @"\_");

    public Task<int> UpsertAsync(
        IReadOnlyCollection<Place> places, string datasetVersion, CancellationToken ct = default)
        => UpsertPlacesIntoAsync("places.places", places, datasetVersion, ct);

    /// <summary>Load places into the staging table (idempotent upsert) ahead of
    /// an atomic <see cref="SwapAsync"/>. Re-running is safe (resume).</summary>
    public Task<int> StagePlacesAsync(
        IReadOnlyCollection<Place> places, string datasetVersion, CancellationToken ct = default)
        => UpsertPlacesIntoAsync("places.places_staging", places, datasetVersion, ct);

    private async Task<int> UpsertPlacesIntoAsync(
        string table, IReadOnlyCollection<Place> places, string datasetVersion, CancellationToken ct)
    {
        await using var conn = new NpgsqlConnection(_connectionString);
        await conn.OpenAsync(ct);
        await using var tx = await conn.BeginTransactionAsync(ct);

        await using var cmd = conn.CreateCommand();
        cmd.CommandText = $"""
            INSERT INTO {table}
                (source, source_id, feature_type, primary_name, name_fold, name_ascii, status, geom, centroid,
                 elevation_m, kommune_name, fylke_name, dataset_version)
            VALUES
                (@source, @sid, @ft, @name, @fold, @ascii, @status,
                 ST_SetSRID(ST_MakePoint(@lng, @lat), 4326),
                 ST_SetSRID(ST_MakePoint(@lng, @lat), 4326)::geography,
                 @elev, @kommune, @fylke, @ver)
            ON CONFLICT (source, source_id) DO UPDATE SET
                feature_type    = EXCLUDED.feature_type,
                primary_name    = EXCLUDED.primary_name,
                name_fold       = EXCLUDED.name_fold,
                name_ascii      = EXCLUDED.name_ascii,
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
        var pAscii = cmd.Parameters.Add("ascii", NpgsqlTypes.NpgsqlDbType.Text);
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
            var fold = p.PrimaryName.ToLowerInvariant();
            pFold.Value = fold;
            pAscii.Value = FoldAscii(fold);
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
