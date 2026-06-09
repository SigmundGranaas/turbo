using System.Net;
using System.Net.Http.Json;
using System.Text.Json;
using FluentAssertions;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// End-to-end behaviour of /api/places over the real host + PostGIS, seeded
/// from the committed Galdhøpiggen sample. These pin the same invariants the
/// place-core golden fixture encodes, but through the full HTTP + SQL +
/// P/Invoke stack.
/// </summary>
public class PlacesApiBehaviour : IClassFixture<PlacesHostFixture>
{
    private readonly PlacesHostFixture _fixture;

    public PlacesApiBehaviour(PlacesHostFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task Reverse_on_the_summit_reads_On_Galdhopiggen_with_enrichment()
    {
        var client = _fixture.CreateClient();
        var d = await client.GetFromJsonAsync<JsonElement>(
            "/api/places/reverse?lat=61.6363&lon=8.3120");

        d.GetProperty("title").GetString().Should().Be("Galdhøpiggen");
        d.GetProperty("qualifier").GetString().Should().Be("on");
        d.GetProperty("kommune").GetString().Should().Be("Lom");
        d.GetProperty("fylke").GetString().Should().Be("Innlandet");
        d.GetProperty("elevationMeters").GetDouble().Should().BeApproximately(2468.25, 0.01);
        d.GetProperty("distanceMeters").GetDouble().Should().BeLessThan(100);
    }

    [Fact]
    public async Task Reverse_in_wilderness_falls_back_to_protected_area_containment()
    {
        var client = _fixture.CreateClient();
        var d = await client.GetFromJsonAsync<JsonElement>(
            $"/api/places/reverse?lat={PlacesHostFixture.WildLat}&lon={PlacesHostFixture.WildLng}");

        d.GetProperty("title").GetString().Should().Be(PlacesHostFixture.ParkName);
        d.GetProperty("qualifier").GetString().Should().Be("inArea");
        d.GetProperty("secondary").GetString().Should().Be(PlacesHostFixture.ParkKind);
        // Kommune containment enriches the park win.
        d.GetProperty("kommune").GetString().Should().Be("Lom");
    }

    [Fact]
    public async Task Reverse_in_a_city_resolves_the_tight_urban_feature()
    {
        var client = _fixture.CreateClient();
        var d = await client.GetFromJsonAsync<JsonElement>(
            "/api/places/reverse?lat=69.6492&lon=18.9553");

        // Dense-city case (real Tromsø data): the cathedral 70 m away wins the
        // title; kommune/fylke come from the row enrichment.
        d.GetProperty("title").GetString().Should().Be("Tromsø domkirke");
        d.GetProperty("qualifier").GetString().Should().Be("atPlace");
        d.GetProperty("kommune").GetString().Should().Be("Tromsø");
        d.GetProperty("fylke").GetString().Should().Be("Troms");
    }

    [Fact]
    public async Task Reverse_far_from_all_data_returns_404()
    {
        var client = _fixture.CreateClient();
        var resp = await client.GetAsync("/api/places/reverse?lat=70.0&lon=25.0");
        resp.StatusCode.Should().Be(HttpStatusCode.NotFound);
    }

    [Fact]
    public async Task Reverse_outside_Norway_returns_400()
    {
        var client = _fixture.CreateClient();
        var resp = await client.GetAsync("/api/places/reverse?lat=51.5&lon=-0.1");
        resp.StatusCode.Should().Be(HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task Search_prefix_returns_Galdhopiggen_first_with_mountain_icon()
    {
        var client = _fixture.CreateClient();
        var body = await client.GetFromJsonAsync<JsonElement>(
            "/api/places/search?q=galdh&lat=61.6363&lon=8.3120&limit=5");

        var items = body.GetProperty("items");
        items.GetArrayLength().Should().BeGreaterThan(0);
        var first = items[0];
        first.GetProperty("title").GetString().Should().Be("Galdhøpiggen");
        first.GetProperty("icon").GetString().Should().Be("mountain");
        first.GetProperty("lat").GetDouble().Should().BeApproximately(61.636, 0.01);
        first.GetProperty("description").GetString().Should().Contain("Lom");
    }

    [Fact]
    public async Task Search_without_query_returns_400()
    {
        var client = _fixture.CreateClient();
        var resp = await client.GetAsync("/api/places/search?q=");
        resp.StatusCode.Should().Be(HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task Reverse_carries_dataset_ETag_and_honours_IfNoneMatch_with_304()
    {
        var client = _fixture.CreateClient();
        const string url = "/api/places/reverse?lat=61.6363&lon=8.3120";

        var first = await client.GetAsync(url);
        first.StatusCode.Should().Be(HttpStatusCode.OK);
        var etag = first.Headers.ETag?.Tag;
        etag.Should().Be("\"test-fixture\"", "the ETag is the active dataset version");

        var request = new HttpRequestMessage(HttpMethod.Get, url);
        request.Headers.TryAddWithoutValidation("If-None-Match", etag);
        var second = await client.SendAsync(request);
        second.StatusCode.Should().Be(HttpStatusCode.NotModified);
    }

    [Fact]
    public async Task Health_reports_dataset_counts_and_version()
    {
        var client = _fixture.CreateClient();
        var h = await client.GetFromJsonAsync<JsonElement>("/api/places/health");

        h.GetProperty("places").GetInt64().Should().Be(_fixture.SeededPlaces);
        h.GetProperty("areas").GetInt64().Should().Be(2);
        h.GetProperty("datasetVersion").GetString().Should().Be("test-fixture");
    }
}
