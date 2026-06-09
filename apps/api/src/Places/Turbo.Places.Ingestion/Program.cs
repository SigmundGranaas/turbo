using System.Globalization;
using Turboapi.Places;
using Turboapi.Places.Core;
using Turboapi.Places.Infrastructure;

// API-based sample ingestion (pre-M4): batch-download real areas from
// Kartverket's REST APIs, store canonical places + containment polygons in
// PostGIS, then verify reverse-geocode + search entirely from our own stack.
// Areas wider than the /punkt 5 km cap are tiled with a disc grid.
//
// Usage:
//   PLACES_DB=<conn> PLACE_CORE_LIB=<dir> dotnet run -- [preset ...|lat lng radiusM]
//   presets: galdhopiggen tromso reine saltfjellet besseggen | all  (default: galdhopiggen)

var presets = new Dictionary<string, (double Lat, double Lng, int RadiusM)>
{
    // Jotunheimen high mountains; 7 km exercises the disc-grid tiling.
    ["galdhopiggen"] = (61.6363, 8.3120, 7000),
    // Dense city + island + coast (By/Tettsted + Adressenavn density).
    ["tromso"] = (69.6492, 18.9553, 3000),
    // Lofoten fishing village: water/landform-heavy coastal names.
    ["reine"] = (67.9320, 13.0880, 5000),
    // Pure wilderness inside Saltfjellet–Svartisen — containment fallback.
    ["saltfjellet"] = (66.6500, 14.3500, 5000),
    // Besseggen ridge: hiking-trail terrain east of the Galdhøpiggen disc.
    ["besseggen"] = (61.5050, 8.7500, 5000),
};

var connectionString = Environment.GetEnvironmentVariable("PLACES_DB")
    ?? "Host=localhost;Port=55432;Database=places;Username=postgres;Password=places";

List<(string Name, double Lat, double Lng, int RadiusM)> runs = args switch
{
    ["all"] => presets.Select(p => (p.Key, p.Value.Lat, p.Value.Lng, p.Value.RadiusM)).ToList(),
    [var lat, var lng, var r] when double.TryParse(lat, NumberStyles.Float, CultureInfo.InvariantCulture, out var la)
        => [("custom", la,
            double.Parse(lng, CultureInfo.InvariantCulture),
            int.Parse(r, CultureInfo.InvariantCulture))],
    { Length: > 0 } => args.Select(a => presets.TryGetValue(a, out var p)
        ? (a, p.Lat, p.Lng, p.RadiusM)
        : throw new ArgumentException($"unknown preset '{a}'")).ToList(),
    _ => [("galdhopiggen", presets["galdhopiggen"].Lat, presets["galdhopiggen"].Lng, presets["galdhopiggen"].RadiusM)],
};

var store = new PgPlaceStore(connectionString);
await store.EnsureSchemaAsync();

using var http = new HttpClient();
http.DefaultRequestHeaders.UserAgent.ParseAdd("turbo-places-ingest/0.1 (+https://github.com/sigmundgranaas/turbo)");
var kartverket = new KartverketStedsnavnClient(http);
var enricher = new KartverketEnrichmentClient(http);
var naturbase = new NaturbaseClient(http);
var version = DateTime.UtcNow.ToString("yyyyMMddHHmmss", CultureInfo.InvariantCulture);

