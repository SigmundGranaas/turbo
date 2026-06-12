using System.Text.Json;
using FluentAssertions;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P1: read Geonorge admin/Naturbase polygon GeoJSON (UTF-8 BOM, CRS 25833,
/// geometry wrapped in a GeometryCollection) into canonical <see cref="Area"/>s
/// with WGS84 geometry. Tested against a REAL extracted feature
/// (fixtures/geonorge-admin-kommune.geojson — Lørenskog kommune).
/// </summary>
public class GeoJsonAreaReaderTests
{
    [Fact]
    public void Reads_a_real_admin_GeoJSON_feature_and_reprojects_to_WGS84()
    {
        var path = Path.Combine(PlacesHostFixture.FindRepoRoot(),
            "apps", "api", "tests", "Turbo.Places.Behaviour", "fixtures", "geonorge-admin-kommune.geojson");
        var spec = new GeoJsonAreaSpec("admin", "kommune", "kommunenummer", "kommunenavn");

        var areas = new GeoJsonAreaReader().ReadAreas(path, spec).ToList();

        areas.Should().HaveCount(1);
        var a = areas[0];
        a.Source.Should().Be("admin");
        a.AreaType.Should().Be("kommune");
        a.Name.Should().Be("Lørenskog");
        a.SourceId.Should().Be("3222");

        // The output geometry must be WGS84 (the source was UTM33 25833).
        using var doc = JsonDocument.Parse(a.GeoJsonGeometry);
        doc.RootElement.GetProperty("type").GetString().Should().Be("MultiPolygon");
        var firstVertex = doc.RootElement.GetProperty("coordinates")[0][0][0];
        var lng = firstVertex[0].GetDouble();
        var lat = firstVertex[1].GetDouble();
        lng.Should().BeInRange(10.7, 11.1, "Lørenskog is ~10.95°E after reprojection");
        lat.Should().BeInRange(59.8, 60.1, "Lørenskog is ~59.93°N after reprojection");
    }
}
