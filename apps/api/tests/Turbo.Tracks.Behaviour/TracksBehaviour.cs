using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Tracks.controller.request;
using Turboapi.Tracks.controller.response;
using Xunit;

namespace Turbo.Tracks.Behaviour;

[Collection("TracksHost")]
public sealed class TracksBehaviour
{
    private readonly TracksHostFixture _host;
    public TracksBehaviour(TracksHostFixture host) => _host = host;

    private static CreateTrackRequest SampleTrack(string name = "Morning run") =>
        new()
        {
            Geometry = new GeometryDto
            {
                Points = new()
                {
                    new PointDto { Longitude = 10.752, Latitude = 59.913 },
                    new PointDto { Longitude = 10.753, Latitude = 59.914 },
                    new PointDto { Longitude = 10.754, Latitude = 59.915 },
                },
                Elevations = new List<double> { 10, 12, 15 },
            },
            Metadata = new MetadataDto
            {
                Name = name,
                Description = "Three points along Karl Johans gate",
                ColorHex = "#FF0000",
                IconKey = "run",
                LineStyleKey = "solid",
                Smoothing = false,
            },
            Stats = new StatsDto
            {
                DistanceMeters = 320.5,
                AscentMeters = 5.0,
                DescentMeters = 0.0,
                MovingTimeSeconds = 180,
                RecordedAt = DateTime.UtcNow,
            },
        };

    [Fact]
    public async Task creating_a_track_makes_it_retrievable_by_its_owner()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);
        var request = SampleTrack();

        var create = await client.PostAsJsonAsync("/api/tracks/Tracks", request);
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = await create.Content.ReadFromJsonAsync<TrackResponse>();
        created.Should().NotBeNull();

        var fetched = await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/tracks/Tracks/{created!.Id}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<TrackResponse>()
                : null;
        }, description: $"GET /api/tracks/Tracks/{created!.Id}");

        fetched.Id.Should().Be(created.Id);
        fetched.Metadata.Name.Should().Be(request.Metadata.Name);
        fetched.Stats.DistanceMeters.Should().BeApproximately(request.Stats.DistanceMeters, 0.001);
        fetched.Geometry.Points.Should().HaveCount(3);
    }

    [Fact]
    public async Task deleting_a_track_makes_subsequent_reads_return_not_found_and_surfaces_a_tombstone()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/tracks/Tracks", SampleTrack("To be deleted"));
        var created = (await create.Content.ReadFromJsonAsync<TrackResponse>())!;

        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/tracks/Tracks/{created.Id}");
            return r.IsSuccessStatusCode ? (object)created : null;
        });

        var delete = await client.DeleteAsync($"/api/tracks/Tracks/{created.Id}");
        delete.StatusCode.Should().Be(HttpStatusCode.NoContent);

        await Eventually.Returns(async () =>
        {
            var r = await client.GetAsync($"/api/tracks/Tracks/{created.Id}");
            return r.StatusCode == HttpStatusCode.NotFound ? (object)"gone" : null;
        }, description: "GET after DELETE eventually returns 404");

        // The delta endpoint should surface the tombstone.
        var delta = await Eventually.Returns<TracksDeltaResponse>(async () =>
        {
            var r = await client.GetAsync("/api/tracks/Tracks?since=1970-01-01T00:00:00Z");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<TracksDeltaResponse>();
            return body is { Deleted: { } d } && d.Any(t => t.Id == created.Id) ? body : null;
        }, description: "Delta surfaces the tombstone after delete");

        delta.Deleted.Should().Contain(t => t.Id == created.Id);
    }

    [Fact]
    public async Task creating_a_track_without_a_token_is_rejected()
    {
        var anonymous = _host.CreateClient();
        var response = await anonymous.PostAsJsonAsync("/api/tracks/Tracks", SampleTrack());
        response.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }

    [Fact]
    public async Task another_user_cannot_read_someone_elses_track()
    {
        var owner = Guid.NewGuid();
        var stranger = Guid.NewGuid();
        var ownerClient = _host.CreateClientAs(owner);
        var strangerClient = _host.CreateClientAs(stranger);

        var create = await ownerClient.PostAsJsonAsync("/api/tracks/Tracks", SampleTrack("Private"));
        var created = (await create.Content.ReadFromJsonAsync<TrackResponse>())!;
        await Eventually.Returns(async () =>
        {
            var r = await ownerClient.GetAsync($"/api/tracks/Tracks/{created.Id}");
            return r.IsSuccessStatusCode ? (object)"there" : null;
        });

        var strangerRead = await strangerClient.GetAsync($"/api/tracks/Tracks/{created.Id}");
        strangerRead.StatusCode.Should().Be(HttpStatusCode.NotFound,
            "ownership filter in the read handler should hide the row from non-owners as a 404, not a 403, to avoid leaking existence");
    }

    [Fact]
    public async Task updating_a_track_with_a_stale_If_Match_returns_412_then_succeeds_with_fresh_version()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/tracks/Tracks", SampleTrack());
        var created = (await create.Content.ReadFromJsonAsync<TrackResponse>())!;

        // Wait for the projection to land so the read endpoint surfaces a version.
        var initial = await Eventually.Returns<TrackResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/tracks/Tracks/{created.Id}");
            if (!r.IsSuccessStatusCode) return null;
            return (await r.Content.ReadFromJsonAsync<TrackResponse>()) is { Version: > 0 } b ? b : null;
        }, description: "initial projection lands with version stamped");

        initial.Version.Should().Be(1);

        // First update with the correct version succeeds.
        var firstUpdate = new UpdateTrackRequest
        {
            Metadata = new MetadataChangesetDto { Name = "Renamed first" },
        };
        var put1 = new HttpRequestMessage(HttpMethod.Put, $"/api/tracks/Tracks/{created.Id}")
        {
            Content = JsonContent.Create(firstUpdate),
        };
        put1.Headers.IfMatch.Add(new EntityTagHeaderValue($"\"{initial.Version}\""));
        var put1Response = await client.SendAsync(put1);
        put1Response.StatusCode.Should().Be(HttpStatusCode.OK);

        // Wait until the projection has bumped the version.
        await Eventually.Returns<TrackResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/tracks/Tracks/{created.Id}");
            if (!r.IsSuccessStatusCode) return null;
            var body = await r.Content.ReadFromJsonAsync<TrackResponse>();
            return body is { Version: > 1, Metadata.Name: "Renamed first" } ? body : null;
        }, description: "first update is visible with bumped version");

        // Second update with the OLD version should be rejected.
        var staleUpdate = new UpdateTrackRequest
        {
            Metadata = new MetadataChangesetDto { Name = "Should fail" },
        };
        var put2 = new HttpRequestMessage(HttpMethod.Put, $"/api/tracks/Tracks/{created.Id}")
        {
            Content = JsonContent.Create(staleUpdate),
        };
        put2.Headers.IfMatch.Add(new EntityTagHeaderValue($"\"{initial.Version}\""));
        var put2Response = await client.SendAsync(put2);
        put2Response.StatusCode.Should().Be(HttpStatusCode.PreconditionFailed,
            "If-Match against a stale version must be rejected with 412 to surface the conflict to the client");
    }
}

[CollectionDefinition("TracksHost")]
public sealed class TracksHostCollection : ICollectionFixture<TracksHostFixture> { }
