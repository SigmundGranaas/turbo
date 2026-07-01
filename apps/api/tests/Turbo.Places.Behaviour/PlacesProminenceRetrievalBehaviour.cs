using FluentAssertions;
using Testcontainers.PostgreSql;
using Turboapi.Places;
using Turboapi.Places.Infrastructure;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// The DB-side half of the prominence prior (the reranker half is covered by
/// place-core's Rust corpus). Retrieval caps the candidate set, so a prominent
/// feature must be *pulled into* that set ahead of a crowd of obscure same-prefix
/// toponyms — otherwise the native reranker never even sees it. This seeds 40
/// short "Berg" farms (which sort first by name length) plus one city "Bergen",
/// with a retrieval cap of 30: without the prominence ordering the city is cut,
/// with it the city leads. Uses an explicit bonus map (no place-core .so needed).
/// </summary>
public sealed class PlacesProminenceRetrievalBehaviour : IAsyncLifetime
{
    private readonly PostgreSqlContainer _postgres = new PostgreSqlBuilder()
        .WithImage("postgis/postgis:16-3.4")
        .WithDatabase("places")
        .Build();

    // A settlement gets a large head-start; a farm a small one (metres-equivalent).
    private static readonly Dictionary<string, double> KindBonus = new()
    {
        ["by"] = 30_000.0,
        ["gard"] = 6_000.0,
    };

    private PgPlaceStore _store = null!;

    public async Task InitializeAsync()
    {
        await _postgres.StartAsync();
        _store = new PgPlaceStore(_postgres.GetConnectionString(), KindBonus, defaultBonusMeters: 3_000.0);
        await _store.EnsureSchemaAsync();

        var places = new List<Place>();
        // 40 farms literally named "Berg" (name length 4 — they sort ahead of the
        // 6-char city on the fallback name-length tiebreak).
        for (var i = 0; i < 40; i++)
        {
            places.Add(new Place(
                Source: "test", SourceId: $"berg-{i}", FeatureType: "Gard",
                PrimaryName: "Berg", Lat: 60.0 + i * 0.01, Lng: 8.0 + i * 0.01,
                Status: "aktiv", ElevationM: null, KommuneName: "Ein", FylkeName: "Innlandet"));
        }
        // The city — beyond the 30-row cap by name length, only reachable via the
        // prominence bonus.
        places.Add(new Place(
            Source: "test", SourceId: "bergen-city", FeatureType: "By",
            PrimaryName: "Bergen", Lat: 60.39, Lng: 5.32,
            Status: "aktiv", ElevationM: null, KommuneName: "Bergen", FylkeName: "Vestland"));

        await _store.UpsertAsync(places, "test");
    }

    public Task DisposeAsync() => _postgres.DisposeAsync().AsTask();

    [Fact]
    public async Task Prominent_city_survives_the_retrieval_cap_over_many_short_toponyms()
    {
        // No map centre: prominence is the only signal that can rescue the city
        // from the 30-row cut. Retrieval limit 30 < the 40 seeded "Berg" farms.
        var rows = await _store.SearchAsync("berg", nearLat: null, nearLng: null, limit: 30);

        rows.Should().HaveCount(30, "retrieval is capped at the requested limit");
        rows.Select(r => r.Name).Should().Contain(
            "Bergen",
            "the prominence prior must pull the city into the candidate set ahead of "
            + "the shorter-named farms, or the reranker never sees it");
    }
}