foreach (var (name, lat, lng, radius) in runs)
{
    Console.WriteLine($"== ingest {name} ({F(lat)}, {F(lng)}) r={radius}m ==");

    // Tile the area: /punkt caps at 5 km, so cover the bbox with a grid of
    // discs (7 km spacing fully covers square cells with 5 km discs).
    var centres = TileCentres(lat, lng, radius).ToList();
    var raw = new Dictionary<string, Place>(); // dedupe overlapping discs by source id
    foreach (var (cLat, cLng) in centres)
    {
        await foreach (var place in kartverket.DownloadAreaAsync(cLat, cLng, Math.Min(radius, KartverketStedsnavnClient.MaxRadiusM)))
            raw.TryAdd(place.SourceId, place);
    }
    Console.WriteLine($"   {raw.Count} unique names from {centres.Count} disc(s)");

    using var gate = new SemaphoreSlim(6);
    var kommuneNumbers = new System.Collections.Concurrent.ConcurrentDictionary<string, string?>();
    var batch = await Task.WhenAll(raw.Values.Select(async p =>
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
    await store.UpsertAsync(batch, version);

    var dLat = radius * 1.5 / 111_320.0;
    var dLng = dLat / Math.Cos(lat * Math.PI / 180.0);
    var areas = new List<Area>(
        await naturbase.DownloadAreasAsync(lng - dLng, lat - dLat, lng + dLng, lat + dLat));
    foreach (var (nummer, fylke) in kommuneNumbers)
    {
        if (await enricher.KommuneAreaAsync(nummer, fylke) is { } kommuneArea)
            areas.Add(kommuneArea);
    }
    await store.UpsertAreasAsync(areas, version);
    Console.WriteLine($"   enriched {batch.Length}; areas: " +
        $"{areas.Count(a => a.AreaType == "protected_area")} protected, " +
        $"{areas.Count(a => a.AreaType == "kommune")} kommune");
}

// ── Verification: reverse + search from our own stack, no Kartverket ────────
Console.WriteLine();
Console.WriteLine("== verify (owned data only) ==");
var reverse = new ReverseGeocodeService(store);
var search = new SearchService(store);
var ok = true;
foreach (var (name, lat, lng, _) in runs)
{
    ok &= await Demo(reverse, name.PadRight(12), lat, lng);
}

// Proximity disambiguation on real duplicate names: the same query near
// different map centres should surface the local feature first.
foreach (var (q, label, nLat, nLng) in new[]
{
    ("storvatnet", "near Tromsø   ", 69.6492, 18.9553),
    ("storvatnet", "near Lofoten  ", 67.9320, 13.0880),
    ("reine", "near Lofoten  ", 67.9320, 13.0880),
})
{
    var results = await search.SearchAsync(q, nLat, nLng, limit: 2);
    var rendered = string.Join("; ", results.Select(r =>
        $"{r.Title} ({F(r.Lat)}, {F(r.Lng)}) [{r.Description}]"));
    Console.WriteLine($"search \"{q}\" {label} -> {rendered}");
}
return ok ? 0 : 1;

static IEnumerable<(double Lat, double Lng)> TileCentres(double lat, double lng, int radiusM)
{
    const double spacingM = 7000; // 5 km discs fully cover 7 km grid cells
    if (radiusM <= KartverketStedsnavnClient.MaxRadiusM)
    {
        yield return (lat, lng);
        yield break;
    }
    var steps = (int)Math.Ceiling((double)radiusM / spacingM);
    var dLat = spacingM / 111_320.0;
    var dLng = dLat / Math.Cos(lat * Math.PI / 180.0);
    for (var i = -steps; i <= steps; i++)
    for (var j = -steps; j <= steps; j++)
        yield return (lat + i * dLat, lng + j * dLng);
}

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
    if (d.ElevationM is { } e) parts.Add($"{F(Math.Round(e))} m");
    var area = string.Join(", ", new[] { d.Kommune, d.Fylke }.Where(s => !string.IsNullOrEmpty(s)));
    if (!string.IsNullOrEmpty(area)) parts.Add(area);
    var subtitle = parts.Count > 0 ? " · " + string.Join(" · ", parts) : "";
    var dist = d.DistanceM is { } m ? $" ({F(Math.Round(m))} m)" : "";
    Console.WriteLine($"reverse @ {tag} -> \"{label}\"{subtitle}{dist}");
    return true;
}

static string F(double v) => v.ToString(CultureInfo.InvariantCulture);
