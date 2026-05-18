using System.Net.Http.Headers;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Activity.controller;
using Turboapi.Activity.domain;
using Xunit;

namespace Turbo.Modulith.Behaviour;

/// <summary>
/// At-least-once delivery guarantee: an envelope delivered twice does
/// NOT produce two rows in the read model. The idempotency table catches
/// the duplicate <c>event_id</c> and the projection skips.
///
/// HTTP GET-by-id can't distinguish "one row" from "two rows" — both
/// return 200 with the body of the first row found. We therefore do
/// assert a SELECT COUNT(*) here, which bends the test philosophy in the
/// same way <c>PauseBrokerAsync</c> bends it: this is a failure-mode
/// test of an invariant that has no clean public surface.
/// </summary>
[Collection("ModulithHost")]
public sealed class IdempotencyBehaviour
{
    private readonly ModulithHostFixture _host;
    public IdempotencyBehaviour(ModulithHostFixture host) => _host = host;

    [Fact]
    public async Task duplicate_delivery_of_the_same_envelope_produces_exactly_one_read_model_row()
    {
        var client = await RegisterAndAuthorizeAsync();

        var request = new ActivityController.CreateActivityRequest(
            new Position { Latitude = 1, Longitude = 2 },
            $"Idempotent activity {Guid.NewGuid():N}",
            "delivered twice",
            "icon");

        var create = await client.PostAsJsonAsync("/api/activity", request);
        create.IsSuccessStatusCode.Should().BeTrue();
        var created = (await create.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>())!;

        // Wait for the natural in-process delivery to land. The dedup row
        // for this event id is now present.
        await Eventually.Returns<ActivityController.ActivityResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{created.ActivityId}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>()
                : null;
        }, description: "first delivery projects");

        (await _host.CountActivityRowsAsync(created.ActivityId))
            .Should().Be(1, "the first projection inserts exactly one row");

        // Simulate a broker redelivery: pull the original envelope out of
        // the outbox (with the SAME event id) and republish it through
        // the in-process bus. Without dedup the projection would either
        // insert a second row OR throw a PK violation on insert.
        await _host.RedeliverLatestActivityEnvelopeAsync(nameof(Turboapi.Activity.domain.events.ActivityCreated));

        // Give the bus + subscriber time to dispatch the redelivered
        // envelope. 500ms is generous — the bus loop tick is well under
        // that.
        await Task.Delay(500);

        (await _host.CountActivityRowsAsync(created.ActivityId))
            .Should().Be(1, "the dedup table must catch the duplicate event id; at-least-once delivery must not corrupt the read model");

        // And the public GET still returns the original body — no
        // transient error leaked out to the caller.
        var fetch = await client.GetAsync($"/api/activity/{created.ActivityId}");
        fetch.IsSuccessStatusCode.Should().BeTrue();
        var body = await fetch.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>();
        body!.Name.Should().Be(request.Name);
    }

    private async Task<HttpClient> RegisterAndAuthorizeAsync()
    {
        var client = _host.CreateClient();
        var email = $"dedup-{Guid.NewGuid():N}@example.com";
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
