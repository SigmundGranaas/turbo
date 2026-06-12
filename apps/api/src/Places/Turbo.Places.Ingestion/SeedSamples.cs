using System.Globalization;
using System.Text.Json;
using Turboapi.Places.Core;
using Turboapi.Places.Infrastructure;

namespace Turboapi.Places.Ingestion;

/// <summary>
/// Offline dev seed: loads the committed real-data samples
/// (apps/api/src/Places/sample-data) plus a couple of synthetic containment
/// polygons, so `dotnet run -- seed-samples` gives a working /api/places with
/// zero network. The local dev flow uses this; production ingests for real.
/// </summary>
public static class SeedSamples
{
    public static async Task<int> RunAsync(string connectionString, string? samplesDir = null)
    {
        samplesDir ??= ResolveSamplesDir();
        Console.WriteLine($"seeding from {samplesDir}");

        var store = new PgPlaceStore(connectionString);
        await store.EnsureSchemaAsync();

        var places = new List<Place>();
        foreach (var file in new[] { "galdhopiggen-ssr.json", "tromso-city-ssr.json" })
        {
            var path = Path.Combine(samplesDir, file);
            if (!File.Exists(path)) continue;
            using var doc = JsonDocument.Parse(await File.ReadAllTextAsync(path));
            foreach (var r in doc.RootElement.EnumerateArray())
                places.Add(MapPlace(r));
        }
        await store.UpsertAsync(places, "samples");

        // Synthetic containment so the wilderness/kommune cascade works offline.
        var areas = new[]
        {
            new Area("sample", "park-1", "protected_area", "Jotunheimen", "Nasjonalpark",
                Square(8.35, 61.47, 8.47, 61.53)),
            new Area("sample", "kommune-1", "kommune", "Lom", "Innlandet",
                Square(8.0, 61.4, 8.6, 61.8)),
        };
        await store.UpsertAreasAsync(areas, "samples");
        await store.PublishDatasetVersionAsync("samples");

        Console.WriteLine($"seeded {places.Count} places + {areas.Length} areas (version 'samples')");

        // Best-effort smoke (needs PLACE_CORE_LIB; harmless if absent).
        try
        {
            var d = await new ReverseGeocodeService(store).DescribeAsync(61.6363, 8.3120);
            Console.WriteLine($"smoke reverse @ Galdhøpiggen -> {d?.Title} ({d?.Qualifier})");
        }
        catch (Exception ex)
        {
            Console.WriteLine($"(smoke reverse skipped: {ex.GetType().Name} — set PLACE_CORE_LIB to enable)");
        }

        return places.Count > 0 ? 0 : 1;
    }

    private static Place MapPlace(JsonElement r) => new(
        Source: r.GetProperty("source").GetString()!,
        SourceId: r.GetProperty("source_id").GetString()!,
        FeatureType: r.GetProperty("feature_type").GetString()!,
        PrimaryName: r.GetProperty("primary_name").GetString()!,
        Lat: r.GetProperty("lat").GetDouble(),
        Lng: r.GetProperty("lng").GetDouble(),
        Status: r.GetProperty("status").GetString()!,
        ElevationM: r.TryGetProperty("elevation_m", out var e) && e.ValueKind == JsonValueKind.Number ? e.GetDouble() : null,
        KommuneName: r.TryGetProperty("kommune_name", out var k) ? k.GetString() : null,
        FylkeName: r.TryGetProperty("fylke_name", out var f) ? f.GetString() : null);

    private static string Square(double minLng, double minLat, double maxLng, double maxLat)
    {
        string S(double v) => v.ToString(CultureInfo.InvariantCulture);
        return $"{{\"type\":\"Polygon\",\"coordinates\":[[" +
               $"[{S(minLng)},{S(minLat)}],[{S(maxLng)},{S(minLat)}]," +
               $"[{S(maxLng)},{S(maxLat)}],[{S(minLng)},{S(maxLat)}],[{S(minLng)},{S(minLat)}]]]}}";
    }

    /// <summary>Walk up from the running assembly to find the committed samples.</summary>
    private static string ResolveSamplesDir()
    {
        var env = Environment.GetEnvironmentVariable("PLACES_SAMPLES_DIR");
        if (!string.IsNullOrEmpty(env)) return env;

        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null)
        {
            var candidate = Path.Combine(dir.FullName, "apps", "api", "src", "Places", "sample-data");
            if (Directory.Exists(candidate)) return candidate;
            dir = dir.Parent;
        }
        // Fallback: a "sample-data" dir next to the binary (container layout).
        return Path.Combine(AppContext.BaseDirectory, "sample-data");
    }
}
