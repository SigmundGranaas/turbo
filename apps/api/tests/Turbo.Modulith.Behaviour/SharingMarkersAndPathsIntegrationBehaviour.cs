using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Geo.controller.request;
using Turboapi.Geo.controller.response;
using Turboapi.Geo.domain.value;
using Turboapi.Sharing.domain.service;
using Turboapi.Tracks.controller.request;
using Turboapi.Tracks.controller.response;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// Confirms that creating a Marker (Location) or a Path (Track)
/// materializes a Resource envelope in the Sharing service via the
/// in-process event bus. Same sidecar mechanism as Collections — the
/// payload modules are unaware Sharing is listening.
/// </summary>
[Collection("ModulithHost")]
public sealed class SharingMarkersAndPathsIntegrationBehaviour
{
    private readonly ModulithHostFixture _host;
    public SharingMarkersAndPathsIntegrationBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task creating_a_marker_lands_a_marker_resource_envelope()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/geo/Locations", new CreateLocationRequest
        {
            Geometry = new GeometryData { Longitude = -0.1, Latitude = 51.5 },
            Display = new DisplayData { Name = "Trailhead", Description = null, Icon = "pin" },
        });
        var created = (await create.Content.ReadFromJsonAsync<LocationResponse>())!;

        var envelope = await Eventually.Returns(async () =>
        {
            var page = await client.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=marker");
            return page!.Items.FirstOrDefault(e => e.Id == created.Id);
        }, description: "Marker resource envelope");

        envelope.Should().NotBeNull();
        envelope!.Type.Should().Be("marker");
        envelope.MyRole.Should().Be("owner");
    }

    [Fact]
    public async Task creating_a_path_lands_a_path_resource_envelope()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/tracks/Tracks", new CreateTrackRequest
        {
            Metadata = new MetadataDto
            {
                Name = "Loop trail",
                Smoothing = false,
            },
            Geometry = new GeometryDto
            {
                Points = new List<PointDto>
                {
                    new() { Longitude = 0.0, Latitude = 0.0 },
                    new() { Longitude = 1.0, Latitude = 1.0 },
                },
            },
            Stats = new StatsDto { DistanceMeters = 100.0 },
        });
        var created = (await create.Content.ReadFromJsonAsync<TrackResponse>())!;

        var envelope = await Eventually.Returns(async () =>
        {
            var page = await client.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=path");
            return page!.Items.FirstOrDefault(e => e.Id == created.Id);
        }, description: "Path resource envelope");

        envelope.Should().NotBeNull();
        envelope!.Type.Should().Be("path");
        envelope.MyRole.Should().Be("owner");
    }
}
