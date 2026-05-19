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

namespace Turbo.Microservices.Behaviour;

/// <summary>
/// Cross-host behaviour: three independent HTTP services, no in-process
/// composition. The user-visible contract must be identical to the
/// modulith deploy — a JWT issued by the Auth host is accepted by the
/// Tracks and Geo hosts, and writes to Tracks round-trip through the
/// NATS-backed projection.
/// </summary>
[Collection("MicroservicesTopology")]
public sealed class MicroservicesTopologyBehaviour
{
    private readonly MicroservicesTopologyFixture _topology;
    public MicroservicesTopologyBehaviour(MicroservicesTopologyFixture topology) => _topology = topology;

    private static CreateTrackRequest SampleTrack(string name = "Cross-host run") => new()
    {
        Geometry = new GeometryDto
        {
            Points = new()
            {
                new PointDto { Longitude = 10.752, Latitude = 59.913 },
                new PointDto { Longitude = 10.753, Latitude = 59.914 },
            },
        },
        Metadata = new MetadataDto { Name = name, IconKey = "run" },
        Stats = new StatsDto { DistanceMeters = 100.0 },
    };

    [Fact]
    public async Task a_jwt_issued_by_the_auth_host_authorizes_the_tracks_and_geo_hosts()
    {
        var email = $"micro-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";

        var register = await _topology.AuthClient.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        register.IsSuccessStatusCode.Should().BeTrue(
            $"register failed: {register.StatusCode}: {await register.Content.ReadAsStringAsync()}");
        var tokens = (await register.Content.ReadFromJsonAsync<AuthTokenResponse>())!;

        var tracksClient = _topology.TracksClient;
        tracksClient.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);

        var tracksPost = await tracksClient.PostAsJsonAsync("/api/tracks/Tracks", SampleTrack());
        tracksPost.StatusCode.Should().Be(HttpStatusCode.Created,
            $"Tracks host rejected the Auth-issued JWT: {await tracksPost.Content.ReadAsStringAsync()}");

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
    public async Task tracks_post_eventually_appears_via_the_tracks_hosts_projection_subscriber()
    {
        var email = $"micro-proj-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";
        var reg = await _topology.AuthClient.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        reg.IsSuccessStatusCode.Should().BeTrue();
        var token = (await reg.Content.ReadFromJsonAsync<AuthTokenResponse>())!.AccessToken;

        var client = _topology.TracksClient;
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", token);

        var create = await client.PostAsJsonAsync("/api/tracks/Tracks", SampleTrack("Projected over NATS"));
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = (await create.Content.ReadFromJsonAsync<TrackResponse>())!;

        var fetched = await Eventually.Returns<TrackResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/tracks/Tracks/{created.Id}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<TrackResponse>()
                : null;
        }, description: "Tracks GET after NATS-delivered projection");

        fetched.Id.Should().Be(created.Id);
        fetched.Metadata.Name.Should().Be("Projected over NATS");
    }
}
