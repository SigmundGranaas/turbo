using FluentAssertions;
using Npgsql;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P1a+P1b end to end: a Geonorge-shaped GeoPackage → GDAL-free read →
/// reproject → normalise → stage → atomic swap → queryable live data. Unusable
/// names are dropped; coordinates land in WGS84.
/// </summary>
public class GpkgIngestEndToEndTests : IClassFixture<PlacesDbFixture>
{
    private readonly PlacesDbFixture _fixture;

    public GpkgIngestEndToEndTests(PlacesDbFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task Ingests_a_GeoPackage_into_live_via_stage_and_swap()
    {
        var path = Path.Combine(Path.GetTempPath(), $"ingest-{Guid.NewGuid():n}.gpkg");
        try
        {
            // Galdhøpiggen + Tromsø in EPSG:25833, plus an "Ukjent" row to drop.
            TestGpkg.Write(path, "ssr_navn", ["ssr_id", "navn", "type"],
            [
                (TestGpkg.PointBlob(146001.63931684673, 6851889.415514315),
                    ["313058", "Galdhøpiggen", "Fjelltopp"]),
                (TestGpkg.PointBlob(653416.32548282, 7731676.0560329845),
                    ["770174", "Tromsø", "By"]),
                (TestGpkg.PointBlob(146050.0, 6851900.0),
                    ["999999", "Ukjent", "Fjell"]),
            ]);

            var store = _fixture.Store;
            var spec = new GpkgSourceSpec("ssr", "ssr_navn", "geom", "ssr_id", "navn", "type");

            var staged = await new GpkgPlaceIngestor().StageAsync(store, path, spec, "gpkg-v1");
            staged.Should().Be(2, "the Ukjent row is rejected");

            await store.SwapAsync("gpkg-v1");

            (await store.GetActiveDatasetVersionAsync()).Should().Be("gpkg-v1");

            var rows = await LiveAsync();
            rows.Should().HaveCount(2);
            rows.Select(r => r.Name).Should().BeEquivalentTo("Galdhøpiggen", "Tromsø");

            var g = rows.Single(r => r.Name == "Galdhøpiggen");
            g.Lat.Should().BeApproximately(61.63644, 1e-4);   // reprojected from UTM33
            g.Lng.Should().BeApproximately(8.31248, 1e-4);
            g.Kind.Should().Be("Fjelltopp");
            g.SourceId.Should().Be("313058");
        }
        finally { File.Delete(path); }
    }

    private async Task<List<(string Name, string Kind, string SourceId, double Lat, double Lng)>> LiveAsync()
    {
        await using var conn = new NpgsqlConnection(_fixture.ConnectionString);
        await conn.OpenAsync();
        await using var cmd = conn.CreateCommand();
        cmd.CommandText =
            "SELECT primary_name, feature_type, source_id, ST_Y(geom), ST_X(geom) FROM places.places";
        var rows = new List<(string, string, string, double, double)>();
        await using var r = await cmd.ExecuteReaderAsync();
        while (await r.ReadAsync())
            rows.Add((r.GetString(0), r.GetString(1), r.GetString(2), r.GetDouble(3), r.GetDouble(4)));
        return rows;
    }
}
