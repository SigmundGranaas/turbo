using System.Text.Json;
using FluentAssertions;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// The Geonorge download flow: capabilities → POST /api/order → file URLs →
/// download. The request-build and response-parse are unit-tested against a
/// REAL captured order response (fixtures/geonorge-order-response.json); the
/// live order + large-file download is exercised in the M4 dry-run.
/// </summary>
public class GeonorgeClientTests
{
    [Fact]
    public void Parses_a_real_order_response_into_ready_file_urls()
    {
        var json = File.ReadAllText(FixturePath("geonorge-order-response.json"));

        var files = GeonorgeClient.ParseOrderResponse(json);

        files.Should().HaveCount(1);
        files[0].Name.Should().Be("Basisdata_03_Oslo_25833_Kommuner_GeoJSON.zip");
        files[0].DownloadUrl.Should().StartWith("https://nedlasting.geonorge.no/api/download/");
        files[0].Status.Should().Be("ReadyForDownload");
    }

    [Fact]
    public void Builds_the_order_body_the_API_accepts()
    {
        var json = GeonorgeClient.BuildOrderJson(
            "041f1e6e-bdbc-4091-b48f-8a5990f3cc5b",
            new GeonorgeArea("fylke", "Oslo", "03"),
            "GeoJSON",
            new GeonorgeProjection("25833", "EUREF89 UTM sone 33, 2d",
                "http://www.opengis.net/def/crs/EPSG/0/25833"));

        using var doc = JsonDocument.Parse(json);
        var line = doc.RootElement.GetProperty("orderLines")[0];
        line.GetProperty("metadataUuid").GetString().Should().Be("041f1e6e-bdbc-4091-b48f-8a5990f3cc5b");
        line.GetProperty("areas")[0].GetProperty("code").GetString().Should().Be("03");
        line.GetProperty("areas")[0].GetProperty("type").GetString().Should().Be("fylke");
        line.GetProperty("projections")[0].GetProperty("code").GetString().Should().Be("25833");
        line.GetProperty("formats")[0].GetProperty("name").GetString().Should().Be("GeoJSON");
    }

    private static string FixturePath(string name) =>
        Path.Combine(PlacesHostFixture.FindRepoRoot(),
            "apps", "api", "tests", "Turbo.Places.Behaviour", "fixtures", name);
}
