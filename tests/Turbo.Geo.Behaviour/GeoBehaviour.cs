using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Geo.controller.request;
using Turboapi.Geo.controller.response;
using Xunit;

namespace Turbo.Geo.Behaviour;

[Collection("GeoHost")]
public sealed class GeoBehaviour
{
    private readonly GeoHostFixture _host;
    public GeoBehaviour(GeoHostFixture host) => _host = host;

    private static CreateLocationRequest SampleLocation(string name = "Home", double lon = 10.752, double lat = 59.913) =>
        new()
        {
            Geometry = new GeometryData { Longitude = lon, Latitude = lat },
            Display = new DisplayData { Name = name, Description = "Sample", Icon = "pin" }
        };

    [Fact]
    public async Task creating_a_location_makes_it_retrievable_by_its_owner()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);
        var request = SampleLocation();

        var create = await client.PostAsJsonAsync("/api/geo/Locations", request);
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = await create.Content.ReadFromJsonAsync<LocationResponse>();
        created.Should().NotBeNull();

        var fetched = await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/geo/Locations/{created!.Id}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<LocationResponse>()
                : null;
        }, description: $"GET /api/geo/Locations/{created!.Id}");

        fetched.Id.Should().Be(created.Id);
        fetched.Geometry.Longitude.Should().BeApproximately(request.Geometry.Longitude, 0.0001);
        fetched.Geometry.Latitude.Should().BeApproximately(request.Geometry.Latitude, 0.0001);
        fetched.Display.Name.Should().Be(request.Display.Name);
    }

    [Fact]
    public async Task updating_a_location_changes_what_subsequent_reads_return()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/geo/Locations", SampleLocation("Initial"));
        var created = await create.Content.ReadFromJsonAsync<LocationResponse>();
        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/geo/Locations/{created!.Id}");
            return r.IsSuccessStatusCode ? (object)created : null;
        });

        var update = new UpdateLocationRequest
        {
            Geometry = new GeometryData { Longitude = 12.5, Latitude = 41.9 },
            Display = new DisplayChangeset { Name = "Renamed" }
        };
        var put = await client.PutAsJsonAsync($"/api/geo/Locations/{created!.Id}", update);
        put.StatusCode.Should().Be(HttpStatusCode.OK);

        var fetched = await Eventually.Returns<LocationResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/geo/Locations/{created.Id}");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<LocationResponse>();
            return body is { Display.Name: "Renamed" } ? body : null;
        }, description: "GET after PUT reflects new coordinates and name");

        fetched.Geometry.Longitude.Should().BeApproximately(12.5, 0.0001);
        fetched.Geometry.Latitude.Should().BeApproximately(41.9, 0.0001);
        fetched.Display.Name.Should().Be("Renamed");
    }

    [Fact]
    public async Task deleting_a_location_makes_subsequent_reads_return_not_found()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/geo/Locations", SampleLocation());
        var created = await create.Content.ReadFromJsonAsync<LocationResponse>();
        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/geo/Locations/{created!.Id}");
            return r.IsSuccessStatusCode ? (object)created : null;
        });

        var delete = await client.DeleteAsync($"/api/geo/Locations/{created!.Id}");
        delete.StatusCode.Should().Be(HttpStatusCode.NoContent);

        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/geo/Locations/{created.Id}");
            return r.StatusCode == HttpStatusCode.NotFound ? (object)"gone" : null;
        }, description: "GET after DELETE eventually returns 404");
    }

    [Fact]
    public async Task an_extent_query_returns_only_locations_inside_the_box_owned_by_the_caller()
    {
        var owner = Guid.NewGuid();
        var stranger = Guid.NewGuid();
        var ownerClient = _host.CreateClientAs(owner);
        var strangerClient = _host.CreateClientAs(stranger);

        var inside = await ownerClient.PostAsJsonAsync("/api/geo/Locations",
            SampleLocation("Inside", lon: 10.0, lat: 60.0));
        var insideId = (await inside.Content.ReadFromJsonAsync<LocationResponse>())!.Id;

        var outside = await ownerClient.PostAsJsonAsync("/api/geo/Locations",
            SampleLocation("Outside", lon: 30.0, lat: 30.0));
        var outsideId = (await outside.Content.ReadFromJsonAsync<LocationResponse>())!.Id;

        var strangerInside = await strangerClient.PostAsJsonAsync("/api/geo/Locations",
            SampleLocation("Stranger inside", lon: 10.1, lat: 60.1));
        strangerInside.IsSuccessStatusCode.Should().BeTrue();

        var listed = await Eventually.Returns<LocationsResponse>(async () =>
        {
            var r = await ownerClient.GetAsync("/api/geo/Locations?minLon=9&minLat=59&maxLon=11&maxLat=61");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<LocationsResponse>();
            return body is { Items: not null } && body.Items.Any(i => i.Id == insideId) ? body : null;
        }, description: "extent query eventually surfaces the inside location");

        var ids = listed.Items.Select(i => i.Id).ToHashSet();
        ids.Should().Contain(insideId);
        ids.Should().NotContain(outsideId);
        ids.Should().NotContain((await strangerInside.Content.ReadFromJsonAsync<LocationResponse>())!.Id);
    }

    [Fact]
    public async Task creating_a_location_without_a_token_is_rejected()
    {
        var anonymous = _host.CreateClient();
        var response = await anonymous.PostAsJsonAsync("/api/geo/Locations", SampleLocation());
        response.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }
}

[CollectionDefinition("GeoHost")]
public sealed class GeoHostCollection : ICollectionFixture<GeoHostFixture> { }
