using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Tracks.controller.request;
using Turboapi.Tracks.controller.response;
using Xunit;

namespace Turbo.Tracks.Behaviour;

/// <summary>
/// Failure-mode guarantee: when the broker is unreachable at the moment a
/// write commits, the write still succeeds with 201, the read endpoint
/// returns 404 while the broker is down, and the read endpoint eventually
/// returns the body after the broker comes back. Same shape as Geo's
/// durability test.
/// </summary>
[Collection("TracksHost")]
public sealed class TracksDurabilityBehaviour
{
    private readonly TracksHostFixture _host;
    public TracksDurabilityBehaviour(TracksHostFixture host) => _host = host;

    [Fact]
    public async Task creates_succeed_and_become_visible_after_the_broker_recovers()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);
        Guid trackId;

        await _host.PauseBrokerAsync();
        try
        {
            var request = new CreateTrackRequest
            {
                Geometry = new GeometryDto
                {
                    Points = new()
                    {
                        new PointDto { Longitude = -0.1, Latitude = 51.5 },
                        new PointDto { Longitude = -0.11, Latitude = 51.51 },
                    },
                },
                Metadata = new MetadataDto { Name = "Recorded during outage" },
                Stats = new StatsDto { DistanceMeters = 1100.0 },
            };
            var create = await client.PostAsJsonAsync("/api/tracks/Tracks", request);
            create.StatusCode.Should().Be(HttpStatusCode.Created,
                "the write path must commit to the outbox even when the broker is unreachable");
            trackId = (await create.Content.ReadFromJsonAsync<TrackResponse>())!.Id;

            var whilePaused = await client.GetAsync($"/api/tracks/Tracks/{trackId}");
            whilePaused.StatusCode.Should().Be(HttpStatusCode.NotFound,
                "the projection runs through the broker; with the broker paused the read model cannot catch up");
        }
        finally
        {
            await _host.UnpauseBrokerAsync();
        }

        var recovered = await Eventually.Returns<TrackResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/tracks/Tracks/{trackId}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<TrackResponse>()
                : null;
        }, timeout: TimeSpan.FromSeconds(20),
           description: $"GET /api/tracks/Tracks/{trackId} after broker recovery");

        recovered.Id.Should().Be(trackId);
        recovered.Metadata.Name.Should().Be("Recorded during outage");
    }
}
