using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Geo.controller.request;
using Turboapi.Geo.controller.response;
using Turboapi.Tracks.controller.request;
using Turboapi.Tracks.controller.response;
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

    private static CreateTrackRequest SampleTrack(string name = "via modulith") => new()
    {
        Geometry = new GeometryDto
        {
            Points = new()
            {
                new PointDto { Longitude = 12, Latitude = 13 },
                new PointDto { Longitude = 12.01, Latitude = 13.01 },
            },
        },
        Metadata = new MetadataDto { Name = name },
        Stats = new StatsDto { DistanceMeters = 200.0 },
    };

    [Fact]
    public async Task a_jwt_issued_by_auth_authorizes_calls_to_tracks_and_geo()
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

        var tracks = await authed.PostAsJsonAsync("/api/tracks/Tracks", SampleTrack("Cross-module flow"));
        tracks.StatusCode.Should().Be(HttpStatusCode.Created,
            $"tracks rejected the auth-issued JWT: {await tracks.Content.ReadAsStringAsync()}");

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
    public async Task creating_a_track_makes_it_retrievable_through_the_modulith_pipeline()
    {
        var client = await RegisterAsync();
        var request = SampleTrack();

        var create = await client.PostAsJsonAsync("/api/tracks/Tracks", request);
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = await create.Content.ReadFromJsonAsync<TrackResponse>();

        var fetched = await Eventually.Returns<TrackResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/tracks/Tracks/{created!.Id}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<TrackResponse>()
                : null;
        }, description: "Tracks GET reflects POST after in-process projection");

        fetched.Id.Should().Be(created!.Id);
        fetched.Metadata.Name.Should().Be(request.Metadata.Name);
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
