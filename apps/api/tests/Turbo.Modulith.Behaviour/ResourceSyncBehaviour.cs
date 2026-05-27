using System.Net.Http.Json;
using FluentAssertions;
using Microsoft.Extensions.DependencyInjection;
using Turboapi.Sharing.controller.request;
using Turboapi.Sharing.data;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;
using Xunit;

namespace Turbo.Modulith.Behaviour;

[Collection("ModulithHost")]
public sealed class ResourceSyncBehaviour
{
    private readonly ModulithHostFixture _host;
    public ResourceSyncBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task owned_resource_appears_in_sync_with_owner_role()
    {
        var owner = Guid.NewGuid();
        var rid = await SeedResourceAsync(owner);

        var client = _host.CreateClientAs(owner);
        var page = await client.GetFromJsonAsync<ResourceSyncPage>("/api/sharing/resources/sync");

        page!.Items.Should().Contain(e => e.Id == rid && e.MyRole == "owner");
    }

    [Fact]
    public async Task shared_resource_appears_for_grantee_with_correct_role()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var rid = await SeedResourceAsync(owner);

        var ownerClient = _host.CreateClientAs(owner);
        await ownerClient.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(rid, friend, "editor", null));

        var friendClient = _host.CreateClientAs(friend);
        var page = await friendClient.GetFromJsonAsync<ResourceSyncPage>("/api/sharing/resources/sync");

        page!.Items.Should().Contain(e => e.Id == rid && e.MyRole == "editor");
    }

    [Fact]
    public async Task since_cursor_filters_out_unchanged_resources()
    {
        var owner = Guid.NewGuid();
        var rid = await SeedResourceAsync(owner);
        var client = _host.CreateClientAs(owner);

        var initial = await client.GetFromJsonAsync<ResourceSyncPage>("/api/sharing/resources/sync");
        initial!.Items.Should().Contain(e => e.Id == rid);

        var future = DateTime.UtcNow.AddMinutes(1).ToString("o");
        var next = await client.GetFromJsonAsync<ResourceSyncPage>(
            $"/api/sharing/resources/sync?since={Uri.EscapeDataString(future)}");

        next!.Items.Should().NotContain(e => e.Id == rid);
    }

    [Fact]
    public async Task types_filter_restricts_returned_envelopes()
    {
        var owner = Guid.NewGuid();
        var collectionRid = await SeedResourceAsync(owner, ResourceType.Collection);
        var markerRid = await SeedResourceAsync(owner, ResourceType.Marker);
        var client = _host.CreateClientAs(owner);

        var page = await client.GetFromJsonAsync<ResourceSyncPage>(
            "/api/sharing/resources/sync?types=collection");

        page!.Items.Should().Contain(e => e.Id == collectionRid);
        page.Items.Should().NotContain(e => e.Id == markerRid);
    }

    [Fact]
    public async Task soft_deleted_resource_arrives_with_deleted_true()
    {
        var owner = Guid.NewGuid();
        var rid = await SeedResourceAsync(owner);
        await SoftDeleteAsync(rid);

        var client = _host.CreateClientAs(owner);
        var page = await client.GetFromJsonAsync<ResourceSyncPage>("/api/sharing/resources/sync");

        page!.Items.Should().Contain(e => e.Id == rid && e.Deleted);
    }

    [Fact]
    public async Task group_grant_propagates_to_every_member_in_sync()
    {
        var owner = Guid.NewGuid();
        var memberA = Guid.NewGuid();
        var memberB = Guid.NewGuid();
        var rid = await SeedResourceAsync(owner);

        var ownerClient = _host.CreateClientAs(owner);
        var groupCreate = await ownerClient.PostAsJsonAsync("/api/sharing/groups",
            new CreateGroupRequest("Trip group"));
        var group = (await groupCreate.Content.ReadFromJsonAsync<GroupDto>())!;

        await ownerClient.PostAsJsonAsync($"/api/sharing/groups/{group.Id}/members",
            new GroupMemberRequest(memberA));
        await ownerClient.PostAsJsonAsync($"/api/sharing/groups/{group.Id}/members",
            new GroupMemberRequest(memberB));
        await ownerClient.PostAsJsonAsync("/api/sharing/grants/groups",
            new GrantToGroupRequest(rid, group.Id, "viewer", null));

        foreach (var member in new[] { memberA, memberB })
        {
            var client = _host.CreateClientAs(member);
            var page = await client.GetFromJsonAsync<ResourceSyncPage>("/api/sharing/resources/sync");
            page!.Items.Should().Contain(e => e.Id == rid && e.MyRole == "viewer");
        }
    }

    private async Task<Guid> SeedResourceAsync(Guid ownerId, string type = "collection")
    {
        var id = Guid.NewGuid();
        using var scope = _host.Services.CreateScope();
        var ctx = scope.ServiceProvider.GetRequiredService<SharingReadContext>();
        ctx.Resources.Add(new ResourceEntity
        {
            Id = id,
            Type = type,
            OwnerId = ownerId,
            Visibility = Visibility.Private.ToWire(),
            Version = 1,
            UpdatedAt = DateTime.UtcNow,
        });
        await ctx.SaveChangesAsync();
        return id;
    }

    private async Task SoftDeleteAsync(Guid rid)
    {
        using var scope = _host.Services.CreateScope();
        var ctx = scope.ServiceProvider.GetRequiredService<SharingReadContext>();
        var r = await ctx.Resources.FindAsync(rid);
        r!.DeletedAt = DateTime.UtcNow;
        r.UpdatedAt = DateTime.UtcNow;
        await ctx.SaveChangesAsync();
    }
}
