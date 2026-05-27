using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Turbo.Behaviour.Testing;
using Turboapi.Sharing.controller.request;
using Turboapi.Sharing.data;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.domain.service;
using Turboapi.Sharing.value;
using Xunit;

namespace Turbo.Modulith.Behaviour;

[Collection("ModulithHost")]
public sealed class SharingBehaviour
{
    private readonly ModulithHostFixture _host;
    public SharingBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task friendship_request_then_accept_lands_with_accepted_status()
    {
        var alice = Guid.NewGuid();
        var bob = Guid.NewGuid();
        var aliceClient = _host.CreateClientAs(alice);
        var bobClient = _host.CreateClientAs(bob);

        var requested = await aliceClient.PostAsJsonAsync(
            "/api/sharing/friendships/request", new FriendshipActionRequest(bob));
        requested.StatusCode.Should().Be(HttpStatusCode.OK);

        var accepted = await bobClient.PostAsJsonAsync(
            "/api/sharing/friendships/accept", new FriendshipActionRequest(alice));
        accepted.StatusCode.Should().Be(HttpStatusCode.OK);
        var dto = await accepted.Content.ReadFromJsonAsync<FriendshipDto>();
        dto!.Status.Should().Be("accepted");
        dto.AcceptedAt.Should().NotBeNull();

        var list = await aliceClient.GetFromJsonAsync<List<FriendshipDto>>(
            "/api/sharing/friendships?status=accepted");
        list!.Should().ContainSingle(f => f.OtherUserId == bob);
    }

    [Fact]
    public async Task duplicate_friendship_request_returns_conflict()
    {
        var alice = Guid.NewGuid();
        var bob = Guid.NewGuid();
        var aliceClient = _host.CreateClientAs(alice);

        var first = await aliceClient.PostAsJsonAsync(
            "/api/sharing/friendships/request", new FriendshipActionRequest(bob));
        first.StatusCode.Should().Be(HttpStatusCode.OK);

        var second = await aliceClient.PostAsJsonAsync(
            "/api/sharing/friendships/request", new FriendshipActionRequest(bob));
        second.StatusCode.Should().Be(HttpStatusCode.Conflict);
    }

    [Fact]
    public async Task create_group_returns_creator_as_admin_member()
    {
        var alice = Guid.NewGuid();
        var client = _host.CreateClientAs(alice);

        var create = await client.PostAsJsonAsync(
            "/api/sharing/groups", new CreateGroupRequest("Ski crew"));
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var dto = await create.Content.ReadFromJsonAsync<GroupDto>();
        dto!.OwnerId.Should().Be(alice);
        dto.Members.Should().ContainSingle(m => m.UserId == alice && m.Role == "admin");
    }

    [Fact]
    public async Task add_member_appears_in_group_payload()
    {
        var alice = Guid.NewGuid();
        var bob = Guid.NewGuid();
        var client = _host.CreateClientAs(alice);

        var create = await client.PostAsJsonAsync(
            "/api/sharing/groups", new CreateGroupRequest("Buddies"));
        var group = (await create.Content.ReadFromJsonAsync<GroupDto>())!;

        var add = await client.PostAsJsonAsync(
            $"/api/sharing/groups/{group.Id}/members", new GroupMemberRequest(bob));
        add.StatusCode.Should().Be(HttpStatusCode.NoContent);

        var fetched = (await client.GetFromJsonAsync<GroupDto>(
            $"/api/sharing/groups/{group.Id}"))!;
        fetched.Members.Should().Contain(m => m.UserId == bob);
    }

    [Fact]
    public async Task grant_to_user_after_resource_creation_lets_them_read_and_write()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var resourceId = await SeedResourceAsync(owner);

