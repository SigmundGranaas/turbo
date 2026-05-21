using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Contracts.V1.Tokens;
using Xunit;

namespace Turbo.Auth.Behaviour;

[Collection("AuthHost")]
public sealed class AuthBehaviour
{
    private readonly AuthHostFixture _host;
    public AuthBehaviour(AuthHostFixture host) => _host = host;

    private static string FreshEmail() => $"user-{Guid.NewGuid():N}@example.com";
    private const string Password = "Sufficiently-Long-Passw0rd!";

    [Fact]
    public async Task registering_with_a_fresh_email_returns_a_usable_token_pair()
    {
        var client = _host.CreateClient();
        var email = FreshEmail();

        var response = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, Password, Password));

        response.IsSuccessStatusCode.Should().BeTrue(
            $"register should succeed, got {response.StatusCode}: {await response.Content.ReadAsStringAsync()}");

        var tokens = await response.Content.ReadFromJsonAsync<AuthTokenResponse>();
        tokens.Should().NotBeNull();
        tokens!.AccessToken.Should().NotBeNullOrWhiteSpace();
        tokens.RefreshToken.Should().NotBeNullOrWhiteSpace();
        tokens.Email.Should().Be(email);
    }

    [Fact]
    public async Task registering_an_email_that_already_exists_is_rejected_as_conflict()
    {
        var client = _host.CreateClient();
        var email = FreshEmail();
        var request = new RegisterUserWithPasswordRequest(email, Password, Password);

        var first = await client.PostAsJsonAsync("/api/auth/auth/register", request);
        first.IsSuccessStatusCode.Should().BeTrue();

        var duplicate = await client.PostAsJsonAsync("/api/auth/auth/register", request);
        duplicate.StatusCode.Should().Be(HttpStatusCode.Conflict);
    }

    [Fact]
    public async Task registering_with_mismatched_confirmation_is_rejected_as_bad_request()
    {
        var client = _host.CreateClient();
        var email = FreshEmail();

        var response = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, Password, Password + "-different"));

        response.StatusCode.Should().Be(HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task logging_in_with_correct_credentials_issues_a_token_pair()
    {
        var client = _host.CreateClient();
        var email = FreshEmail();
        await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, Password, Password));

        var login = await client.PostAsJsonAsync("/api/auth/auth/login",
            new LoginUserWithPasswordRequest(email, Password));

        login.IsSuccessStatusCode.Should().BeTrue();
        var tokens = await login.Content.ReadFromJsonAsync<AuthTokenResponse>();
        tokens!.AccessToken.Should().NotBeNullOrWhiteSpace();
        tokens.RefreshToken.Should().NotBeNullOrWhiteSpace();
        tokens.Email.Should().Be(email);
    }

    [Fact]
    public async Task logging_in_with_an_incorrect_password_is_rejected_as_unauthorized()
    {
        var client = _host.CreateClient();
        var email = FreshEmail();
        await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, Password, Password));

        var login = await client.PostAsJsonAsync("/api/auth/auth/login",
            new LoginUserWithPasswordRequest(email, "wrong-password"));

        login.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }

    [Fact]
    public async Task a_refresh_token_can_be_exchanged_once_but_not_reused()
    {
        var client = _host.CreateClient();
        var email = FreshEmail();
        var register = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, Password, Password));
        var initial = await register.Content.ReadFromJsonAsync<AuthTokenResponse>();

        var firstRefresh = await client.PostAsJsonAsync("/api/auth/token/refresh",
            new RefreshTokenRequest(initial!.RefreshToken));
        firstRefresh.IsSuccessStatusCode.Should().BeTrue();

        var replay = await client.PostAsJsonAsync("/api/auth/token/refresh",
            new RefreshTokenRequest(initial.RefreshToken));
        replay.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }

    [Fact]
    public async Task revoking_a_refresh_token_prevents_subsequent_refresh()
    {
        var client = _host.CreateClient();
        var email = FreshEmail();
        var register = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, Password, Password));
        var initial = await register.Content.ReadFromJsonAsync<AuthTokenResponse>();

        var revoke = await client.PostAsJsonAsync("/api/auth/token/revoke",
            new RevokeTokenRequest(initial!.RefreshToken));
        revoke.IsSuccessStatusCode.Should().BeTrue();

        var refreshAfterRevoke = await client.PostAsJsonAsync("/api/auth/token/refresh",
            new RefreshTokenRequest(initial.RefreshToken));
        refreshAfterRevoke.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }
}

[CollectionDefinition("AuthHost")]
public sealed class AuthHostCollection : ICollectionFixture<AuthHostFixture> { }
