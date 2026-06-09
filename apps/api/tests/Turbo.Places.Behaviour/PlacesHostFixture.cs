using System.Text.Json;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Testcontainers.PostgreSql;
using Turbo.Host.Places;
using Turboapi.Places;
using Turboapi.Places.Infrastructure;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// Places host over a Testcontainers PostGIS, seeded deterministically from
/// the committed real-data fixture (apps/api/src/Places/sample-data) plus two
/// synthetic containment polygons — zero network beyond the container pull.
/// Leaner than <c>TurboHostFixture</c> on purpose: Places has no NATS and no
/// auth, so neither container/JWT applies.
/// </summary>
public sealed class PlacesHostFixture : IAsyncLifetime
{
    /// <summary>A point inside <see cref="ParkName"/>'s synthetic polygon,
    /// ~15 km from every fixture toponym — exercises pure containment.</summary>
    public const double WildLat = 61.505, WildLng = 8.41;

    public const string ParkName = "Jotunheimen";
    public const string ParkKind = "Nasjonalpark";

    private readonly PostgreSqlContainer _postgres = new PostgreSqlBuilder()
        // Same image the dev stack uses; already present on CI runners that
        // ran the ingestion slice (TurboTestContainers tracks 17-3.5-alpine —
        // converge when Places moves onto the shared fixture).
        .WithImage("postgis/postgis:16-3.4")
        .WithDatabase("places")
        .Build();

    private WebApplicationFactory<PlacesHostProgram>? _factory;

    /// <summary>Total rows seeded from the sample-data fixtures (pinned by
    /// the health behaviour, so fixture growth can't silently skew it).</summary>
    public int SeededPlaces { get; private set; }

    public HttpClient CreateClient() => _factory!.CreateClient();

    public async Task InitializeAsync()
    {
        // place-core's cdylib must exist before the first P/Invoke. Resolve it
        // from the repo layout so `cargo build --features cabi` is the only
        // prerequisite, regardless of test working directory.
        var repoRoot = FindRepoRoot();
        var libDir = new[] { "release", "debug" }
            .Select(c => Path.Combine(repoRoot, "packages", "place-core", "target", c))
            .FirstOrDefault(d => File.Exists(Path.Combine(d, "libplace_core.so")))
            ?? throw new InvalidOperationException(
                "libplace_core.so not found — run `cargo build --features cabi` in packages/place-core first.");
        Environment.SetEnvironmentVariable("PLACE_CORE_LIB", libDir);

        await _postgres.StartAsync();

        _factory = new WebApplicationFactory<PlacesHostProgram>().WithWebHostBuilder(builder =>
        {
            builder.UseEnvironment("Test");
            builder.UseContentRoot(Path.Combine(repoRoot, "apps", "api", "hosts", "Turbo.Host.Places"));
            builder.UseSetting("ConnectionStrings:Places", _postgres.GetConnectionString());
        });

        // Force host startup (runs the schema initializer), then seed.
        _ = _factory.CreateClient();
        await SeedAsync(repoRoot, _postgres.GetConnectionString());
    }

    public async Task DisposeAsync()
    {
        _factory?.Dispose();
        await _postgres.DisposeAsync();
    }

    private async Task SeedAsync(string repoRoot, string connectionString)
    {
        var store = new PgPlaceStore(connectionString);

        // Real-data fixtures: Jotunheimen high mountains + Tromsø city core —
        // peak/water/farm cases and the dense urban (Adressenavn/Kirke) cases.
        foreach (var file in new[] { "galdhopiggen-ssr.json", "tromso-city-ssr.json" })
        {
            var fixturePath = Path.Combine(
                repoRoot, "apps", "api", "src", "Places", "sample-data", file);
            using var doc = JsonDocument.Parse(await File.ReadAllTextAsync(fixturePath));
            var places = doc.RootElement.EnumerateArray()
                .Select(r => new Place(
                    Source: r.GetProperty("source").GetString()!,
                    SourceId: r.GetProperty("source_id").GetString()!,
                    FeatureType: r.GetProperty("feature_type").GetString()!,
                    PrimaryName: r.GetProperty("primary_name").GetString()!,
                    Lat: r.GetProperty("lat").GetDouble(),
                    Lng: r.GetProperty("lng").GetDouble(),
                    Status: r.GetProperty("status").GetString()!,
                    ElevationM: r.TryGetProperty("elevation_m", out var e) && e.ValueKind == JsonValueKind.Number
                        ? e.GetDouble() : null,
                    KommuneName: r.TryGetProperty("kommune_name", out var k) ? k.GetString() : null,
                    FylkeName: r.TryGetProperty("fylke_name", out var f) ? f.GetString() : null))
                .ToList();
            SeededPlaces += places.Count;
            await store.UpsertAsync(places, "test-fixture");
        }

        // Synthetic containment polygons: a park square around the wilderness
        // point and a kommune square covering the whole sample area.
        await store.UpsertAreasAsync(new[]
        {
            new Area("test", "park-1", "protected_area", ParkName, ParkKind,
                Square(WildLng - 0.06, WildLat - 0.03, WildLng + 0.06, WildLat + 0.03)),
            new Area("test", "kommune-1", "kommune", "Lom", "Innlandet",
                Square(8.0, 61.4, 8.6, 61.8)),
        }, "test-fixture");
    }

    private static string Square(double minLng, double minLat, double maxLng, double maxLat) =>
        $$"""
        {"type":"Polygon","coordinates":[[
            [{{S(minLng)}},{{S(minLat)}}],[{{S(maxLng)}},{{S(minLat)}}],
            [{{S(maxLng)}},{{S(maxLat)}}],[{{S(minLng)}},{{S(maxLat)}}],
            [{{S(minLng)}},{{S(minLat)}}]]]}
        """;

    private static string S(double v) => v.ToString(System.Globalization.CultureInfo.InvariantCulture);

    internal static string FindRepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null)
        {
            if (Directory.Exists(Path.Combine(dir.FullName, "packages", "place-core")))
                return dir.FullName;
            dir = dir.Parent;
        }
        throw new InvalidOperationException("Repo root (containing packages/place-core) not found.");
    }
}
