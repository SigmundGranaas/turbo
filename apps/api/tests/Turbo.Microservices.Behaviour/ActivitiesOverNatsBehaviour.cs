using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Activities.BackcountrySki.controller;
using Turboapi.Activities.BackcountrySki.controller.request;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.controller;
using Turboapi.Activities.Fishing.controller;
using Turboapi.Activities.Fishing.controller.request;
using Turboapi.Activities.Fishing.value;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Xunit;

namespace Turbo.Microservices.Behaviour;

/// <summary>
/// Verifies the activities pipeline against the microservice deploy:
/// Auth + Activities are separate processes; events cross the wire over
/// NATS JetStream; the per-kind subscribers register durable consumers
/// on the activities host and project the cross-kind summary read model.
///
/// Mirrors the modulith behaviour suite — the same user-visible contract
/// must hold under both deploy shapes. If a NATS subscriber wiring bug
/// breaks the projection, this is where it shows up first.
/// </summary>
[Collection("MicroservicesTopology")]
public sealed class ActivitiesOverNatsBehaviour
{
    private readonly MicroservicesTopologyFixture _topology;
    public ActivitiesOverNatsBehaviour(MicroservicesTopologyFixture topology) => _topology = topology;

    [Fact]
    public async Task an_auth_host_token_authorizes_calls_to_the_activities_host()
    {
        var client = await RegisterAndAuthorizeActivitiesAsync();

        // Even reading the kind catalog requires auth — confirms the JWT
        // crosses host boundaries.
        var kinds = await client.GetAsync("/api/activities/kinds");
        kinds.StatusCode.Should().Be(HttpStatusCode.OK,
            $"Activities host rejected the Auth-issued JWT: {await kinds.Content.ReadAsStringAsync()}");
    }

    [Fact]
    public async Task creating_a_fishing_activity_lands_in_the_bbox_query_over_nats()
    {
        var client = await RegisterAndAuthorizeActivitiesAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "NATS pond",
            Longitude = 10.21,
            Latitude = 60.42,
            Details = new FishingDetailsDto
            {
                WaterKind = WaterKind.Lake,
                ShoreOrBoat = ShoreOrBoat.Shore,
            },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created,
            $"create failed: {await create.Content.ReadAsStringAsync()}");
        var created = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!;

        // The outbox dispatcher publishes ActivitySummaryUpserted on NATS;
        // the per-kind NATS subscriber on the same host consumes it and
        // projects into the cross-kind summaries table. Eventually polls
        // the read-side bbox endpoint until that projection lands.
        var summary = await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=10.1&minLat=60.3&maxLon=10.3&maxLat=60.5");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == created.Id) == true ? body : null;
        }, description: "summary projection arrives over NATS");

        var item = summary.Items.Single(i => i.Id == created.Id);
        item.Kind.Should().Be("fishing");
        item.Name.Should().Be("NATS pond");
        item.GeometryKind.Should().Be("Point");
    }

    [Fact]
    public async Task creating_a_backcountry_ski_route_projects_a_linestring_summary_over_nats()
    {
        var client = await RegisterAndAuthorizeActivitiesAsync();

        var create = await client.PostAsJsonAsync(
            "/api/activities/backcountry-ski",
            new CreateBackcountrySkiActivityRequest
            {
                Name = "NATS ridge",
                RouteWkt = "LINESTRING(8.30 61.10, 8.31 61.11, 8.32 61.12)",
                Details = new BackcountrySkiDetailsDto
                {
                    AscentMeters = 600, DescentMeters = 600, DistanceMeters = 4000,
                    ElevationMinMeters = 1200, ElevationMaxMeters = 1800,
                    AtesRating = AtesRating.Challenging, DominantAspect = Aspect.N,
                },
            });
        create.StatusCode.Should().Be(HttpStatusCode.Created,
            $"create failed: {await create.Content.ReadAsStringAsync()}");
        var created = (await create.Content.ReadFromJsonAsync<CreateBackcountrySkiActivityResponse>())!;

        var summary = await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=8.2&minLat=61.0&maxLon=8.4&maxLat=61.2");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == created.Id) == true ? body : null;
        }, description: "bcski summary projection arrives over NATS");

        var item = summary.Items.Single(i => i.Id == created.Id);
        item.Kind.Should().Be("backcountry_ski");
        item.GeometryKind.Should().Be("LineString");
    }

    [Fact]
    public async Task deleting_an_activity_removes_it_from_the_projection_over_nats()
    {
        var client = await RegisterAndAuthorizeActivitiesAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "To delete",
            Longitude = 11.55, Latitude = 60.55,
            Details = new FishingDetailsDto { WaterKind = WaterKind.River, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        // Wait for the create to project before triggering the delete — the
        // tombstone path is only meaningful if the row was there first.
        await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=11.4&minLat=60.4&maxLon=11.7&maxLat=60.7");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == id) == true ? body : null;
        }, description: "create projects before we issue the delete");

        var del = await client.DeleteAsync($"/api/activities/fishing/{id}");
        del.StatusCode.Should().Be(HttpStatusCode.NoContent);

        // The tombstone propagates over NATS and the projection drops the row.
        await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=11.4&minLat=60.4&maxLon=11.7&maxLat=60.7");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body!.Items.All(i => i.Id != id) ? body : null;
        }, description: "tombstone propagates over NATS and the projection drops the row");
    }

    [Fact]
    public async Task updates_project_the_latest_revision_over_nats()
    {
        var client = await RegisterAndAuthorizeActivitiesAsync();

        var create = await client.PostAsJsonAsync("/api/activities/fishing", new CreateFishingActivityRequest
        {
            Name = "First name",
            Longitude = 9.0, Latitude = 59.5,
            Details = new FishingDetailsDto { WaterKind = WaterKind.Lake, ShoreOrBoat = ShoreOrBoat.Shore },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var id = (await create.Content.ReadFromJsonAsync<CreateFishingActivityResponse>())!.Id;

        await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=8.9&minLat=59.4&maxLon=9.1&maxLat=59.6");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == id && i.Name == "First name") == true ? body : null;
        }, description: "first revision projects");

        var update = await client.PutAsJsonAsync($"/api/activities/fishing/{id}",
            new UpdateFishingActivityRequest { Name = "Second name" });
        update.StatusCode.Should().Be(HttpStatusCode.NoContent);

        // The projection updates to the latest name — uses Name as a proxy
        // because the bbox response doesn't carry an explicit version field
        // (clients refetch the detail when they need it).
        await Eventually.Returns<ActivitySummariesResponse>(async () =>
        {
            var r = await client.GetAsync(
                "/api/activities/summaries/bbox?minLon=8.9&minLat=59.4&maxLon=9.1&maxLat=59.6");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<ActivitySummariesResponse>();
            return body?.Items.Any(i => i.Id == id && i.Name == "Second name") == true ? body : null;
        }, description: "update is reflected over NATS");
    }

    private async Task<HttpClient> RegisterAndAuthorizeActivitiesAsync()
    {
        var email = $"acts-nats-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";
        var reg = await _topology.AuthClient.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        reg.IsSuccessStatusCode.Should().BeTrue(
            $"register failed: {reg.StatusCode}: {await reg.Content.ReadAsStringAsync()}");
        var tokens = (await reg.Content.ReadFromJsonAsync<AuthTokenResponse>())!;

        var client = _topology.ActivitiesClient;
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", tokens.AccessToken);
        return client;
    }
}
