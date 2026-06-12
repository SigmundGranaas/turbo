using FluentAssertions;
using Npgsql;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// SSR bulk end to end: the real Stedsnavn GML fixture → streaming GML read →
/// reproject/normalise → batched stage → atomic swap → queryable live data.
/// Pins that the GML path lands identical canonical rows to the GPKG/REST paths.
/// </summary>
public class GmlIngestEndToEndTests : IClassFixture<PlacesDbFixture>
{
    private readonly PlacesDbFixture _fixture;

    public GmlIngestEndToEndTests(PlacesDbFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task Streams_SSR_GML_into_live_via_stage_and_swap()
    {
        var path = Path.Combine(PlacesHostFixture.FindRepoRoot(),
            "apps", "api", "tests", "Turbo.Places.Behaviour", "fixtures", "ssr-stedsnavn.gml");

        var store = _fixture.Store;
        // Tiny batchSize exercises the multi-batch streaming path.
        var staged = await new BulkPlaceIngestor(new GeonorgeClient(new HttpClient()))
            .StageFileAsync(store, path, "ssr", "ssr-gml-v1", batchSize: 1);
        staged.Should().Be(2);

        await store.SwapAsync("ssr-gml-v1");
        (await store.GetActiveDatasetVersionAsync()).Should().Be("ssr-gml-v1");

        var rows = await LiveAsync();
        rows.Select(r => r.Name).Should().BeEquivalentTo("Rossnos", "Slædjokkelva");

        var rossnos = rows.Single(r => r.Name == "Rossnos");
        rossnos.Kind.Should().Be("topp");
        rossnos.Kommune.Should().Be("Ullensvang");
        rossnos.Lat.Should().BeApproximately(60.051153, 1e-5);
        rossnos.Lng.Should().BeApproximately(6.597936, 1e-5);
    }

    private async Task<List<(string Name, string Kind, string? Kommune, double Lat, double Lng)>> LiveAsync()
    {
        await using var conn = new NpgsqlConnection(_fixture.ConnectionString);
        await conn.OpenAsync();
        await using var cmd = conn.CreateCommand();
        cmd.CommandText =
            "SELECT primary_name, feature_type, kommune_name, ST_Y(geom), ST_X(geom) FROM places.places";
        var rows = new List<(string, string, string?, double, double)>();
        await using var r = await cmd.ExecuteReaderAsync();
        while (await r.ReadAsync())
            rows.Add((r.GetString(0), r.GetString(1), r.IsDBNull(2) ? null : r.GetString(2),
                r.GetDouble(3), r.GetDouble(4)));
        return rows;
    }
}
