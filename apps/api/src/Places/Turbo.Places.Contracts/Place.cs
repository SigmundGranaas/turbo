namespace Turboapi.Places;

/// <summary>
/// One canonical reference feature, normalised from a source dataset (SSR,
/// Matrikkel, Naturbase). The physical model in <c>places.places</c> mirrors
/// these fields; see docs/architecture/2026-06-places-backend-plan.md §2.
/// </summary>
/// <param name="Source">Origin dataset, e.g. <c>ssr</c>.</param>
/// <param name="SourceId">Stable id within the source (e.g. SSR stedsnummer).</param>
/// <param name="FeatureType">Raw <c>navneobjekttype</c>; matched against the
/// place-core ruleset kind groups at query time (lower-cased there).</param>
/// <param name="Status">Lifecycle (<c>stedstatus</c>); non-<c>aktiv</c> is
/// penalised by place-core ranking.</param>
public sealed record Place(
    string Source,
    string SourceId,
    string FeatureType,
    string PrimaryName,
    double Lat,
    double Lng,
    string Status,
    double? ElevationM = null,
    string? KommuneName = null,
    string? FylkeName = null);

/// <summary>
/// A nearest-feature row returned by the reverse-geocode query — shaped to
/// become a place-core toponym candidate, carrying the precomputed enrichment
/// (elevation, containing kommune/fylke).
/// </summary>
public sealed record ReverseCandidate(
    string Name,
    string Kind,
    double DistanceM,
    string Status,
    double? ElevationM = null,
    string? KommuneName = null,
    string? FylkeName = null);
