using System.Globalization;
using Turboapi.Places.Core;
using Turboapi.Places.Infrastructure;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// Drives the full SSR (Sentralt stedsnavnregister) bulk pipeline against the
/// live Geonorge API: order GML → download → extract → stream-read (reproject
/// 25833→WGS84) → stage → swap, then reverse + search from our own data. SSR is
/// GML-only, so this is the place counterpart to <see cref="BulkAdminDemo"/>.
/// Defaults to one fylke (Nordland) — the national file is large; pass an
/// explicit area to widen or narrow. Run: <c>dotnet run -- bulk-ssr fylke 18 Nordland</c>.
/// </summary>
public static class BulkSsrDemo
{
    // "Stedsnavn" (StedsnavnForVanligBruk) — GML + 25833 + fylke/landsdekkende.
    private const string StedsnavnUuid = "30caed2f-454e-44be-b5cc-26bb5c0110ca";

    private static readonly GeonorgeProjection Utm33 =
        new("25833", "EUREF89 UTM sone 33, 2d", "http://www.opengis.net/def/crs/EPSG/0/25833");

    public static async Task<int> RunAsync(
        string connectionString, string areaType, string areaCode, string areaName)
    {
        var store = new PgPlaceStore(connectionString);
        await store.EnsureSchemaAsync();

        using var http = new HttpClient { Timeout = TimeSpan.FromMinutes(10) };
        http.DefaultRequestHeaders.UserAgent.ParseAdd("turbo-places-ingest/0.1 (+https://github.com/sigmundgranaas/turbo)");
        var client = new GeonorgeClient(http);
        var ingestor = new BulkPlaceIngestor(client);

        // One ledger row per run (running → skipped_unchanged | success | failed):
        // the shared ingest-run tracking surfaced at /api/places/ingest/runs.
        var runId = await store.BeginIngestRunAsync("ssr");
        string? upstreamVersion = null;
        try
        {
            // Freshness pre-check: a small metadata GET yields SSR's upstream
            // DateUpdated. If it matches the active dataset's stored source_version
            // we skip the whole order + ~3 GB download — the point is to not pull
            // data from Kartverket that we already have. PLACES_INGEST_FORCE=1 forces.
            var force = Environment.GetEnvironmentVariable("PLACES_INGEST_FORCE") is "1" or "true";
            upstreamVersion = await client.GetDatasetVersionAsync(StedsnavnUuid);
            var activeVersion = await store.GetActiveSourceVersionAsync();
            if (!force && upstreamVersion is not null && upstreamVersion == activeVersion)
            {
                Console.WriteLine($"SSR unchanged (source_version={upstreamVersion}); skipping order + download.");
                await store.CompleteIngestRunAsync(runId, "skipped_unchanged", upstreamVersion, 0, null);
                return 0;
            }
            Console.WriteLine(upstreamVersion is null
                ? "upstream SSR version unavailable — ingesting (no freshness skip)"
                : $"SSR source_version {activeVersion ?? "(none)"} -> {upstreamVersion}; ingesting");

            var version = "bulk-ssr-" + DateTime.UtcNow.ToString("yyyyMMddHHmmss", CultureInfo.InvariantCulture);
            var workDir = Path.Combine(Path.GetTempPath(), "turbo-ssr-" + Guid.NewGuid().ToString("n"));

            Console.WriteLine($"== bulk-ssr: {areaName} ({areaType} {areaCode}) GML from Geonorge ==");
            var area = new GeonorgeArea(areaType, areaName, areaCode);

            var staged = await ingestor.StageAsync(store, StedsnavnUuid, area, Utm33, "ssr", version, workDir);
            Console.WriteLine($"staged {staged} place(s)");
            if (staged == 0)
            {
                Console.WriteLine("nothing staged — aborting");
                await store.CompleteIngestRunAsync(runId, "failed", upstreamVersion, 0, "nothing staged");
                return 1;
            }

            await store.SwapAsync(version, sourceVersion: upstreamVersion);
            Console.WriteLine($"swapped to {version} (active, source_version={upstreamVersion ?? "(none)"})");
            await store.CompleteIngestRunAsync(runId, "success", upstreamVersion, staged, null);

            // Sjunkhatten / Bodø — toponym density check on freshly bulk-loaded data.
            var reverse = new ReverseGeocodeService(store);
            var d = await reverse.DescribeAsync(67.2800, 14.4050);
            Console.WriteLine(d is null
                ? "reverse @ Bodø area -> (no result)"
                : $"reverse @ Bodø area -> \"{d.Title}\" ({d.Qualifier}) [bulk SSR]");

            try { Directory.Delete(workDir, recursive: true); } catch { /* best effort */ }
            return 0;
        }
        catch (Exception ex)
        {
            await store.CompleteIngestRunAsync(runId, "failed", upstreamVersion, 0, ex.Message);
            throw;
        }
    }
}
