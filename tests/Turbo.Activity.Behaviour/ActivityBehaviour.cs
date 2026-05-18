using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Activity.controller;
using Turboapi.Activity.domain;
using Xunit;

namespace Turbo.Activity.Behaviour;

[Collection("ActivityHost")]
public sealed class ActivityBehaviour
{
    private readonly ActivityHostFixture _host;
    public ActivityBehaviour(ActivityHostFixture host) => _host = host;

    private static ActivityController.CreateActivityRequest SampleRequest(string name = "Lunch run") =>
        new(new Position { Latitude = 59.913, Longitude = 10.752 }, name, "5km loop", "run");

    [Fact]
    public async Task creating_an_activity_makes_it_retrievable_by_its_owner()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);
        var request = SampleRequest();

        var create = await client.PostAsJsonAsync("/api/activity", request);
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = await create.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>();
        created.Should().NotBeNull();

        var fetched = await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{created!.ActivityId}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>()
                : null;
        }, description: $"GET /api/activity/{created!.ActivityId}");

        fetched.Id.Should().Be(created.ActivityId);
        fetched.OwnerId.Should().Be(owner);
        fetched.Name.Should().Be(request.Name);
        fetched.Description.Should().Be(request.Description);
        fetched.Icon.Should().Be(request.Icon);
    }

    [Fact]
    public async Task patching_an_activity_updates_what_subsequent_reads_return()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/activity", SampleRequest("Old name"));
        var created = await create.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>();
        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{created!.ActivityId}");
            return r.IsSuccessStatusCode ? (object)created : null;
        });

        var patch = new ActivityController.EditActivityRequest("New name", "New description", "bike");
        var patched = await client.PatchAsJsonAsync($"/api/activity/{created!.ActivityId}", patch);
        patched.StatusCode.Should().Be(HttpStatusCode.OK);

        var fetched = await Eventually.Returns<ActivityController.ActivityResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{created.ActivityId}");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>();
            return body is { Name: "New name" } ? body : null;
        }, description: "GET after PATCH reflects new name");

        fetched.Name.Should().Be("New name");
        fetched.Description.Should().Be("New description");
        fetched.Icon.Should().Be("bike");
    }

    [Fact]
    public async Task deleting_an_activity_makes_subsequent_reads_return_not_found()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/activity", SampleRequest());
        var created = await create.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>();
        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{created!.ActivityId}");
            return r.IsSuccessStatusCode ? (object)created : null;
        });

        var delete = await client.DeleteAsync($"/api/activity/{created!.ActivityId}");
        delete.StatusCode.Should().Be(HttpStatusCode.NoContent);

        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{created.ActivityId}");
            return r.StatusCode == HttpStatusCode.NotFound ? (object)"gone" : null;
        }, description: "GET after DELETE eventually returns 404");
    }

    [Fact]
    public async Task an_activity_owned_by_one_user_is_not_visible_to_another()
    {
        var owner = Guid.NewGuid();
        var intruder = Guid.NewGuid();
        var ownerClient = _host.CreateClientAs(owner);
        var intruderClient = _host.CreateClientAs(intruder);

        var create = await ownerClient.PostAsJsonAsync("/api/activity", SampleRequest());
        var created = await create.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>();
        await Eventually.Returns(async () =>
        {
            var r = await ownerClient.GetAsync($"/api/activity/{created!.ActivityId}");
            return r.IsSuccessStatusCode ? (object)created : null;
        });

        var intruderGet = await intruderClient.GetAsync($"/api/activity/{created!.ActivityId}");
        intruderGet.StatusCode.Should().Be(HttpStatusCode.NotFound);
    }

    [Fact]
    public async Task creating_an_activity_without_a_token_is_rejected()
    {
        var anonymous = _host.CreateClient();
        var response = await anonymous.PostAsJsonAsync("/api/activity", SampleRequest());
        response.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }
}

[CollectionDefinition("ActivityHost")]
public sealed class ActivityHostCollection : ICollectionFixture<ActivityHostFixture> { }
