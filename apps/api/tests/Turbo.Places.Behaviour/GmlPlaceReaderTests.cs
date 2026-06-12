using System.Text;
using FluentAssertions;
using Turboapi.Places;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P-bulk: stream the SSR (Sentralt stedsnavnregister) GML into canonical
/// <see cref="Place"/>s. Tested against a REAL Stedsnavn WFS extract
/// (fixtures/ssr-stedsnavn.gml — one point "topp" and one multilingual
/// "bekk" centerline) so the reader is pinned to the live schema, not a guess.
/// </summary>
public class GmlPlaceReaderTests
{
    private static string FixturePath() => Path.Combine(PlacesHostFixture.FindRepoRoot(),
        "apps", "api", "tests", "Turbo.Places.Behaviour", "fixtures", "ssr-stedsnavn.gml");

    [Fact]
    public void Reads_a_real_point_feature_with_geographic_coordinates()
    {
        var places = new GmlPlaceReader().ReadPlaces(FixturePath()).ToList();

        var rossnos = places.Single(p => p.PrimaryName == "Rossnos");
        rossnos.Source.Should().Be("ssr");
        rossnos.SourceId.Should().Be("4382");
        rossnos.FeatureType.Should().Be("topp");
        rossnos.Status.Should().Be("aktiv", "SSR ForVanligBruk only ships active names");
        rossnos.KommuneName.Should().Be("Ullensvang");
        rossnos.FylkeName.Should().Be("Vestland");
        // EPSG:4258 posList is lat lon — no reprojection, just axis-correct read.
        rossnos.Lat.Should().BeApproximately(60.051153, 1e-6);
        rossnos.Lng.Should().BeApproximately(6.597936, 1e-6);
    }

    [Fact]
    public void Picks_the_primary_hovednavn_for_a_multilingual_centerline()
    {
        var places = new GmlPlaceReader().ReadPlaces(FixturePath()).ToList();

        // The bekk carries two hovednavn (norsk #1, nordsamisk #2); the lowest
        // stedsnavnnummer is the primary display name.
        var bekk = places.Single(p => p.FeatureType == "bekk");
        bekk.PrimaryName.Should().Be("Slædjokkelva");
        bekk.SourceId.Should().Be("19560");
        bekk.KommuneName.Should().Be("Gáivuotna - Kåfjord - Kaivuono");
        // A LineString collapses to its first vertex as the label point.
        bekk.Lat.Should().BeApproximately(69.653351, 1e-6);
        bekk.Lng.Should().BeApproximately(20.470564, 1e-6);
    }

    [Fact]
    public void Reads_exactly_the_usable_features()
    {
        new GmlPlaceReader().ReadPlaces(FixturePath()).Should().HaveCount(2);
    }

    [Fact]
    public void Reprojects_a_projected_UTM33_point_to_WGS84()
    {
        // The national download GML ships EPSG:25833 (easting northing); the WFS
        // ships 4258. The reader must branch on srsName. Galdhøpiggen in UTM33.
        const string gml = """
            <wfs:FeatureCollection
               xmlns:wfs="http://www.opengis.net/wfs/2.0"
               xmlns:gml="http://www.opengis.net/gml/3.2"
               xmlns:app="https://skjema.geonorge.no/SOSI/produktspesifikasjon/StedsnavnForVanligBruk/20231001">
              <wfs:member>
                <app:Sted gml:id="sted.1">
                  <app:posisjon>
                    <gml:Point srsName="urn:ogc:def:crs:EPSG::25833">
                      <gml:pos>146001.63931684673 6851889.415514315</gml:pos>
                    </gml:Point>
                  </app:posisjon>
                  <app:stedsnavn>
                    <app:Stedsnavn>
                      <app:navnestatus>hovednavn</app:navnestatus>
                      <app:stedsnavnnummer>1</app:stedsnavnnummer>
                      <app:skrivemåte>
                        <app:Skrivemåte><app:komplettskrivemåte>Galdhøpiggen</app:komplettskrivemåte></app:Skrivemåte>
                      </app:skrivemåte>
                    </app:Stedsnavn>
                  </app:stedsnavn>
                  <app:navneobjekttype>topp</app:navneobjekttype>
                  <app:stedsnummer>1</app:stedsnummer>
                </app:Sted>
              </wfs:member>
            </wfs:FeatureCollection>
            """;
        using var stream = new MemoryStream(Encoding.UTF8.GetBytes(gml));

        var p = new GmlPlaceReader().ReadPlaces(stream).Single();

        p.PrimaryName.Should().Be("Galdhøpiggen");
        p.Lat.Should().BeApproximately(61.63644, 0.001, "25833 easting/northing reproject to the summit");
        p.Lng.Should().BeApproximately(8.31248, 0.001);
    }

