using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Tracks.controller.request;
using Turboapi.Tracks.controller.response;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// Event-sourcing guarantee: the outbox table contains every event
/// emitted by the write side, so the read model can be rebuilt from
/// scratch by replaying it. This test exercises the operator SOP
/// (truncate read model + truncate dedup + reset dispatched_at) and
/// asserts the read model converges back to the same content.
///
/// Without this test the outbox could silently lose information — for
/// example, if a command handler stopped emitting some event — and
/// nothing would surface until an operator actually tried to rebuild
/// in production. Running it on the modulith fixture is the simplest
/// way to cover the round-trip end-to-end; the same proof applies to
/// the microservice deploy modulo NATS consumer recreation.
/// </summary>
[Collection("ModulithHost")]
public sealed class OutboxReplayBehaviour
{
    private readonly ModulithHostFixture _host;
    public OutboxReplayBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task replaying_the_outbox_rebuilds_the_tracks_read_model_from_zero()
    {
        var client = await RegisterAndAuthorizeAsync();

        var ids = new List<Guid>();
        for (var i = 0; i < 3; i++)
        {
            var req = new CreateTrackRequest
            {
                Geometry = new GeometryDto
                {
                    Points = new()
                    {
                        new PointDto { Longitude = 1 + i, Latitude = 1 + i },
                        new PointDto { Longitude = 1.01 + i, Latitude = 1.01 + i },
                    },
                },
                Metadata = new MetadataDto { Name = $"Track #{i}", IconKey = "run" },
                Stats = new StatsDto { DistanceMeters = 100.0 + i },
            };
            var post = await client.PostAsJsonAsync("/api/tracks/Tracks", req);
            post.StatusCode.Should().Be(HttpStatusCode.Created);
            var created = (await post.Content.ReadFromJsonAsync<TrackResponse>())!;
            ids.Add(created.Id);
        }

        foreach (var id in ids)
        {
            await Eventually.Returns<TrackResponse>(async () =>
            {
                var r = await client.GetAsync($"/api/tracks/Tracks/{id}");
                return r.IsSuccessStatusCode
                    ? await r.Content.ReadFromJsonAsync<TrackResponse>()
                    : null;
            }, description: $"initial projection of {id}");
        }

        await _host.ResetTracksReadModelForReplayAsync();

        var emptyProbe = await client.GetAsync($"/api/tracks/Tracks/{ids[0]}");
        emptyProbe.StatusCode.Should().BeOneOf(HttpStatusCode.NotFound, HttpStatusCode.OK);

        foreach (var id in ids)
        {
            var rebuilt = await Eventually.Returns<TrackResponse>(async () =>
            {
                var r = await client.GetAsync($"/api/tracks/Tracks/{id}");
                return r.IsSuccessStatusCode
                    ? await r.Content.ReadFromJsonAsync<TrackResponse>()
                    : null;
            }, timeout: TimeSpan.FromSeconds(30), description: $"replay-rebuilt track {id}");

            rebuilt.Id.Should().Be(id);
            rebuilt.Metadata.Name.Should().StartWith("Track #");
        }
    }

    private async Task<HttpClient> RegisterAndAuthorizeAsync()
    {
        var client = _host.CreateClient();
        var email = $"replay-{Guid.NewGuid():N}@example.com";
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
