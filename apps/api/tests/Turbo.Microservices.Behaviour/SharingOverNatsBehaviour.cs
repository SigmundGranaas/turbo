using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Collections.controller.request;
using Turboapi.Collections.controller.response;
using Turboapi.Geo.controller.request;
using Turboapi.Geo.controller.response;
using Turboapi.Sharing.controller;
using Turboapi.Sharing.controller.request;
using Turboapi.Sharing.domain.service;
using Turboapi.Tracks.controller.request;
using Turboapi.Tracks.controller.response;
using Xunit;

namespace Turbo.Microservices.Behaviour;

/// <summary>
/// True end-to-end across the microservices topology: the Collections /
/// Geo / Tracks hosts publish domain events to their own JetStream
/// streams; the Sharing host runs in its own process with no in-process
/// reference to those modules, and its NATS subscribers pick the events
/// up. These tests verify the Resource sidecar flows end-to-end over
/// the broker — the path that production actually exercises, distinct
/// from the modulith integration tests which run everything in one
/// process.
/// </summary>
[Collection("MicroservicesTopology")]
public sealed class SharingOverNatsBehaviour
{
    private readonly MicroservicesTopologyFixture _topology;
    public SharingOverNatsBehaviour(MicroservicesTopologyFixture topology) => _topology = topology;

    [Fact]
    public async Task creating_a_collection_eventually_lands_a_resource_envelope_in_the_sharing_host_via_nats()
    {
        var owner = Guid.NewGuid();
        var collectionsClient = _topology.CollectionsClientAs(owner);
        var sharingClient = _topology.SharingClientAs(owner);

        var create = await collectionsClient.PostAsJsonAsync(
            "/api/collections/Collections",
            new CreateCollectionRequest
            {
                Name = "Trip plan over NATS",
                SortOrder = 0,
            });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        var envelope = await Eventually.Returns(async () =>
        {
            var page = await sharingClient.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.FirstOrDefault(e => e.Id == created.Id);
        }, description: "Sharing host resource envelope from cross-host NATS delivery");

        envelope.Should().NotBeNull();
        envelope!.Type.Should().Be("collection");
        envelope.MyRole.Should().Be("owner");
    }

    [Fact]
    public async Task creating_a_marker_eventually_lands_a_resource_envelope_via_nats()
    {
        var owner = Guid.NewGuid();
        var geoClient = _topology.GeoClientAs(owner);
        var sharingClient = _topology.SharingClientAs(owner);

        var create = await geoClient.PostAsJsonAsync("/api/geo/Locations", new CreateLocationRequest
        {
            Geometry = new GeometryData { Longitude = 10.752, Latitude = 59.913 },
            Display = new DisplayData { Name = "Trailhead via NATS", Description = null, Icon = "pin" },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = (await create.Content.ReadFromJsonAsync<LocationResponse>())!;

        var envelope = await Eventually.Returns(async () =>
        {
            var page = await sharingClient.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=marker");
            return page!.Items.FirstOrDefault(e => e.Id == created.Id);
        }, description: "Sharing host marker envelope from cross-host NATS delivery");

        envelope.Should().NotBeNull();
        envelope!.Type.Should().Be("marker");
    }

    [Fact]
    public async Task creating_a_track_eventually_lands_a_resource_envelope_via_nats()
    {
        var owner = Guid.NewGuid();
        var tracksClient = _topology.TracksClientAs(owner);
        var sharingClient = _topology.SharingClientAs(owner);

        var create = await tracksClient.PostAsJsonAsync("/api/tracks/Tracks", new CreateTrackRequest
        {
            Geometry = new GeometryDto
            {
                Points = new()
                {
                    new PointDto { Longitude = 10.752, Latitude = 59.913 },
                    new PointDto { Longitude = 10.753, Latitude = 59.914 },
                },
            },
            Metadata = new MetadataDto { Name = "Loop over NATS", IconKey = "run" },
            Stats = new StatsDto { DistanceMeters = 100.0 },
        });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = (await create.Content.ReadFromJsonAsync<TrackResponse>())!;

        var envelope = await Eventually.Returns(async () =>
        {
            var page = await sharingClient.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=path");
            return page!.Items.FirstOrDefault(e => e.Id == created.Id);
        }, description: "Sharing host path envelope from cross-host NATS delivery");

        envelope.Should().NotBeNull();
        envelope!.Type.Should().Be("path");
    }

    [Fact]
    public async Task collection_shared_to_a_friend_via_grant_appears_in_their_sync_across_hosts()
    {
        // Two distinct users. Owner creates a Collection on the Collections
        // host; once Sharing has the envelope (via NATS), the owner issues
        // a grant; the friend then sees the resource in their own sync.
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var collectionsClient = _topology.CollectionsClientAs(owner);
        var ownerSharingClient = _topology.SharingClientAs(owner);
        var friendSharingClient = _topology.SharingClientAs(friend);

        var create = await collectionsClient.PostAsJsonAsync(
            "/api/collections/Collections",
            new CreateCollectionRequest { Name = "Shared trip", SortOrder = 0 });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        await Eventually.Returns(async () =>
        {
            var page = await ownerSharingClient.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Sharing host has the new collection's envelope before grant issuance");

        var grant = await ownerSharingClient.PostAsJsonAsync(
            "/api/sharing/grants/users",
            new GrantToUserRequest(created.Id, friend, "viewer", null));
        grant.StatusCode.Should().Be(HttpStatusCode.OK);

        var friendPage = await friendSharingClient.GetFromJsonAsync<ResourceSyncPage>(
            "/api/sharing/resources/sync?types=collection");
        friendPage!.Items.Should().Contain(e => e.Id == created.Id && e.MyRole == "viewer");
    }

    [Fact]
    public async Task friend_code_lookup_works_in_the_microservices_topology()
    {
        var alice = Guid.NewGuid();
        var bob = Guid.NewGuid();
        var aliceSharing = _topology.SharingClientAs(alice);
        var bobSharing = _topology.SharingClientAs(bob);

        var profile = await aliceSharing.GetFromJsonAsync<UserProfileDto>("/api/sharing/me/profile");
        profile!.FriendCode.Should().NotBeNullOrEmpty();

        var lookup = await bobSharing.GetFromJsonAsync<UserLookupResponse>(
            $"/api/sharing/users/lookup?code={profile.FriendCode}");
        lookup!.UserId.Should().Be(alice);
    }
}