    [Fact]
    public void Resolves_srsName_from_the_MultiPoint_parent_in_the_download_GML()
    {
        // The national/fylke download wraps points in a gml:MultiPoint that
        // carries srsName=25833; the inner gml:Point omits it. The reader must
        // inherit the ancestor's CRS, else UTM eastings leak through as degrees.
        const string gml = """
            <gml:FeatureCollection
               xmlns:gml="http://www.opengis.net/gml/3.2"
               xmlns:app="https://skjema.geonorge.no/SOSI/produktspesifikasjon/StedsnavnForVanligBruk/20231001">
              <gml:featureMember>
                <app:Sted gml:id="id1">
                  <app:multipunkt>
                    <gml:MultiPoint srsName="urn:ogc:def:crs:EPSG::25833" srsDimension="2">
                      <gml:pointMember>
                        <gml:Point><gml:pos>146001.63931684673 6851889.415514315</gml:pos></gml:Point>
                      </gml:pointMember>
                    </gml:MultiPoint>
                  </app:multipunkt>
                  <app:stedsnavn>
                    <app:Stedsnavn>
                      <app:navnestatus>hovednavn</app:navnestatus>
                      <app:stedsnavnnummer>1</app:stedsnavnnummer>
                      <app:skrivemåte>
                        <app:Skrivemåte><app:komplettskrivemåte>Tåa</app:komplettskrivemåte></app:Skrivemåte>
                      </app:skrivemåte>
                    </app:Stedsnavn>
                  </app:stedsnavn>
                  <app:navneobjekttype>fjell</app:navneobjekttype>
                  <app:stedsnummer>42</app:stedsnummer>
                </app:Sted>
              </gml:featureMember>
            </gml:FeatureCollection>
            """;
        using var stream = new MemoryStream(Encoding.UTF8.GetBytes(gml));

        var p = new GmlPlaceReader().ReadPlaces(stream).Single();

        p.PrimaryName.Should().Be("Tåa");
        p.Lat.Should().BeApproximately(61.63644, 0.001, "the MultiPoint's 25833 srsName must be inherited");
        p.Lng.Should().BeApproximately(8.31248, 0.001);
    }

    [Fact]
    public void Skips_unusable_names()
    {
        const string gml = """
            <wfs:FeatureCollection
               xmlns:wfs="http://www.opengis.net/wfs/2.0"
               xmlns:gml="http://www.opengis.net/gml/3.2"
               xmlns:app="https://skjema.geonorge.no/SOSI/produktspesifikasjon/StedsnavnForVanligBruk/20231001">
              <wfs:member>
                <app:Sted gml:id="sted.1">
                  <app:posisjon>
                    <gml:Point srsName="urn:ogc:def:crs:EPSG::4258"><gml:pos>60.0 7.0</gml:pos></gml:Point>
                  </app:posisjon>
                  <app:stedsnavn>
                    <app:Stedsnavn>
                      <app:navnestatus>hovednavn</app:navnestatus>
                      <app:stedsnavnnummer>1</app:stedsnavnnummer>
                      <app:skrivemåte>
                        <app:Skrivemåte><app:komplettskrivemåte>Ukjent</app:komplettskrivemåte></app:Skrivemåte>
                      </app:skrivemåte>
                    </app:Stedsnavn>
                  </app:stedsnavn>
                  <app:navneobjekttype>topp</app:navneobjekttype>
                  <app:stedsnummer>1</app:stedsnummer>
                </app:Sted>
              </wfs:member>
            </wfs:FeatureCollection>
            """;
        using var stream = new MemoryStream(Encoding.UTF8.GetBytes(gml));

        new GmlPlaceReader().ReadPlaces(stream).Should().BeEmpty("placeholder names are rejected");
    }
}
