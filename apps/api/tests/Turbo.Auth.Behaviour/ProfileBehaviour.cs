using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.UseCases.Queries.ValidateSession;
using Xunit;

namespace Turbo.Auth.Behaviour;

[Collection("AuthHost")]
public sealed class ProfileBehaviour
{
    private readonly AuthHostFixture _host;
    public ProfileBehaviour(AuthHostFixture host) => _host = host;

    private static string FreshEmail() => $"user-{Guid.NewGuid():N}@example.com";
    private const string Password = "Sufficiently-Long-Passw0rd!";

    private async Task<(HttpClient client, AuthTokenResponse tokens)> RegisterAndAuthenticate()
    {
        var client = _host.CreateClient();
        var email = FreshEmail();
        var register = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, Password, Password));
        register.IsSuccessStatusCode.Should().BeTrue(
            $"register should succeed, got {register.StatusCode}: {await register.Content.ReadAsStringAsync()}");
        var tokens = await register.Content.ReadFromJsonAsync<AuthTokenResponse>();
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", tokens!.AccessToken);
        return (client, tokens);
    }

    [Fact]
    public async Task changing_password_then_logging_in_with_the_new_password_succeeds()
    {
        var (client, tokens) = await RegisterAndAuthenticate();
        const string newPassword = "An-Even-Better-Passw0rd!";

        var change = await client.PostAsJsonAsync("/api/auth/auth/change-password",
            new ChangePasswordRequest(Password, newPassword, newPassword));
        change.StatusCode.Should().Be(HttpStatusCode.NoContent,
            await change.Content.ReadAsStringAsync());

        var loginNew = await client.PostAsJsonAsync("/api/auth/auth/login",
            new LoginUserWithPasswordRequest(tokens.Email, newPassword));
        loginNew.IsSuccessStatusCode.Should().BeTrue();

        var loginOld = await client.PostAsJsonAsync("/api/auth/auth/login",
            new LoginUserWithPasswordRequest(tokens.Email, Password));
        loginOld.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }

    [Fact]
    public async Task changing_password_with_the_wrong_current_password_is_unauthorized()
    {
        var (client, _) = await RegisterAndAuthenticate();

        var change = await client.PostAsJsonAsync("/api/auth/auth/change-password",
            new ChangePasswordRequest("not-the-current-password", "An-Even-Better-Passw0rd!", "An-Even-Better-Passw0rd!"));

        change.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }

    [Fact]
    public async Task changing_password_with_mismatched_confirmation_is_bad_request()
    {
        var (client, _) = await RegisterAndAuthenticate();

        var change = await client.PostAsJsonAsync("/api/auth/auth/change-password",
            new ChangePasswordRequest(Password, "An-Even-Better-Passw0rd!", "different-confirmation"));

        change.StatusCode.Should().Be(HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task changing_password_without_authentication_is_unauthorized()
    {
        var client = _host.CreateClient();

        var change = await client.PostAsJsonAsync("/api/auth/auth/change-password",
            new ChangePasswordRequest(Password, "An-Even-Better-Passw0rd!", "An-Even-Better-Passw0rd!"));

        change.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }

    [Fact]
    public async Task updating_the_display_name_is_reflected_by_session_me()
    {
        var (client, _) = await RegisterAndAuthenticate();

        var update = await client.PutAsJsonAsync("/api/auth/profile",
            new UpdateProfileRequest("Sigmund the Explorer"));
        update.IsSuccessStatusCode.Should().BeTrue(await update.Content.ReadAsStringAsync());

        var profile = await update.Content.ReadFromJsonAsync<ProfileResponse>();
        profile!.DisplayName.Should().Be("Sigmund the Explorer");

        var me = await client.GetAsync("/api/auth/session/me");
        me.IsSuccessStatusCode.Should().BeTrue();
        var session = await me.Content.ReadFromJsonAsync<ValidateSessionResponse>();
        session!.DisplayName.Should().Be("Sigmund the Explorer");
    }

    [Fact]
    public async Task a_blank_display_name_clears_it()
    {
        var (client, _) = await RegisterAndAuthenticate();

        await client.PutAsJsonAsync("/api/auth/profile", new UpdateProfileRequest("Temporary Name"));

        var cleared = await client.PutAsJsonAsync("/api/auth/profile", new UpdateProfileRequest("   "));
        cleared.IsSuccessStatusCode.Should().BeTrue();
        var profile = await cleared.Content.ReadFromJsonAsync<ProfileResponse>();
        profile!.DisplayName.Should().BeNull();
    }

    [Fact]
    public async Task an_over_long_display_name_is_rejected_as_bad_request()
    {
        var (client, _) = await RegisterAndAuthenticate();

        var update = await client.PutAsJsonAsync("/api/auth/profile",
            new UpdateProfileRequest(new string('a', 200)));

        update.StatusCode.Should().Be(HttpStatusCode.BadRequest);
    }
}
