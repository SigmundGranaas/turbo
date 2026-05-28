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
    public async Task friend_with_a_viewer_grant_can_actually_fetch_the_collections_content()
    {
        // The real proof: not just "envelope visible in sync", but "friend
        // can GET the collection from the Collections host with the actual
        // name, items, metadata. Without this the Sharing layer is
        // decorative — grants exist but the payload modules never consult
        // them, so a friend with viewer role still gets 404 from
        // /api/collections/Collections/{id}.
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var ownerCollections = _topology.CollectionsClientAs(owner);
        var friendCollections = _topology.CollectionsClientAs(friend);
        var ownerSharing = _topology.SharingClientAs(owner);

        var create = await ownerCollections.PostAsJsonAsync(
            "/api/collections/Collections",
            new CreateCollectionRequest
            {
                Name = "Trip with content",
                Description = "Visible to my friend",
                SortOrder = 0,
            });
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        // Wait for the sidecar to land the Resource envelope.
        await Eventually.Returns(async () =>
        {
            var page = await ownerSharing.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Sharing envelope materialised");

        var grant = await ownerSharing.PostAsJsonAsync(
            "/api/sharing/grants/users",
            new GrantToUserRequest(created.Id, friend, "viewer", null));
        grant.StatusCode.Should().Be(HttpStatusCode.OK);

        // Friend now fetches the collection from the Collections host. This
        // is the user-visible value: "alice shared a trip with me, I want
        // to see what's in it".
        var fetched = await friendCollections.GetAsync(
            $"/api/collections/Collections/{created.Id}");
        fetched.StatusCode.Should().Be(HttpStatusCode.OK,
            $"Friend with viewer grant must be able to read the collection. "
            + $"Got {fetched.StatusCode}: {await fetched.Content.ReadAsStringAsync()}");
        var body = (await fetched.Content.ReadFromJsonAsync<CollectionResponse>())!;
        body.Name.Should().Be("Trip with content");
        body.Description.Should().Be("Visible to my friend");
    }

    [Fact]
    public async Task friend_with_an_editor_grant_can_add_an_item_to_the_shared_collection()
    {
        // Editor goes further than viewer: the friend should be able to
        // MUTATE the shared collection (add an item). Same gate problem:
        // if Collections only consults its own OwnerId field, the editor
        // grant doesn't actually confer write access.
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var ownerCollections = _topology.CollectionsClientAs(owner);
        var friendCollections = _topology.CollectionsClientAs(friend);
        var ownerSharing = _topology.SharingClientAs(owner);

        var create = await ownerCollections.PostAsJsonAsync(
            "/api/collections/Collections",
            new CreateCollectionRequest { Name = "Joint planning", SortOrder = 0 });
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        await Eventually.Returns(async () =>
        {
            var page = await ownerSharing.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Sharing envelope materialised");

        await ownerSharing.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(created.Id, friend, "editor", null));

        var add = await friendCollections.PostAsJsonAsync(
            $"/api/collections/Collections/{created.Id}/items",
            new AddItemRequest { Type = "marker", Uuid = Guid.NewGuid().ToString() });
        add.StatusCode.Should().Be(HttpStatusCode.NoContent,
            $"Friend with editor grant must be able to add items. "
            + $"Got {add.StatusCode}: {await add.Content.ReadAsStringAsync()}");
    }

    [Fact]
    public async Task friend_with_no_grant_cannot_read_the_collection()
    {
        // Negative case: without a grant, the friend's GET must NOT succeed.
        // This guards the property "grants are the gate" — if it ever
        // started returning 200 for unrelated users, the access-control
        // layer would be a no-op.
        var owner = Guid.NewGuid();
        var stranger = Guid.NewGuid();
        var ownerCollections = _topology.CollectionsClientAs(owner);
        var strangerCollections = _topology.CollectionsClientAs(stranger);

        var create = await ownerCollections.PostAsJsonAsync(
            "/api/collections/Collections",
            new CreateCollectionRequest { Name = "Private trip", SortOrder = 0 });
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        var fetched = await strangerCollections.GetAsync(
            $"/api/collections/Collections/{created.Id}");
        fetched.StatusCode.Should().BeOneOf(
            HttpStatusCode.NotFound,
            HttpStatusCode.Forbidden);
    }

    [Fact]
    public async Task friend_with_a_viewer_grant_can_actually_fetch_the_markers_content()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var ownerGeo = _topology.GeoClientAs(owner);
        var friendGeo = _topology.GeoClientAs(friend);
        var ownerSharing = _topology.SharingClientAs(owner);

        var create = await ownerGeo.PostAsJsonAsync("/api/geo/Locations", new CreateLocationRequest
        {
            Geometry = new GeometryData { Longitude = 10.752, Latitude = 59.913 },
            Display = new DisplayData
            {
                Name = "Hidden cabin",
                Description = "Take the trail past the lake",
                Icon = "home",
            },
        });
        var created = (await create.Content.ReadFromJsonAsync<LocationResponse>())!;

        await Eventually.Returns(async () =>
        {
            var page = await ownerSharing.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=marker");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Sharing envelope materialised");

        await ownerSharing.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(created.Id, friend, "viewer", null));

        var fetched = await friendGeo.GetAsync($"/api/geo/Locations/{created.Id}");
        fetched.StatusCode.Should().Be(HttpStatusCode.OK,
            $"Friend with viewer grant must read the marker. "
            + $"Got {fetched.StatusCode}: {await fetched.Content.ReadAsStringAsync()}");
        var body = (await fetched.Content.ReadFromJsonAsync<LocationResponse>())!;
        body.Display.Name.Should().Be("Hidden cabin");
        body.Display.Description.Should().Be("Take the trail past the lake");
    }

    [Fact]
    public async Task friend_with_a_viewer_grant_can_actually_fetch_the_tracks_geometry()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var ownerTracks = _topology.TracksClientAs(owner);
        var friendTracks = _topology.TracksClientAs(friend);
        var ownerSharing = _topology.SharingClientAs(owner);

        var create = await ownerTracks.PostAsJsonAsync("/api/tracks/Tracks", new CreateTrackRequest
        {
            Geometry = new GeometryDto
            {
                Points = new()
                {
                    new PointDto { Longitude = 10.0, Latitude = 60.0 },
                    new PointDto { Longitude = 10.1, Latitude = 60.1 },
                },
            },
            Metadata = new MetadataDto { Name = "Ridge route", IconKey = "hike" },
            Stats = new StatsDto { DistanceMeters = 5000.0 },
        });
        var created = (await create.Content.ReadFromJsonAsync<TrackResponse>())!;

        await Eventually.Returns(async () =>
        {
            var page = await ownerSharing.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=path");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Sharing envelope materialised");

        await ownerSharing.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(created.Id, friend, "viewer", null));

        var fetched = await friendTracks.GetAsync($"/api/tracks/Tracks/{created.Id}");
        fetched.StatusCode.Should().Be(HttpStatusCode.OK,
            $"Friend with viewer grant must read the track. "
            + $"Got {fetched.StatusCode}: {await fetched.Content.ReadAsStringAsync()}");
        var body = (await fetched.Content.ReadFromJsonAsync<TrackResponse>())!;
        body.Metadata.Name.Should().Be("Ridge route");
        body.Geometry.Points.Should().HaveCount(2);
    }

    [Fact]
    public async Task group_member_can_read_a_collection_shared_with_their_group()
    {
        var owner = Guid.NewGuid();
        var memberA = Guid.NewGuid();
        var memberB = Guid.NewGuid();
        var ownerCollections = _topology.CollectionsClientAs(owner);
        var ownerSharing = _topology.SharingClientAs(owner);

        var create = await ownerCollections.PostAsJsonAsync(
            "/api/collections/Collections",
            new CreateCollectionRequest { Name = "Group trip", SortOrder = 0 });
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        await Eventually.Returns(async () =>
        {
            var page = await ownerSharing.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Envelope materialised");

        var groupResp = await ownerSharing.PostAsJsonAsync("/api/sharing/groups",
            new CreateGroupRequest("Ski crew"));
        var group = (await groupResp.Content.ReadFromJsonAsync<GroupDto>())!;
        await ownerSharing.PostAsJsonAsync($"/api/sharing/groups/{group.Id}/members",
            new GroupMemberRequest(memberA));
        await ownerSharing.PostAsJsonAsync($"/api/sharing/groups/{group.Id}/members",
            new GroupMemberRequest(memberB));
        await ownerSharing.PostAsJsonAsync("/api/sharing/grants/groups",
            new GrantToGroupRequest(created.Id, group.Id, "viewer", null));

        foreach (var member in new[] { memberA, memberB })
        {
            var memberClient = _topology.CollectionsClientAs(member);
            var fetched = await memberClient.GetAsync(
                $"/api/collections/Collections/{created.Id}");
            fetched.StatusCode.Should().Be(HttpStatusCode.OK,
                $"Group member {member} must be able to read the collection.");
            var body = (await fetched.Content.ReadFromJsonAsync<CollectionResponse>())!;
            body.Name.Should().Be("Group trip");
        }
    }

    [Fact]
    public async Task link_redemption_by_a_stranger_grants_access_to_the_collections_content()
    {
        var owner = Guid.NewGuid();
        var stranger = Guid.NewGuid();
        var ownerCollections = _topology.CollectionsClientAs(owner);
        var strangerCollections = _topology.CollectionsClientAs(stranger);
        var ownerSharing = _topology.SharingClientAs(owner);
        var strangerSharing = _topology.SharingClientAs(stranger);

        var create = await ownerCollections.PostAsJsonAsync(
            "/api/collections/Collections",
            new CreateCollectionRequest { Name = "Public link trip", SortOrder = 0 });
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        await Eventually.Returns(async () =>
        {
            var page = await ownerSharing.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Envelope materialised");

        var linkResp = await ownerSharing.PostAsJsonAsync("/api/sharing/grants/links",
            new GrantAsLinkRequest(created.Id, "viewer", null));
        var link = (await linkResp.Content.ReadFromJsonAsync<LinkGrantDto>())!;

        var redeem = await strangerSharing.PostAsync(
            $"/api/sharing/grants/links/{link.LinkToken}/redeem", null);
        redeem.StatusCode.Should().Be(HttpStatusCode.OK);

        var fetched = await strangerCollections.GetAsync(
            $"/api/collections/Collections/{created.Id}");
        fetched.StatusCode.Should().Be(HttpStatusCode.OK,
            "After redeeming the link, the stranger must be able to read the actual collection content.");
        var body = (await fetched.Content.ReadFromJsonAsync<CollectionResponse>())!;
        body.Name.Should().Be("Public link trip");
    }

    [Fact]
    public async Task revoked_grant_immediately_blocks_further_reads()
    {
        // The other side of "access is real": after the owner revokes, the
        // friend's NEXT read returns 404/403. No stale access after grant
        // revocation is exactly what makes "share" trustworthy.
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var ownerCollections = _topology.CollectionsClientAs(owner);
        var friendCollections = _topology.CollectionsClientAs(friend);
        var ownerSharing = _topology.SharingClientAs(owner);

        var create = await ownerCollections.PostAsJsonAsync(
            "/api/collections/Collections",
            new CreateCollectionRequest { Name = "Revoke test", SortOrder = 0 });
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        await Eventually.Returns(async () =>
        {
            var page = await ownerSharing.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Envelope materialised");

        await ownerSharing.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(created.Id, friend, "viewer", null));

        var beforeRevoke = await friendCollections.GetAsync(
            $"/api/collections/Collections/{created.Id}");
        beforeRevoke.StatusCode.Should().Be(HttpStatusCode.OK,
            "Sanity: the grant works before revoke.");

        await ownerSharing.DeleteAsync(
            $"/api/sharing/grants/resources/{created.Id}/users/{friend}");

        var afterRevoke = await friendCollections.GetAsync(
            $"/api/collections/Collections/{created.Id}");
        afterRevoke.StatusCode.Should().BeOneOf(
            new[] { HttpStatusCode.NotFound, HttpStatusCode.Forbidden },
            "Revoke must take effect immediately on the next read; no stale access.");
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
