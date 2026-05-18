using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Geo.controller.request;
using Turboapi.Geo.controller.response;
using Turboapi.Activity.controller;
using Turboapi.Activity.domain;
using Xunit;

namespace Turbo.Microservices.Behaviour;

/// <summary>
/// Cross-host behaviour: three independent HTTP services, no in-process
/// composition. The user-visible contract must be identical to the
/// modulith deploy — a JWT issued by the Auth host is accepted by the
/// Activity and Geo hosts, and writes to Activity round-trip through the
/// NATS-backed projection.
/// </summary>
[Collection("MicroservicesTopology")]
public sealed class MicroservicesTopologyBehaviour
{
    private readonly MicroservicesTopologyFixture _topology;
    public MicroservicesTopologyBehaviour(MicroservicesTopologyFixture topology) => _topology = topology;

    [Fact]
    public async Task a_jwt_issued_by_the_auth_host_authorizes_the_activity_and_geo_hosts()
    {
        var email = $"micro-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";

        var register = await _topology.AuthClient.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        register.IsSuccessStatusCode.Should().BeTrue(
            $"register failed: {register.StatusCode}: {await register.Content.ReadAsStringAsync()}");
        var tokens = (await register.Content.ReadFromJsonAsync<AuthTokenResponse>())!;

        var activityClient = _topology.ActivityClient;
        activityClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);

        var activityPost = await activityClient.PostAsJsonAsync("/api/activity",
            new ActivityController.CreateActivityRequest(
                new Position { Latitude = 59.913, Longitude = 10.752 },
                "Cross-host run",
                "across three hosts via NATS",
                "run"));
        activityPost.StatusCode.Should().Be(HttpStatusCode.Created,
            $"Activity host rejected the Auth-issued JWT: {await activityPost.Content.ReadAsStringAsync()}");

        var geoClient = _topology.GeoClient;
        geoClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);

        var geoPost = await geoClient.PostAsJsonAsync("/api/geo/Locations", new CreateLocationRequest
        {
            Geometry = new GeometryData { Longitude = 10.752, Latitude = 59.913 },
            Display = new DisplayData { Name = "Microservice location", Description = "", Icon = "pin" }
        });
        geoPost.StatusCode.Should().Be(HttpStatusCode.Created,
            $"Geo host rejected the Auth-issued JWT: {await geoPost.Content.ReadAsStringAsync()}");
    }

    [Fact]
    public async Task activity_post_eventually_appears_via_the_activity_hosts_projection_subscriber()
    {
        var email = $"micro-proj-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";
        var reg = await _topology.AuthClient.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        reg.IsSuccessStatusCode.Should().BeTrue();
        var token = (await reg.Content.ReadFromJsonAsync<AuthTokenResponse>())!.AccessToken;

        var client = _topology.ActivityClient;
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", token);

        var create = await client.PostAsJsonAsync("/api/activity",
            new ActivityController.CreateActivityRequest(
                new Position { Latitude = 1, Longitude = 2 },
                "Projected over NATS",
                "outbox → dispatcher → NATS → subscriber → read model",
                "run"));
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = (await create.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>())!;

        var fetched = await Eventually.Returns<ActivityController.ActivityResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{created.ActivityId}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>()
                : null;
        }, description: "Activity GET after NATS-delivered projection");

        fetched.Id.Should().Be(created.ActivityId);
        fetched.Name.Should().Be("Projected over NATS");
    }
}
