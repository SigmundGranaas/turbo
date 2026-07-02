namespace Turboapi.Places.Core;

/// <summary>
/// Persistence + spatial query for the canonical places dataset. The slice
/// uses a Postgres/PostGIS implementation; the contract stays storage-agnostic
/// so the offline SQLite reader can satisfy it too.
/// </summary>
public interface IPlaceStore
{
    /// <summary>Create the schema, extensions, and indexes if absent (idempotent).</summary>
    Task EnsureSchemaAsync(CancellationToken ct = default);

    /// <summary>Upsert canonical places by (source, source_id).</summary>
    Task<int> UpsertAsync(IReadOnlyCollection<Place> places, string datasetVersion, CancellationToken ct = default);

    /// <summary>Upsert polygon areas (protected areas, kommuner) by
    /// (source, source_id).</summary>
    Task<int> UpsertAreasAsync(IReadOnlyCollection<Area> areas, string datasetVersion, CancellationToken ct = default);

    /// <summary>Nearest named features within <paramref name="radiusM"/> metres,
    /// closest first — the place-core toponym candidates.</summary>
    Task<IReadOnlyList<ReverseCandidate>> NearestAsync(
        double lat, double lng, double radiusM, int limit, CancellationToken ct = default);

    /// <summary>Point-containment over the areas table: smallest containing
    /// protected area + containing kommune/fylke.</summary>
    Task<Containment> ContainingAsync(double lat, double lng, CancellationToken ct = default);

    /// <summary>Top-N fuzzy name matches for <paramref name="query"/> (trigram +
    /// prefix), optionally distance-annotated against a map centre. Relevance
    /// retrieval only — final ordering is place-core's.</summary>
    Task<IReadOnlyList<SearchRow>> SearchAsync(
        string query, double? nearLat, double? nearLng, int limit, CancellationToken ct = default);

    /// <summary>Dataset stats for the health endpoint: row counts and the
    /// active publication version.</summary>
    Task<(long Places, long Areas, string? DatasetVersion)> StatsAsync(CancellationToken ct = default);

    /// <summary>The active publication version from <c>places.dataset</c>
    /// (a cheap 1-row read — the ETag source). <c>null</c> before first publish.</summary>
    Task<string?> GetActiveDatasetVersionAsync(CancellationToken ct = default);

    /// <summary>The active dataset's upstream freshness marker (Geonorge
    /// <c>DateUpdated</c>), or <c>null</c> if none recorded. The ingest compares
    /// it against the live upstream marker to skip an unchanged re-ingest before
    /// ordering or downloading.</summary>
    Task<string?> GetActiveSourceVersionAsync(CancellationToken ct = default);

    /// <summary>Mark <paramref name="version"/> the sole active publication
    /// (prior active → superseded), atomically, recording its
    /// <paramref name="sourceVersion"/> provenance. The ingestion swap's promote
    /// step; decoupled from row content so reads flip exactly once.</summary>
    Task PublishDatasetVersionAsync(
        string version, string? sourceVersion = null, CancellationToken ct = default);

    /// <summary>Open an ingest-run ledger row (<c>running</c>) for
    /// <paramref name="source"/> and return its id. The shared run-tracking shape
    /// (mirrors the tileserver's ingest_job): one queryable place for ingest
    /// history, status, and staleness.</summary>
    Task<Guid> BeginIngestRunAsync(string source, CancellationToken ct = default);

    /// <summary>Close the ingest-run row: final status, upstream marker, rows
    /// written, and an optional error message.</summary>
    Task CompleteIngestRunAsync(
        Guid runId, string status, string? sourceVersion, long rowsWritten, string? error,
        CancellationToken ct = default);

    /// <summary>Most-recent ingest runs, newest first (the /ingest/runs surface).</summary>
    Task<IReadOnlyList<IngestRun>> RecentIngestRunsAsync(int limit, CancellationToken ct = default);
}
