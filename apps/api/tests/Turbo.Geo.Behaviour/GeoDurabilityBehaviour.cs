using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Turboapi.Geo.controller.request;
using Turboapi.Geo.controller.response;
using Xunit;

namespace Turbo.Geo.Behaviour;

/// <summary>
/// Failure-mode guarantee: when the broker is unreachable at the moment
/// a write commits, the write still succeeds with 201, the read endpoint
/// returns 404 while the broker is down (the projection runs through the
/// subscriber, which depends on the dispatcher), and the read endpoint
/// eventually returns the body after the broker comes back. This is the
/// same shape as the Activity durability test; both modules now run the
/// projection asynchronously through the outbox + transport pipeline.
/// </summary>
[Collection("GeoHost")]
public sealed class GeoDurabilityBehaviour
{
    private readonly GeoHostFixture _host;
    public GeoDurabilityBehaviour(GeoHostFixture host) => _host = host;

    [Fact]
    public async Task creates_succeed_and_become_visible_after_the_broker_recovers()
    {
        var owner = Guid.NewGuid();
        var client = _host.CreateClientAs(owner);
        Guid locationId;

        await _host.PauseBrokerAsync();
        try
        {
            var request = new CreateLocationRequest
            {
                Geometry = new GeometryData { Longitude = -0.1, Latitude = 51.5 },
                Display = new DisplayData { Name = "Recorded during outage", Description = "", Icon = "pin" }
            };
            var create = await client.PostAsJsonAsync("/api/geo/Locations", request);
            create.StatusCode.Should().Be(HttpStatusCode.Created,
                "the write path must commit to the outbox even when the broker is unreachable");
            locationId = (await create.Content.ReadFromJsonAsync<LocationResponse>())!.Id;

            var whilePaused = await client.GetAsync($"/api/geo/Locations/{locationId}");
            whilePaused.StatusCode.Should().Be(HttpStatusCode.NotFound,
                "the projection runs through the broker; with the broker paused the read model cannot catch up");
        }
        finally
        {
            await _host.UnpauseBrokerAsync();
        }

        var recovered = await Eventually.Returns<LocationResponse>(async () =>
        {
            var r = await client.GetAsync($"/api/geo/Locations/{locationId}");
            return r.IsSuccessStatusCode
                ? await r.Content.ReadFromJsonAsync<LocationResponse>()
                : null;
        }, timeout: TimeSpan.FromSeconds(20),
           description: $"GET /api/geo/Locations/{locationId} after broker recovery");

        recovered.Id.Should().Be(locationId);
        recovered.Display.Name.Should().Be("Recorded during outage");
    }
}
