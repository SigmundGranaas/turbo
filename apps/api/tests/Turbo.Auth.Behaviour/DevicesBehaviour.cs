using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Contracts.V1.Notifications;
using Xunit;

namespace Turbo.Auth.Behaviour;

[Collection("AuthHost")]
public sealed class DevicesBehaviour
{
    private readonly AuthHostFixture _host;
    public DevicesBehaviour(AuthHostFixture host) => _host = host;

    private static string FreshEmail() => $"user-{Guid.NewGuid():N}@example.com";
    private const string Password = "Sufficiently-Long-Passw0rd!";

    private async Task<HttpClient> RegisterAndAuthenticate()
    {
        var client = _host.CreateClient();
        var register = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(FreshEmail(), Password, Password));
        register.IsSuccessStatusCode.Should().BeTrue();
        var tokens = await register.Content.ReadFromJsonAsync<AuthTokenResponse>();
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", tokens!.AccessToken);
        return client;
    }

    [Fact]
    public async Task registering_a_device_token_succeeds_and_is_idempotent()
    {
        var client = await RegisterAndAuthenticate();
        var token = $"fcm-{Guid.NewGuid():N}";

        var first = await client.PostAsJsonAsync("/api/auth/devices",
            new RegisterDeviceRequest(token, "android"));
        first.StatusCode.Should().Be(HttpStatusCode.NoContent, await first.Content.ReadAsStringAsync());

        // Re-registering the same token (e.g. on a later launch) must not conflict.
        var again = await client.PostAsJsonAsync("/api/auth/devices",
            new RegisterDeviceRequest(token, "ios"));
        again.StatusCode.Should().Be(HttpStatusCode.NoContent);
    }

    [Fact]
    public async Task registering_with_a_blank_token_is_bad_request()
    {
        var client = await RegisterAndAuthenticate();

        var response = await client.PostAsJsonAsync("/api/auth/devices",
            new RegisterDeviceRequest("", "android"));

        response.StatusCode.Should().Be(HttpStatusCode.BadRequest);
    }

    [Fact]
    public async Task unregistering_a_device_token_succeeds()
    {
        var client = await RegisterAndAuthenticate();
        var token = $"fcm-{Guid.NewGuid():N}";
        await client.PostAsJsonAsync("/api/auth/devices", new RegisterDeviceRequest(token, "android"));

        var unregister = await client.PostAsJsonAsync("/api/auth/devices/unregister",
            new UnregisterDeviceRequest(token));

        unregister.StatusCode.Should().Be(HttpStatusCode.NoContent);
    }

    [Fact]
    public async Task registering_a_device_without_authentication_is_unauthorized()
    {
        var client = _host.CreateClient();

        var response = await client.PostAsJsonAsync("/api/auth/devices",
            new RegisterDeviceRequest("fcm-token", "android"));

        response.StatusCode.Should().Be(HttpStatusCode.Unauthorized);
    }
}
