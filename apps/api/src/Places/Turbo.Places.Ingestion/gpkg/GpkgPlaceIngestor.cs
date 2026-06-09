using Turboapi.Places;
using Turboapi.Places.Infrastructure;

namespace Turboapi.Places.Ingestion;

/// <summary>Where the canonical fields live in a source GeoPackage's feature
/// table. The SSR/Matrikkel column names are known per dataset (confirmed in
/// the M4 dry-run against the real files).</summary>
public sealed record GpkgSourceSpec(
    string Source,
    string Table,
    string GeometryColumn,
    string IdColumn,
    string NameColumn,
    string TypeColumn);

/// <summary>
/// Streams a Geonorge GeoPackage into the staging table: read features
/// (GDAL-free) → reject unusable names (<see cref="Normalization"/>) →
/// reproject UTM33→WGS84 (<see cref="Utm33"/>) → canonical <see cref="Place"/>
/// → <c>StagePlacesAsync</c> in batches. The caller swaps when all sources are
/// staged. Reusing Normalization + the canonical Place keeps bulk rows
/// identical to the REST-sampling path.
/// </summary>
public sealed class GpkgPlaceIngestor
{
    private readonly GpkgReader _reader = new();

    public async Task<int> StageAsync(
        PgPlaceStore store, string gpkgPath, GpkgSourceSpec spec, string version,
        int batchSize = 5000, CancellationToken ct = default)
    {
        var attrs = new[] { spec.IdColumn, spec.NameColumn, spec.TypeColumn };
        var batch = new List<Place>(batchSize);
        var staged = 0;

        foreach (var feature in _reader.ReadFeatures(gpkgPath, spec.Table, spec.GeometryColumn, attrs))
        {
            var place = Map(feature, spec);
            if (place is null) continue;

            batch.Add(place);
            if (batch.Count >= batchSize)
            {
                staged += await store.StagePlacesAsync(batch, version, ct);
                batch.Clear();
            }
        }
        if (batch.Count > 0)
            staged += await store.StagePlacesAsync(batch, version, ct);

        return staged;
    }

    private static Place? Map(GpkgFeature feature, GpkgSourceSpec spec)
    {
        var name = feature.Attributes.GetValueOrDefault(spec.NameColumn);
        if (!Normalization.IsUsableName(name)) return null;

        var id = feature.Attributes.GetValueOrDefault(spec.IdColumn);
        if (string.IsNullOrWhiteSpace(id)) return null;

        var (lat, lng) = Utm33.ToWgs84(feature.Geometry.Coordinate.X, feature.Geometry.Coordinate.Y);
        var kind = feature.Attributes.GetValueOrDefault(spec.TypeColumn) ?? "";

        return new Place(spec.Source, id, kind, name!.Trim(), lat, lng, "aktiv");
    }
}