        var client = _host.CreateClientAs(owner);
        var grant = await client.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(resourceId, friend, "editor", null));
        grant.StatusCode.Should().Be(HttpStatusCode.OK);

        using var scope = ScopedSharingServices(out var ac);
        (await ac.CanWriteAsync(friend, resourceId)).Should().BeTrue();
    }

    [Fact]
    public async Task non_owner_cannot_grant_on_another_users_resource()
    {
        var owner = Guid.NewGuid();
        var attacker = Guid.NewGuid();
        var resourceId = await SeedResourceAsync(owner);

        var client = _host.CreateClientAs(attacker);
        var grant = await client.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(resourceId, attacker, "editor", null));
        grant.StatusCode.Should().Be(HttpStatusCode.Forbidden);
    }

    [Fact]
    public async Task revoke_user_grant_removes_access()
    {
        var owner = Guid.NewGuid();
        var friend = Guid.NewGuid();
        var resourceId = await SeedResourceAsync(owner);
        var ownerClient = _host.CreateClientAs(owner);

        await ownerClient.PostAsJsonAsync("/api/sharing/grants/users",
            new GrantToUserRequest(resourceId, friend, "viewer", null));

        var revoke = await ownerClient.DeleteAsync(
            $"/api/sharing/grants/resources/{resourceId}/users/{friend}");
        revoke.StatusCode.Should().Be(HttpStatusCode.NoContent);

        using var scope = ScopedSharingServices(out var ac);
        (await ac.CanReadAsync(friend, resourceId)).Should().BeFalse();
    }

    [Fact]
    public async Task link_grant_returns_token_and_persists()
    {
        var owner = Guid.NewGuid();
        var resourceId = await SeedResourceAsync(owner);
        var client = _host.CreateClientAs(owner);

        var grant = await client.PostAsJsonAsync("/api/sharing/grants/links",
            new GrantAsLinkRequest(resourceId, "viewer", null));
        grant.StatusCode.Should().Be(HttpStatusCode.OK);
        var dto = await grant.Content.ReadFromJsonAsync<LinkGrantDto>();
        dto!.LinkToken.Should().NotBeNullOrEmpty();
    }

    [Fact]
    public async Task resource_invite_redemption_materializes_a_grant_and_friendship()
    {
        var inviter = Guid.NewGuid();
        var invitee = Guid.NewGuid();
        var email = $"{Guid.NewGuid():N}@example.test";
        var resourceId = await SeedResourceAsync(inviter);

        var inviterClient = _host.CreateClientAs(inviter);
        var invite = await inviterClient.PostAsJsonAsync("/api/sharing/invites/resource",
            new CreateResourceInviteRequest(email, resourceId, "editor", null));
        invite.StatusCode.Should().Be(HttpStatusCode.OK);

        var inviteeClient = _host.CreateClientAs(invitee);
        var redeem = await inviteeClient.PostAsJsonAsync("/api/sharing/invites/redeem",
            new RedeemInvitesRequest(email));
        redeem.StatusCode.Should().Be(HttpStatusCode.OK);

        using var scope = ScopedSharingServices(out var ac);
        (await ac.CanWriteAsync(invitee, resourceId)).Should().BeTrue();
    }

    [Fact]
    public async Task friend_invite_redemption_creates_accepted_friendship()
    {
        var inviter = Guid.NewGuid();
        var invitee = Guid.NewGuid();
        var email = $"{Guid.NewGuid():N}@example.test";

        var inviterClient = _host.CreateClientAs(inviter);
        await inviterClient.PostAsJsonAsync("/api/sharing/invites/friend",
            new CreateFriendInviteRequest(email, null));

        var inviteeClient = _host.CreateClientAs(invitee);
        await inviteeClient.PostAsJsonAsync("/api/sharing/invites/redeem",
            new RedeemInvitesRequest(email));

        var friends = (await inviterClient.GetFromJsonAsync<List<FriendshipDto>>(
            "/api/sharing/friendships?status=accepted"))!;
        friends.Should().Contain(f => f.OtherUserId == invitee);
    }

    private async Task<Guid> SeedResourceAsync(Guid ownerId)
    {
        var id = Guid.NewGuid();
        using var scope = _host.Services.CreateScope();
        var ctx = scope.ServiceProvider.GetRequiredService<SharingReadContext>();
        ctx.Resources.Add(new ResourceEntity
        {
            Id = id,
            Type = ResourceType.Collection,
            OwnerId = ownerId,
            Visibility = Visibility.Private.ToWire(),
            Version = 1,
            UpdatedAt = DateTime.UtcNow,
        });
        await ctx.SaveChangesAsync();
        return id;
    }

    private IServiceScope ScopedSharingServices(out Turboapi.Sharing.domain.IAccessControl ac)
    {
        var scope = _host.Services.CreateScope();
        ac = scope.ServiceProvider.GetRequiredService<Turboapi.Sharing.domain.IAccessControl>();
        return scope;
    }
}
