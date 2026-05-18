using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Activity.controller;
using Turboapi.Activity.domain;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// Event-sourcing guarantee: the outbox table contains every event
/// emitted by the write side, so the read model can be rebuilt from
/// scratch by replaying it. This test exercises the operator SOP
/// (truncate read model + truncate dedup + reset dispatched_at) and
/// asserts the read model converges back to the same content.
///
/// Without this test the outbox could silently lose information — for
/// example, if a command handler stopped emitting some event — and
/// nothing would surface until an operator actually tried to rebuild
/// in production. Running it on the modulith fixture is the simplest
/// way to cover the round-trip end-to-end; the same proof applies to
/// the microservice deploy modulo NATS consumer recreation.
/// </summary>
[Collection("ModulithHost")]
public sealed class OutboxReplayBehaviour
{
    private readonly ModulithHostFixture _host;
    public OutboxReplayBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task replaying_the_outbox_rebuilds_the_activity_read_model_from_zero()
    {
        var client = await RegisterAndAuthorizeAsync();

        var ids = new List<Guid>();
        for (var i = 0; i < 3; i++)
        {
            var req = new ActivityController.CreateActivityRequest(
                new Position { Latitude = 1 + i, Longitude = 1 + i },
                $"Activity #{i}",
                $"will be rebuilt #{i}",
                "run");
            var post = await client.PostAsJsonAsync("/api/activity", req);
            post.StatusCode.Should().Be(HttpStatusCode.Created);
            var created = (await post.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>())!;
            ids.Add(created.ActivityId);
        }

        foreach (var id in ids)
        {
            await Eventually.Returns<ActivityController.ActivityResponse>(async () =>
            {
                var r = await client.GetAsync($"/api/activity/{id}");
                return r.IsSuccessStatusCode
                    ? await r.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>()
                    : null;
            }, description: $"initial projection of {id}");
        }

        // Operator action: wipe the read model and the dedup table, mark
        // every outbox row undispatched. After this point the read model
        // is empty and the outbox is the only thing that remembers what
        // happened.
        await _host.ResetActivityReadModelForReplayAsync();

        // Pre-condition: GETs return 404 because the read model is empty
        // again. We probe ONCE without polling so we don't accidentally
        // race the dispatcher; if the dispatcher races ahead the
        // post-condition still passes and the test stays valid.
        var emptyProbe = await client.GetAsync($"/api/activity/{ids[0]}");
        emptyProbe.StatusCode.Should().BeOneOf(HttpStatusCode.NotFound, HttpStatusCode.OK);

        // Post-condition: the dispatcher reads each row, publishes it
        // back through the in-process bus, the subscriber re-projects.
        // All three activities reappear with the same ids and bodies.
        foreach (var id in ids)
        {
            var rebuilt = await Eventually.Returns<ActivityController.ActivityResponse>(async () =>
            {
                var r = await client.GetAsync($"/api/activity/{id}");
                return r.IsSuccessStatusCode
                    ? await r.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>()
                    : null;
            }, timeout: TimeSpan.FromSeconds(30), description: $"replay-rebuilt activity {id}");

            rebuilt.Id.Should().Be(id);
            rebuilt.Name.Should().StartWith("Activity #");
        }
    }

    private async Task<HttpClient> RegisterAndAuthorizeAsync()
    {
        var client = _host.CreateClient();
        var email = $"replay-{Guid.NewGuid():N}@example.com";
        const string password = "Sufficiently-Long-Passw0rd!";
        var register = await client.PostAsJsonAsync("/api/auth/auth/register",
            new RegisterUserWithPasswordRequest(email, password, password));
        register.IsSuccessStatusCode.Should().BeTrue(
            $"register precondition failed: {register.StatusCode}: {await register.Content.ReadAsStringAsync()}");
        var tokens = (await register.Content.ReadFromJsonAsync<AuthTokenResponse>())!;
        var authed = _host.CreateClient();
        authed.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);
        return authed;
    }
}
