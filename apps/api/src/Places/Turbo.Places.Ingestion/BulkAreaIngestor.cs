using System.IO.Compression;
using Turboapi.Places.Infrastructure;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// End-to-end bulk ingestion for a Geonorge polygon (GeoJSON) dataset: order →
/// download zip → extract → read matching files (reproject UTM33→WGS84) → stage
/// areas. The caller swaps once all sources are staged. Network-bound, so it's
/// run as a job / the M4 dry-run rather than in CI (the readers + client parse
/// are unit-tested separately).
/// </summary>
public sealed class BulkAreaIngestor
{
    private readonly GeonorgeClient _geonorge;
    private readonly GeoJsonAreaReader _reader = new();

    public BulkAreaIngestor(GeonorgeClient geonorge) => _geonorge = geonorge;

    /// <summary>Order + download + read a GeoJSON area dataset into staging.
    /// <paramref name="fileNameContains"/> selects the geojson inside the zip
    /// (e.g. "Kommune" to skip the "Grense" boundary-line file).</summary>
    public async Task<int> StageAsync(
        PgPlaceStore store, string metadataUuid, GeonorgeArea area, GeonorgeProjection projection,
        GeoJsonAreaSpec spec, string fileNameContains, string version, string workDir,
        CancellationToken ct = default)
    {
        Directory.CreateDirectory(workDir);
        var files = await _geonorge.OrderAsync(metadataUuid, area, "GeoJSON", projection, ct);

        var staged = 0;
        foreach (var file in files.Where(f => f.Status == "ReadyForDownload"))
        {
            var zipPath = Path.Combine(workDir, file.Name);
            await _geonorge.DownloadToAsync(file.DownloadUrl, zipPath, ct);

            var extractDir = Path.Combine(workDir, Path.GetFileNameWithoutExtension(file.Name));
            if (Directory.Exists(extractDir)) Directory.Delete(extractDir, recursive: true);
            ZipFile.ExtractToDirectory(zipPath, extractDir);

            foreach (var geojson in Directory.EnumerateFiles(extractDir, "*.geojson", SearchOption.AllDirectories)
                         .Where(p => Path.GetFileName(p).Contains(fileNameContains, StringComparison.OrdinalIgnoreCase)))
            {
                var areas = _reader.ReadAreas(geojson, spec).ToList();
                staged += await store.StageAreasAsync(areas, version, ct);
            }
        }
        return staged;
    }
}
