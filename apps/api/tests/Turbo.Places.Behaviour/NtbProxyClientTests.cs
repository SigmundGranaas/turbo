using System.Text.Json;
using FluentAssertions;
using Microsoft.Extensions.Options;
using Turboapi.Places.Core;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>Pure-logic tests for the Nasjonal Turbase proxy normalisation and
/// URL building — no host or upstream needed.</summary>
public class NtbProxyClientTests
{
    private static JsonElement Doc(string json) => JsonDocument.Parse(json).RootElement;

    [Fact]
    public void BuildListUri_injects_key_and_a_bbox_centred_near_query()
    {
        var client = new NasjonalTurbaseProxyClient(
            new HttpClient(),
            Options.Create(new TurbasenConfig { ApiKey = "secret", ApiVersion = "v3" }));

        var uri = client.BuildListUri("steder", 59.0, 10.0, 60.0, 11.0, 80);

        uri.Host.Should().Be("api.nasjonalturbase.no");
        uri.AbsolutePath.Should().Be("/v3/steder");
        var q = uri.Query.TrimStart('?').Split('&')
            .Select(p => p.Split('=', 2))
            .ToDictionary(p => p[0], p => Uri.UnescapeDataString(p[1]));
        q["api_key"].Should().Be("secret");

        using var near = JsonDocument.Parse(q["near"]);
        var coords = near.RootElement.GetProperty("$geometry").GetProperty("coordinates");
        coords[0].GetDouble().Should().BeApproximately(10.5, 1e-9); // lon
        coords[1].GetDouble().Should().BeApproximately(59.5, 1e-9); // lat
        near.RootElement.GetProperty("$maxDistance").GetDouble().Should().BePositive();
    }

    [Fact]
    public void PoiFromSted_with_Hytte_tag_is_a_cabin()
    {
        var poi = NasjonalTurbaseProxyClient.PoiFromSted(Doc("""
            { "_id": "abc", "navn": "Test cabin", "tags": ["Hytte"],
              "geojson": { "type": "Point", "coordinates": [10.0, 60.0] } }
            """));

        poi.Should().NotBeNull();
        poi!.Type.Should().Be("cabin");
        poi.Title.Should().Be("Test cabin");
        poi.Lat.Should().Be(60.0);
        poi.UtUrl.Should().Be("https://ut.no/hytte/abc");
    }

    [Fact]
    public void PoiFromSted_without_Hytte_tag_is_a_place()
    {
        var poi = NasjonalTurbaseProxyClient.PoiFromSted(Doc("""
            { "_id": "p1", "navn": "Viewpoint", "tags": ["Utsiktspunkt"],
              "geojson": { "type": "Point", "coordinates": [9.0, 61.0] } }
            """));

        poi!.Type.Should().Be("place");
    }

    [Fact]
    public void PoiFromSted_without_geometry_is_null()
    {
        NasjonalTurbaseProxyClient.PoiFromSted(Doc("""{ "_id": "x", "navn": "No geo" }"""))
            .Should().BeNull();
    }

    [Fact]
    public void PoiFromTur_is_a_trip_and_prefers_embedded_ut_link()
    {
        var poi = NasjonalTurbaseProxyClient.PoiFromTur(Doc("""
            { "_id": "t1", "navn": "A hike",
              "lenker": [ { "url": "https://ut.no/turforslag/999/a-hike" } ],
              "geojson": { "type": "Point", "coordinates": [10.0, 60.0] } }
            """));

        poi!.Type.Should().Be("trip");
        poi.UtUrl.Should().Be("https://ut.no/turforslag/999/a-hike");
    }

    [Fact]
    public void RouteFromTur_extracts_polyline_and_metadata()
    {
        var route = NasjonalTurbaseProxyClient.RouteFromTur(Doc("""
            { "_id": "t2", "navn": "Long hike", "distanse": 5400, "gradering": "Middels",
              "geojson": { "type": "LineString",
                           "coordinates": [ [10.0, 60.0], [10.1, 60.1] ] } }
            """));

        route.Points.Should().HaveCount(2);
        route.Points[0].Should().Equal(10.0, 60.0); // [lng, lat]
        route.DistanceMeters.Should().Be(5400);
        route.Grade.Should().Be("Middels");
    }

    [Fact]
    public void Documents_reads_bare_array_and_wrapped_envelopes()
    {
        NasjonalTurbaseProxyClient.Documents(Doc("""[ {"a":1}, {"b":2} ]""")).Should().HaveCount(2);
        NasjonalTurbaseProxyClient.Documents(Doc("""{ "documents": [ {"a":1} ] }""")).Should().HaveCount(1);
        NasjonalTurbaseProxyClient.Documents(Doc("""{ "nope": true }""")).Should().BeEmpty();
    }

    [Fact]
    public async Task Unconfigured_client_returns_empty()
    {
        var client = new NasjonalTurbaseProxyClient(
            new HttpClient(), Options.Create(new TurbasenConfig { ApiKey = "" }));

        client.IsConfigured.Should().BeFalse();
        (await client.FetchPoisAsync(59, 10, 60, 11)).Should().BeEmpty();
        (await client.FetchRouteAsync("anything")).Should().BeNull();
    }
}
