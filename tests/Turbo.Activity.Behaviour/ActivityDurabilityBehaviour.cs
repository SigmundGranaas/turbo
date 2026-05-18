using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Activity.controller;
using Turboapi.Activity.domain;
using Xunit;

namespace Turbo.Activity.Behaviour;

/// <summary>
/// Failure-mode guarantee: when the broker is unreachable at the moment a
/// write commits, the write still succeeds with 2xx and the read endpoint
/// eventually returns the data once the broker comes back. This was the
/// motivation for introducing the transactional outbox in Step 2; against
/// the pre-outbox implementation the POST would fail or time out.
///
/// The test asserts only HTTP-visible outcomes — status codes and the
/// eventual response body. It does not look at outbox rows, dispatcher
/// state, or any internal table.
/// </summary>
[Collection("ActivityHost")]
public sealed class ActivityDurabilityBehaviour
{
    private readonly ActivityHostFixture _host;
    public ActivityDurabilityBehaviour(ActivityHostFixture host) => _host = host;

    [Fact]
    public async Task creates_succeed_and_become_visible_after_the_broker_recovers()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);
        Guid activityId;

        await _host.PauseBrokerAsync();
        try
        {
            var request = new ActivityController.CreateActivityRequest(
                new Position { Latitude = 51.5, Longitude = -0.1 },
                "While broker is down",
                "should still succeed",
                "pin");

            var create = await client.PostAsJsonAsync("/api/activity", request);
            create.StatusCode.Should().Be(HttpStatusCode.Created,
                "the write path must commit to the outbox even when the broker is unreachable");

            var created = await create.Content.ReadFromJsonAsync<ActivityController.CreateActivityResponse>();
            created.Should().NotBeNull();
            activityId = created!.ActivityId;

            var whilePaused = await client.GetAsync($"/api/activity/{activityId}");
            whilePaused.StatusCode.Should().Be(HttpStatusCode.NotFound,
                "the projection cannot catch up while the broker is paused");
        }
        finally
        {
            await _host.UnpauseBrokerAsync();
        }

        var recovered = await Eventually.Returns<ActivityController.ActivityResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/activity/{activityId}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<ActivityController.ActivityResponse>()
                : null;
        }, timeout: TimeSpan.FromSeconds(20),
           description: $"GET /api/activity/{activityId} after broker recovery");

        recovered.Id.Should().Be(activityId);
        recovered.OwnerId.Should().Be(owner);
        recovered.Name.Should().Be("While broker is down");
    }
}
