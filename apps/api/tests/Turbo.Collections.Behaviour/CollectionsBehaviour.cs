using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Collections.controller.request;
using Turboapi.Collections.controller.response;
using Xunit;

namespace Turbo.Collections.Behaviour;

[Collection("CollectionsHost")]
public sealed class CollectionsBehaviour
{
    private readonly CollectionsHostFixture _host;
    public CollectionsBehaviour(CollectionsHostFixture host) => _host = host;

    private static CreateCollectionRequest SampleCollection(string name = "Favorites") => new()
    {
        Name = name,
        Description = "Pinned items",
        ColorHex = "#FF6600",
        IconKey = "star",
        SortOrder = 0,
    };

    [Fact]
    public async Task creating_a_collection_makes_it_retrievable_by_its_owner()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);
        var request = SampleCollection();

        var create = await client.PostAsJsonAsync("/api/collections/Collections", request);
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        var fetched = await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/collections/Collections/{created.Id}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<CollectionResponse>()
                : null;
        }, description: $"GET /api/collections/Collections/{created.Id}");

        fetched.Id.Should().Be(created.Id);
        fetched.Name.Should().Be(request.Name);
        fetched.Items.Should().BeEmpty();
    }

    [Fact]
    public async Task adding_items_to_a_collection_surfaces_them_through_the_get_endpoint()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/collections/Collections", SampleCollection("My run"));
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;
        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/collections/Collections/{created.Id}");
            return r.IsSuccessStatusCode ? (object)created : null;
        });

        var markerUuid = Guid.NewGuid().ToString();
        var pathUuid = Guid.NewGuid().ToString();

        var addMarker = await client.PostAsJsonAsync(
            $"/api/collections/Collections/{created.Id}/items",
            new AddItemRequest { Type = "marker", Uuid = markerUuid });
        addMarker.StatusCode.Should().Be(HttpStatusCode.NoContent);

        var addPath = await client.PostAsJsonAsync(
            $"/api/collections/Collections/{created.Id}/items",
            new AddItemRequest { Type = "path", Uuid = pathUuid });
        addPath.StatusCode.Should().Be(HttpStatusCode.NoContent);

        var withItems = await Eventually.Returns<CollectionResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/collections/Collections/{created.Id}");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<CollectionResponse>();
            return body is { Items: { Count: 2 } } ? body : null;
        }, description: "GET surfaces both added items");

        withItems.Items.Should().Contain(i => i.Type == "marker" && i.Uuid == markerUuid);
        withItems.Items.Should().Contain(i => i.Type == "path" && i.Uuid == pathUuid);
    }

    [Fact]
    public async Task removing_an_item_drops_it_from_subsequent_reads()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/collections/Collections", SampleCollection());
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;
        var markerUuid = Guid.NewGuid().ToString();

        // Wait for the create to project before mutating items — the
        // add-item command handler reads from the read model and would
        // otherwise 404 against a freshly-created collection.
        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/collections/Collections/{created.Id}");
            return r.IsSuccessStatusCode ? (object)created : null;
        }, description: "create projects before add-item");

        var add = await client.PostAsJsonAsync(
            $"/api/collections/Collections/{created.Id}/items",
            new AddItemRequest { Type = "marker", Uuid = markerUuid });
        add.StatusCode.Should().Be(HttpStatusCode.NoContent);

        await Eventually.Returns<CollectionResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/collections/Collections/{created.Id}");
            if (!r.IsSuccessStatusCode) return null;
            var b = await r.Content.ReadFromJsonAsync<CollectionResponse>();
            return b is { Items: { Count: 1 } } ? b : null;
        }, timeout: TimeSpan.FromSeconds(20),
           description: "add eventually surfaces the item in the read model");

        var remove = await client.DeleteAsync(
            $"/api/collections/Collections/{created.Id}/items/marker/{markerUuid}");
        remove.StatusCode.Should().Be(HttpStatusCode.NoContent);

        await Eventually.Returns<CollectionResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/collections/Collections/{created.Id}");
            if (!r.IsSuccessStatusCode) return null;
            var b = await r.Content.ReadFromJsonAsync<CollectionResponse>();
            return b is { Items: { Count: 0 } } ? b : null;
        }, timeout: TimeSpan.FromSeconds(20),
           description: "remove eventually drops the item from the read model");
    }

    [Fact]
    public async Task deleting_a_collection_surfaces_a_tombstone_via_the_delta_endpoint()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/collections/Collections", SampleCollection("Going away"));
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;
        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/collections/Collections/{created.Id}");
            return r.IsSuccessStatusCode ? (object)created : null;
        });

        var delete = await client.DeleteAsync($"/api/collections/Collections/{created.Id}");
        delete.StatusCode.Should().Be(HttpStatusCode.NoContent);

        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/collections/Collections/{created.Id}");
            return r.StatusCode == HttpStatusCode.NotFound ? (object)"gone" : null;
        }, description: "GET after DELETE eventually returns 404");

        var delta = await Eventually.Returns<CollectionsDeltaResponse>(async () =>
        {
            var r = await client.GetAsync("/api/collections/Collections?since=1970-01-01T00:00:00Z");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<CollectionsDeltaResponse>();
            return body is { Deleted: { } d } && d.Any(t => t.Id == created.Id) ? body : null;
        }, description: "Delta surfaces the tombstone");

        delta.Deleted.Should().Contain(t => t.Id == created.Id);
    }

    [Fact]
    public async Task creating_a_collection_without_a_token_is_rejected()
    {
        var anonymous = _host.CreateClient();
        var response = await anonymous.PostAsJsonAsync("/api/collections/Collections", SampleCollection());
        response.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }

    [Fact]
    public async Task another_user_cannot_read_someone_elses_collection()
    {
        var owner = Guid.NewGuid();
        var stranger = Guid.NewGuid();
        var ownerClient = _host.CreateClientAs(owner);
        var strangerClient = _host.CreateClientAs(stranger);

        var create = await ownerClient.PostAsJsonAsync("/api/collections/Collections", SampleCollection("Private"));
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;
        await Eventually.Returns(async () =>
        {
            var r = await ownerClient.GetAsync($"/api/collections/Collections/{created.Id}");
            return r.IsSuccessStatusCode ? (object)"there" : null;
        });

        var strangerRead = await strangerClient.GetAsync($"/api/collections/Collections/{created.Id}");
        strangerRead.StatusCode.Should().Be(HttpStatusCode.NotFound,
            "ownership filter in the read handler should hide the row from non-owners as a 404, not 403, to avoid leaking existence");
    }
}

[CollectionDefinition("CollectionsHost")]
public sealed class CollectionsHostCollection : ICollectionFixture<CollectionsHostFixture> { }
