using System.Globalization;
using Turboapi.Places.Core;
using Turboapi.Places.Infrastructure;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// Drives the full Geonorge admin-kommuner pipeline end to end against the live
/// API, then reverse-geocodes a point in the loaded area — proof that the
/// national pipeline (order → download → read → reproject → stage → swap)
/// works on real data. Run: <c>dotnet run -- bulk-admin 03 Oslo</c>.
/// </summary>
public static class BulkAdminDemo
{
    private const string AdminKommunerUuid = "041f1e6e-bdbc-4091-b48f-8a5990f3cc5b";

    private static readonly GeonorgeProjection Utm33 =
        new("25833", "EUREF89 UTM sone 33, 2d", "http://www.opengis.net/def/crs/EPSG/0/25833");

    public static async Task<int> RunAsync(string connectionString, string fylkeCode, string fylkeName)
    {
        var store = new PgPlaceStore(connectionString);
        await store.EnsureSchemaAsync();

        using var http = new HttpClient();
        http.DefaultRequestHeaders.UserAgent.ParseAdd("turbo-places-ingest/0.1 (+https://github.com/sigmundgranaas/turbo)");
        var ingestor = new BulkAreaIngestor(new GeonorgeClient(http));

        var version = "bulk-admin-" + DateTime.UtcNow.ToString("yyyyMMddHHmmss", CultureInfo.InvariantCulture);
        var workDir = Path.Combine(Path.GetTempPath(), "turbo-bulk-" + Guid.NewGuid().ToString("n"));

        Console.WriteLine($"== bulk-admin: {fylkeName} ({fylkeCode}) kommuner from Geonorge ==");
        var spec = new GeoJsonAreaSpec("admin", "kommune", "kommunenummer", "kommunenavn");
        var area = new GeonorgeArea("fylke", fylkeName, fylkeCode);

        var staged = await ingestor.StageAsync(
            store, AdminKommunerUuid, area, Utm33, spec, fileNameContains: "Kommune", version, workDir);
        Console.WriteLine($"staged {staged} kommune polygon(s)");
        if (staged == 0)
        {
            Console.WriteLine("nothing staged — aborting");
            return 1;
        }

        await store.SwapAsync(version);
        Console.WriteLine($"swapped to {version} (active)");

        // Reverse central Oslo — no toponyms/parks loaded, so the kommune
        // polygon containment (bulk-loaded + reprojected) answers the fallback.
        var reverse = new ReverseGeocodeService(store);
        var d = await reverse.DescribeAsync(59.9139, 10.7522);
        Console.WriteLine(d is null
            ? "reverse @ central Oslo -> (no result)"
            : $"reverse @ central Oslo -> \"{d.Title}\" [from bulk-loaded admin data]");

        try { Directory.Delete(workDir, recursive: true); } catch { /* best effort */ }
        return d?.Title is { Length: > 0 } ? 0 : 1;
    }
}
