using System.Net.Http.Json;
using FluentAssertions;
using Microsoft.Extensions.DependencyInjection;
using Turboapi.Collections.controller.request;
using Turboapi.Collections.controller.response;
using Turboapi.Sharing.controller.request;
using Turboapi.Sharing.domain.service;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// Phase-1 integration: creating a Collection materializes a Resource
/// envelope in the Sharing service via the in-process event bus. From
/// that moment the collection is shareable through the universal
/// /api/sharing/grants endpoints without any Collections-side change.
/// </summary>
[Collection("ModulithHost")]
public sealed class SharingCollectionsIntegrationBehaviour
{
    private readonly ModulithHostFixture _host;
    public SharingCollectionsIntegrationBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task creating_a_collection_lands_a_corresponding_resource_envelope()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync("/api/collections/Collections",
            new CreateCollectionRequest
            {
                Name = "Trip plan",
                Description = null,
                ColorHex = null,
                IconKey = null,
                SortOrder = 0,
            });
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        // The sidecar handler runs asynchronously through the outbox
        // dispatcher; poll the sync endpoint until it shows up.
        var seen = await Eventually.Returns(async () =>
        {
            var page = await client.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.FirstOrDefault(e => e.Id == created.Id);
        }, description: "Resource envelope for new collection");

        seen.Should().NotBeNull();
        seen!.MyRole.Should().Be("owner");
        seen.Type.Should().Be("collection");
    }

    [Fact]
    public async Task sharing_a_collection_via_grant_makes_it_visible_to_friend_via_sync()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var ownerClient = _host.CreateClientAs(owner);

        var create = await ownerClient.PostAsJsonAsync("/api/collections/Collections",
            new CreateCollectionRequest { Name = "Shared trip", SortOrder = 0 });
        var created = (await create.Content.ReadFromJsonAsync<CollectionResponse>())!;

        // Wait for the sidecar to land the resource envelope.
        await Eventually.Returns(async () =>
        {
            var page = await ownerClient.GetFromJsonAsync<ResourceSyncPage>(
                "/api/sharing/resources/sync?types=collection");
            return page!.Items.Any(e => e.Id == created.Id) ? (object)true : null;
        }, description: "Resource envelope created by sidecar");

        var grant = await ownerClient.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(created.Id, friend, "viewer", null));
        grant.EnsureSuccessStatusCode();

        var friendClient = _host.CreateClientAs(friend);
        var page = await friendClient.GetFromJsonAsync<ResourceSyncPage>(
            "/api/sharing/resources/sync?types=collection");
        page!.Items.Should().Contain(e => e.Id == created.Id && e.MyRole == "viewer");
    }
}
