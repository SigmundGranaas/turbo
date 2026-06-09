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
var batch = await Task.WhenAll(raw.Select(async p =>
{
    await gate.WaitAsync();
    try
    {
        var elevation = await enricher.ElevationAsync(p.Lat, p.Lng);
        var (kommune, fylke) = await enricher.KommuneAsync(p.Lat, p.Lng);
        return p with { ElevationM = elevation, KommuneName = kommune, FylkeName = fylke };
    }
    finally { gate.Release(); }
}));

var upserted = await store.UpsertAsync(batch, version);
Console.WriteLine($"enriched + upserted {upserted}");

// Reverse-geocode the centre — from our own data, no Kartverket call.
var reverse = new ReverseGeocodeService(store);
var description = await reverse.DescribeAsync(lat, lng);

if (description is null)
{
    Console.WriteLine("reverse: (no result)");
    return 1;
}

var label = description.Qualifier switch
{
    "on" => $"On {description.Title}",
    "inArea" => $"In {description.Title}",
    "atPlace" => $"At {description.Title}",
    "closeTo" => $"Close to {description.Title}",
    "near" => $"Near {description.Title}",
    _ => description.Title,
};
var subtitleParts = new List<string>();
if (description.ElevationM is { } e)
    subtitleParts.Add($"{e.ToString("0", CultureInfo.InvariantCulture)} m");
var area = string.Join(", ", new[] { description.Kommune, description.Fylke }.Where(s => !string.IsNullOrEmpty(s)));
if (!string.IsNullOrEmpty(area)) subtitleParts.Add(area);
var subtitle = subtitleParts.Count > 0 ? " · " + string.Join(" · ", subtitleParts) : "";

Console.WriteLine($"reverse @ centre -> \"{label}\"{subtitle} " +
                  $"({description.DistanceM?.ToString("0", CultureInfo.InvariantCulture)} m) " +
                  $"[from our own stack]");
return 0;
