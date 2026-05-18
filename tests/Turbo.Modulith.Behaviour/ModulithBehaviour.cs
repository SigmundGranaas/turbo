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

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// Cross-module behaviour against the modulith deploy: one process, one
/// Postgres host, three databases, in-process event delivery, no NATS.
/// The user-visible contract must be identical to the microservice deploy:
/// register → JWT works against the other two modules' endpoints; writes
/// land in the read model via in-process projection.
/// </summary>
[Collection("ModulithHost")]
public sealed class ModulithBehaviour
{
    private readonly ModulithHostFixture _host;
    public ModulithBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task a_jwt_issued_by_auth_authorizes_calls_to_activity_and_geo()
    {
        var client = _host.CreateClient();
        var email = $"modulith-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";

        var register = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        register.IsSuccessStatusCode.Should().BeTrue(
            $"register failed: {register.StatusCode}: {await register.Content.ReadAsStringAsync()}");

        var tokens = await register.Content.ReadFromJsonAsync<AuthTokenResponse>();
        tokens.Should().NotBeNull();
        tokens!.AccessToken.Should().NotBeNullOrEmpty();

        var authed = _host.CreateClient();
        authed.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);

        // Activity accepts the JWT issued by Auth
        var activity = await authed.PostAsJsonAsync("/api/activity",
            new ActivityController.CreateActivityRequest(
                new Position { Latitude = 59.913, Longitude = 10.752 },
                "Cross-module flow",
                "registered via auth, posted via activity",
                "run"));
        activity.StatusCode.Should().Be(HttpStatusCode.Created,
            $"activity rejected the auth-issued JWT: {await activity.Content.ReadAsStringAsync()}");

        // Geo accepts the same JWT
        var geo = await authed.PostAsJsonAsync("/api/geo/Locations",
            new CreateLocationRequest
            {
                Geometry = new GeometryData { Longitude = 10.752, Latitude = 59.913 },
                Display = new DisplayData { Name = "Modulith location", Description = "in-process projection", Icon = "pin" }
            });
        geo.StatusCode.Should().Be(HttpStatusCode.Created,
            $"geo rejected the auth-issued JWT: {await geo.Content.ReadAsStringAsync()}");

        var geoBody = await geo.Content.ReadFromJsonAsync<LocationResponse>();
        geoBody!.Display.Name.Should().Be("Modulith location");
    }

    [Fact]
    public async Task creating_an_activity_makes_it_retrievable_through_the_modulith_pipeline()
    {
        var client = await RegisterAsync();
        var request = new ActivityController.CreateActivityRequest(
            new Position { Latitude = 12, Longitude = 13 },
            "via modulith",
            "in-process delivery",
            "icon");

        var create = await client.PostAsJsonAsync("/api/activity", request);
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = await create.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>();

        // The projection runs through the in-process subscriber host, not NATS.
        var fetched = await Eventually.Returns<ActivityController.ActivityResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{created!.ActivityId}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>()
                : null;
        }, description: "Activity GET reflects POST after in-process projection");

        fetched.Id.Should().Be(created!.ActivityId);
        fetched.Name.Should().Be("via modulith");
    }

    [Fact]
    public async Task creating_a_location_in_the_modulith_round_trips_through_the_read_model()
    {
        var client = await RegisterAsync();

        var create = await client.PostAsJsonAsync("/api/geo/Locations", new CreateLocationRequest
        {
            Geometry = new GeometryData { Longitude = -0.1, Latitude = 51.5 },
            Display = new DisplayData { Name = "London", Description = "in modulith", Icon = "pin" }
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = await create.Content.ReadFromJsonAsync<LocationResponse>();

        // The projection runs through the in-process subscriber chain.
        var body = await Eventually.Returns<LocationResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/geo/Locations/{created!.Id}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<LocationResponse>()
                : null;
        }, description: "Geo GET reflects POST after in-process projection");

        body.Display.Name.Should().Be("London");
        body.Geometry.Longitude.Should().BeApproximately(-0.1, 0.0001);
    }

    private async Task<HttpClient> RegisterAsync()
    {
        var client = _host.CreateClient();
        var email = $"modulith-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";
        var register = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        register.IsSuccessStatusCode.Should().BeTrue(
            $"register precondition failed: {register.StatusCode}: {await register.Content.ReadAsStringAsync()}");
        var tokens = (await register.Content.ReadFromJsonAsync<AuthTokenResponse>())!;
        var authed = _host.CreateClient();
        authed.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);
        return authed;
    }
}
