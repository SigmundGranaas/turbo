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

    /// <summary>Mark <paramref name="version"/> the sole active publication
    /// (prior active → superseded), atomically. The ingestion swap's promote
    /// step; decoupled from row content so reads flip exactly once.</summary>
    Task PublishDatasetVersionAsync(string version, CancellationToken ct = default);
}
