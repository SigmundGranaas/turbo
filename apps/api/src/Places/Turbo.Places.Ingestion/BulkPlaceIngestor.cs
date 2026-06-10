using System.IO.Compression;
using Turboapi.Places;
using Turboapi.Places.Infrastructure;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// End-to-end bulk ingestion for the SSR (Sentralt stedsnavnregister) place
/// dataset, which Geonorge ships GML-only: order → download zip → extract →
/// stream each GML through <see cref="GmlPlaceReader"/> (reproject 25833→WGS84)
/// → stage places in batches. The caller swaps once all sources are staged.
/// Network-bound, so it runs as a job (the reader + client parse are
/// unit-tested separately against real fixtures).
/// </summary>
public sealed class BulkPlaceIngestor
{
    private readonly GeonorgeClient _geonorge;
    private readonly GmlPlaceReader _reader = new();

    public BulkPlaceIngestor(GeonorgeClient geonorge) => _geonorge = geonorge;

    /// <summary>Order + download + stream a GML place dataset into staging.</summary>
    public async Task<int> StageAsync(
        PgPlaceStore store, string metadataUuid, GeonorgeArea area, GeonorgeProjection projection,
        string source, string version, string workDir, int batchSize = 5000,
        CancellationToken ct = default)
    {
        Directory.CreateDirectory(workDir);
        var files = await _geonorge.OrderAsync(metadataUuid, area, "GML", projection, ct);

        var staged = 0;
        foreach (var file in files.Where(f => f.Status == "ReadyForDownload"))
        {
            var zipPath = Path.Combine(workDir, file.Name);
            await _geonorge.DownloadToAsync(file.DownloadUrl, zipPath, ct);

            var extractDir = Path.Combine(workDir, Path.GetFileNameWithoutExtension(file.Name));
            if (Directory.Exists(extractDir)) Directory.Delete(extractDir, recursive: true);
            ZipFile.ExtractToDirectory(zipPath, extractDir);

            foreach (var gml in Directory.EnumerateFiles(extractDir, "*.gml", SearchOption.AllDirectories))
                staged += await StageFileAsync(store, gml, source, version, batchSize, ct);
        }
        return staged;
    }

    /// <summary>Stream one GML file into staging in bounded batches — never
    /// materialises the whole (national) file in memory.</summary>
    public async Task<int> StageFileAsync(
        PgPlaceStore store, string gmlPath, string source, string version,
        int batchSize = 5000, CancellationToken ct = default)
    {
        var batch = new List<Place>(batchSize);
        var staged = 0;
        foreach (var place in _reader.ReadPlaces(gmlPath, source))
        {
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
}
