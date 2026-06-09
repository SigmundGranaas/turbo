using System.Globalization;
using Turboapi.Places;
using Turboapi.Places.Core;
using Turboapi.Places.Infrastructure;

// M1 vertical slice: batch-download a real sample area from Kartverket, store
// canonical places in PostGIS, then reverse-geocode a coordinate entirely from
// our own stack (place-core ranking, no Kartverket at query time).
//
// Usage:
//   PLACES_DB=<conn> PLACE_CORE_LIB=<dir-with-libplace_core.so> \
//     dotnet run --project Turbo.Places.Ingestion -- [centerLat centerLng radiusM]

var connectionString = Environment.GetEnvironmentVariable("PLACES_DB")
    ?? "Host=localhost;Port=55432;Database=places;Username=postgres;Password=places";

// Default sample area: Galdhøpiggen, 5 km radius.
var lat = args.Length > 0 ? double.Parse(args[0], CultureInfo.InvariantCulture) : 61.6363;
var lng = args.Length > 1 ? double.Parse(args[1], CultureInfo.InvariantCulture) : 8.3120;
var radius = args.Length > 2 ? int.Parse(args[2], CultureInfo.InvariantCulture) : 5000;

var version = DateTime.UtcNow.ToString("yyyyMMddHHmmss", CultureInfo.InvariantCulture);

Console.WriteLine($"== Turbo.Places M1 slice ==");
Console.WriteLine($"sample area: ({lat.ToString(CultureInfo.InvariantCulture)}, " +
                  $"{lng.ToString(CultureInfo.InvariantCulture)}) r={radius}m");

var store = new PgPlaceStore(connectionString);
await store.EnsureSchemaAsync();
Console.WriteLine("schema ready");

using var http = new HttpClient();
http.DefaultRequestHeaders.UserAgent.ParseAdd("turbo-places-ingest/0.1 (+https://github.com/sigmundgranaas/turbo)");
var kartverket = new KartverketStedsnavnClient(http);

var raw = new List<Place>();
await foreach (var place in kartverket.DownloadAreaAsync(lat, lng, radius))
{
    raw.Add(place);
}
Console.WriteLine($"downloaded {raw.Count} names from Kartverket");

// Precompute per-feature enrichment (elevation + kommune/fylke), bounded
// concurrency so we're a good API citizen.
var enricher = new KartverketEnrichmentClient(http);
using var gate = new SemaphoreSlim(6);
var kommuneNumbers = new System.Collections.Concurrent.ConcurrentDictionary<string, string?>();
var batch = await Task.WhenAll(raw.Select(async p =>
{
    await gate.WaitAsync();
    try
    {
        var elevation = await enricher.ElevationAsync(p.Lat, p.Lng);
        var (nummer, kommune, fylke) = await enricher.KommuneAsync(p.Lat, p.Lng);
        if (nummer is not null) kommuneNumbers.TryAdd(nummer, fylke);
        return p with { ElevationM = elevation, KommuneName = kommune, FylkeName = fylke };
    }
    finally { gate.Release(); }
}));

var upserted = await store.UpsertAsync(batch, version);
Console.WriteLine($"enriched + upserted {upserted}");

// Polygon areas for containment: protected areas (Naturbase) for a bbox
// around the sample disc, plus the boundary polygon of every kommune seen.
var dLat = radius * 3.0 / 111_320.0; // 3x the disc so park demo points fit
var dLng = dLat / Math.Cos(lat * Math.PI / 180.0);
var naturbase = new NaturbaseClient(http);
var areas = new List<Area>(
    await naturbase.DownloadAreasAsync(lng - dLng, lat - dLat, lng + dLng, lat + dLat));
foreach (var (nummer, fylke) in kommuneNumbers)
{
    if (await enricher.KommuneAreaAsync(nummer, fylke) is { } kommuneArea)
        areas.Add(kommuneArea);
}
var areaCount = await store.UpsertAreasAsync(areas, version);
Console.WriteLine($"areas upserted: {areaCount} " +
    $"({areas.Count(a => a.AreaType == "protected_area")} protected, " +
    $"{areas.Count(a => a.AreaType == "kommune")} kommune)");

// Reverse-geocode demo points — from our own data, no Kartverket call.
// The wilderness point sits inside a protected area but outside the
// ingested toponym disc, exercising the polygon-containment fallback.
var reverse = new ReverseGeocodeService(store);
var ok = await Demo(reverse, "centre    ", lat, lng);
ok &= await Demo(reverse, "wilderness", lat - 0.13, lng + 0.10);

// Forward search — same stack, place-core ordering + icons.
var search = new SearchService(store);
foreach (var q in new[] { "galdh", "tverr" })
{
    var results = await search.SearchAsync(q, lat, lng, limit: 3);
    var rendered = string.Join("; ", results.Select(r => $"{r.Title} [{r.Icon}] ({r.Description})"));
    Console.WriteLine($"search \"{q}\" -> {rendered}");
    ok &= results.Count > 0;
}
return ok ? 0 : 1;

static async Task<bool> Demo(ReverseGeocodeService reverse, string tag, double lat, double lng)
{
    var d = await reverse.DescribeAsync(lat, lng);
    if (d is null)
    {
        Console.WriteLine($"reverse @ {tag} -> (no result)");
        return false;
    }
    var label = d.Qualifier switch
    {
        "on" => $"On {d.Title}",
        "inArea" => $"In {d.Title}",
        "atPlace" => $"At {d.Title}",
        "closeTo" => $"Close to {d.Title}",
        "near" => $"Near {d.Title}",
        _ => d.Title,
    };
    var parts = new List<string>();
    if (d.Secondary is { Length: > 0 }) parts.Add(d.Secondary);
    if (d.ElevationM is { } e) parts.Add($"{e.ToString("0", CultureInfo.InvariantCulture)} m");
    var area = string.Join(", ", new[] { d.Kommune, d.Fylke }.Where(s => !string.IsNullOrEmpty(s)));
    if (!string.IsNullOrEmpty(area)) parts.Add(area);
    var subtitle = parts.Count > 0 ? " · " + string.Join(" · ", parts) : "";
    var dist = d.DistanceM is { } m ? $" ({m.ToString("0", CultureInfo.InvariantCulture)} m)" : "";
    Console.WriteLine($"reverse @ {tag} -> \"{label}\"{subtitle}{dist} [from our own stack]");
    return true;
}
