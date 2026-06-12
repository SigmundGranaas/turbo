using System.Net;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using FluentAssertions;
using Turbo.Host.Places;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P4: the standalone host's defense-in-depth rate limit (the gateway is the
/// primary guard). Builds a second host over the shared fixture's DB with a
/// tiny per-window limit and confirms it returns 429 once exceeded.
/// </summary>
public class PlacesRateLimitBehaviour : IClassFixture<PlacesHostFixture>
{
    private readonly PlacesHostFixture _fixture;

    public PlacesRateLimitBehaviour(PlacesHostFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task Exceeding_the_window_limit_returns_429()
    {
        using var factory = new WebApplicationFactory<PlacesHostProgram>().WithWebHostBuilder(b =>
        {
            b.UseEnvironment("Test");
            b.UseContentRoot(Path.Combine(PlacesHostFixture.FindRepoRoot(), "apps", "api", "hosts", "Turbo.Host.Places"));
            b.UseSetting("ConnectionStrings:Places", _fixture.ConnectionString);
            b.UseSetting("Places:RateLimitPermitPerWindow", "3");
            b.UseSetting("Places:RateLimitWindowSeconds", "60");
        });
        var client = factory.CreateClient();

        var statuses = new List<HttpStatusCode>();
        for (var i = 0; i < 6; i++)
            statuses.Add((await client.GetAsync("/api/places/health")).StatusCode);

        statuses.Should().Contain(HttpStatusCode.OK, "the first requests are within the limit");
        statuses.Should().Contain(HttpStatusCode.TooManyRequests, "requests past the window limit are 429'd");
    }
}
