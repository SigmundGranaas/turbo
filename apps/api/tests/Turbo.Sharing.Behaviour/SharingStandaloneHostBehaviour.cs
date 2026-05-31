using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Sharing.controller.request;
using Turboapi.Sharing.domain.service;
using Xunit;

namespace Turbo.Sharing.Behaviour;

/// <summary>
/// End-to-end tests against the dedicated Turbo.Host.Sharing process —
/// the deploy shape where Sharing runs as its own service rather than
/// as a module inside Turbo.Host.Modulith. Verifies that DI wiring,
/// migrations, JWT authentication, and the HTTP surface all work in
/// the standalone topology.
/// </summary>
[Collection("SharingHost")]
public sealed class SharingStandaloneHostBehaviour
{
    private readonly SharingHostFixture _host;
    public SharingStandaloneHostBehaviour(SharingHostFixture host) => _host = host;

    [Fact]
    public async Task standalone_host_serves_friendship_request_endpoint()
    {
        var alice = Guid.NewGuid();
        var bob = Guid.NewGuid();
        var client = _host.CreateClientAs(alice);

        var response = await client.PostAsJsonAsync(
            "/api/sharing/friendships/request",
            new FriendshipActionRequest(bob));
        response.StatusCode.Should().Be(HttpStatusCode.OK);

        var listed = await client.GetFromJsonAsync<List<FriendshipDto>>(
            "/api/sharing/friendships");
        listed!.Should().ContainSingle(f => f.OtherUserId == bob);
    }

    [Fact]
    public async Task standalone_host_serves_groups_endpoint()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);

        var create = await client.PostAsJsonAsync(
            "/api/sharing/groups",
            new CreateGroupRequest("Travel buddies"));
        create.StatusCode.Should().Be(HttpStatusCode.Created);
        var dto = (await create.Content.ReadFromJsonAsync<GroupDto>())!;
        dto.OwnerId.Should().Be(owner);
        dto.Name.Should().Be("Travel buddies");
    }

    [Fact]
    public async Task standalone_host_unauthenticated_friendships_returns_401()
    {
        var client = _host.CreateClient();
        var response = await client.GetAsync("/api/sharing/friendships");
        response.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }

    [Fact]
    public async Task standalone_host_healthz_is_open()
    {
        var client = _host.CreateClient();
        var response = await client.GetAsync("/healthz");
        response.StatusCode.Should().Be(HttpStatusCode.OK);
    }
}

[CollectionDefinition("SharingHost")]
public sealed class SharingHostCollection : ICollectionFixture<SharingHostFixture> { }
