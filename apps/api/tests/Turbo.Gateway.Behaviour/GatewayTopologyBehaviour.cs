using System.Net;
using FluentAssertions;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Turbo.Gateway;
using Xunit;

namespace Turbo.Gateway.Behaviour;

/// <summary>
/// The gateway's routing surface is the deploy contract: changing topology
/// without keeping the routes aligned silently breaks the front door. These
/// tests probe each topology purely through HTTP — a route the gateway
/// recognises but whose backend is unreachable returns 502 / 503 / 504
/// (BadGateway / ServiceUnavailable / GatewayTimeout); a route it does not
/// recognise returns 404. Asserting on the specific upstream-failure codes,
/// rather than "anything but 404", catches the case where a misconfigured
/// gateway returns 500 from its own pipeline.
/// </summary>
public sealed class GatewayTopologyBehaviour
{
    private static readonly HttpStatusCode[] UpstreamUnreachable =
    {
        HttpStatusCode.BadGateway,         // 502 — YARP could not connect
        HttpStatusCode.ServiceUnavailable, // 503 — cluster has no healthy destinations
        HttpStatusCode.GatewayTimeout,     // 504 — upstream timed out
    };

    private static WebApplicationFactory<GatewayProgram> BuildGateway(string topology) =>
        new WebApplicationFactory<GatewayProgram>().WithWebHostBuilder(builder =>
        {
            builder.UseSetting("Topology", topology);
        });

    [Theory]
    [InlineData("/api/auth/login")]
    [InlineData("/api/geo/locations")]
    [InlineData("/api/tracks/Tracks")]
    [InlineData("/api/collections/Collections")]
    [InlineData("/api/activities/summaries/bbox")]
    public async Task modulith_topology_routes_every_module_prefix(string path)
    {
        using var factory = BuildGateway("Modulith");
        using var client = factory.CreateClient();

        var response = await client.GetAsync(path);

        response.StatusCode.Should().BeOneOf(UpstreamUnreachable,
            "the gateway must register a route for {0} under Modulith and forward to an upstream — got {1}", path, response.StatusCode);
    }

    [Theory]
    [InlineData("/api/auth/login")]
    [InlineData("/api/geo/locations")]
    [InlineData("/api/tracks/Tracks")]
    [InlineData("/api/collections/Collections")]
    [InlineData("/api/activities/summaries/bbox")]
    public async Task microservices_topology_routes_every_module_prefix(string path)
    {
        using var factory = BuildGateway("Microservices");
        using var client = factory.CreateClient();

        var response = await client.GetAsync(path);

        response.StatusCode.Should().BeOneOf(UpstreamUnreachable,
            "the gateway must register a route for {0} under Microservices and forward to an upstream — got {1}", path, response.StatusCode);
    }

    [Fact]
    public async Task healthz_returns_200_without_topology_routing()
    {
        foreach (var topology in new[] { "Modulith", "Microservices" })
        {
            using var factory = BuildGateway(topology);
            using var client = factory.CreateClient();

            var response = await client.GetAsync("/healthz");

            response.StatusCode.Should().Be(HttpStatusCode.OK,
                "the gateway must serve its own /healthz directly — topology {0}", topology);
        }
    }

    [Fact]
    public async Task unknown_paths_return_404_under_either_topology()
    {
        foreach (var topology in new[] { "Modulith", "Microservices" })
        {
            using var factory = BuildGateway(topology);
            using var client = factory.CreateClient();

            var response = await client.GetAsync("/api/nonexistent/endpoint");

            response.StatusCode.Should().Be(HttpStatusCode.NotFound,
                "the gateway must not invent routes — topology {0}", topology);
        }
    }
}
