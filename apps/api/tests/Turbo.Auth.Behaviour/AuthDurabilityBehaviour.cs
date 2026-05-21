using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Xunit;

namespace Turbo.Auth.Behaviour;

/// <summary>
/// Failure-mode guarantee: when the broker is unreachable at the moment a
/// write commits, register and login still succeed with 2xx. Before
/// Step 2 the pre-commit publish would either fail the request or leak
/// events into Kafka for state that had not been persisted; after the
/// outbox is in place the publish is decoupled from the request path.
/// </summary>
[Collection("AuthHost")]
public sealed class AuthDurabilityBehaviour
{
    private readonly AuthHostFixture _host;
    public AuthDurabilityBehaviour(AuthHostFixture host) => _host = host;

    [Fact]
    public async Task register_and_login_succeed_while_the_broker_is_paused()
    {
        var client = _host.CreateClient();
        var email = $"durable-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";

        await _host.PauseBrokerAsync();
        try
        {
            var register = await client.PostAsJsonAsync("/api/auth/auth/register",
                new RegisterUserWithPasswordRequest(email, password, password));
            register.IsSuccessStatusCode.Should().BeTrue(
                $"register must commit through the outbox even when the broker is unreachable, got {register.StatusCode}: {await register.Content.ReadAsStringAsync()}");

            var login = await client.PostAsJsonAsync("/api/auth/auth/login",
                new LoginUserWithPasswordRequest(email, password));
            login.IsSuccessStatusCode.Should().BeTrue(
                "login reads from the DB and writes audit events through the outbox; neither depends on the broker being up");
        }
        finally
        {
            await _host.UnpauseBrokerAsync();
        }

        // After recovery the same credentials still work — confirming the
        // account state survived the outage without being rolled back.
        var loginAfterRecovery = await client.PostAsJsonAsync("/api/auth/auth/login",
            new LoginUserWithPasswordRequest(email, password));
        loginAfterRecovery.IsSuccessStatusCode.Should().BeTrue();
    }
}
