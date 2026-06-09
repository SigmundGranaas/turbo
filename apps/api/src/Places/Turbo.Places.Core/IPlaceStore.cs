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
}
