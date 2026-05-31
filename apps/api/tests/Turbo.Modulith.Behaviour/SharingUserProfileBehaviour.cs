using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Sharing.controller;
using Turboapi.Sharing.domain.service;
using Xunit;

namespace Turbo.Modulith.Behaviour;

[Collection("ModulithHost")]
public sealed class SharingUserProfileBehaviour
{
    private readonly ModulithHostFixture _host;
    public SharingUserProfileBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task me_profile_generates_a_friend_code_on_first_read()
    {
        var user = Guid.NewGuid();
        var client = _host.CreateClientAs(user);

        var profile = await client.GetFromJsonAsync<UserProfileDto>("/api/sharing/me/profile");
        profile!.UserId.Should().Be(user);
        profile.FriendCode.Should().NotBeNullOrEmpty();
        profile.FriendCode.Should().MatchRegex("^[a-z0-9]+$"); // curated lowercase alphabet
    }

    [Fact]
    public async Task me_profile_returns_the_same_code_on_subsequent_reads()
    {
        var user = Guid.NewGuid();
        var client = _host.CreateClientAs(user);

        var first = await client.GetFromJsonAsync<UserProfileDto>("/api/sharing/me/profile");
        var second = await client.GetFromJsonAsync<UserProfileDto>("/api/sharing/me/profile");

        second!.FriendCode.Should().Be(first!.FriendCode);
    }

    [Fact]
    public async Task lookup_by_code_resolves_a_friend_code_to_a_user_id()
    {
        var alice = Guid.NewGuid();
        var bob = Guid.NewGuid();
        var aliceClient = _host.CreateClientAs(alice);
        var bobClient = _host.CreateClientAs(bob);

        // Ensure both profiles exist.
        var aliceProfile = await aliceClient.GetFromJsonAsync<UserProfileDto>("/api/sharing/me/profile");

        // Bob looks up Alice by her friend code.
        var resp = await bobClient.GetAsync($"/api/sharing/users/lookup?code={aliceProfile!.FriendCode}");
        resp.StatusCode.Should().Be(HttpStatusCode.OK);
        var dto = (await resp.Content.ReadFromJsonAsync<UserLookupResponse>())!;
        dto.UserId.Should().Be(alice);
    }

    [Fact]
    public async Task lookup_strips_turbo_prefix()
    {
        var alice = Guid.NewGuid();
        var bob = Guid.NewGuid();
        var aliceClient = _host.CreateClientAs(alice);
        var bobClient = _host.CreateClientAs(bob);

        var aliceProfile = await aliceClient.GetFromJsonAsync<UserProfileDto>("/api/sharing/me/profile");

        var resp = await bobClient.GetAsync(
            $"/api/sharing/users/lookup?code=turbo-{aliceProfile!.FriendCode}");
        resp.StatusCode.Should().Be(HttpStatusCode.OK);
    }

    [Fact]
    public async Task lookup_returns_404_for_an_unknown_code()
    {
        var user = Guid.NewGuid();
        var client = _host.CreateClientAs(user);

        var resp = await client.GetAsync("/api/sharing/users/lookup?code=zzzzzzz");
        resp.StatusCode.Should().Be(HttpStatusCode.NotFound);
    }
}
