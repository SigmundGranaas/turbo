using System.Net;
using System.Net.Http.Json;
using FluentAssertions;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// Dataset-versioning contract (P0.1): the ETag is the active publication
/// version from <c>places.dataset</c>, served by a cached 1-row lookup — not a
/// per-request <c>max(dataset_version)</c> over the (eventually million-row)
/// places table. Pinning it via the dataset table also proves the version is
/// decoupled from row content, which is what makes the P1 atomic swap correct.
/// </summary>
public class PlacesVersioningBehaviour : IClassFixture<PlacesHostFixture>
{
    private readonly PlacesHostFixture _fixture;

    public PlacesVersioningBehaviour(PlacesHostFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task ETag_is_the_dataset_table_active_version_not_the_max_row_version()
    {
        // Publish a new active version that exists on NO places row. A
        // max(places.dataset_version) implementation returns "test-fixture";
        // the dataset-table source returns "v-next".
        await _fixture.PublishDatasetVersionAsync("v-next");

        var client = _fixture.CreateClient();
        var resp = await client.GetAsync("/api/places/reverse?lat=61.6363&lon=8.3120");

        resp.StatusCode.Should().Be(HttpStatusCode.OK);
        resp.Headers.ETag!.Tag.Should().Be("\"v-next\"",
            "the ETag must reflect the active dataset publication, independent of row content");

        // Health reports the same active version.
        var health = await client.GetFromJsonAsync<System.Text.Json.JsonElement>("/api/places/health");
        health.GetProperty("datasetVersion").GetString().Should().Be("v-next");
    }

    [Fact]
    public async Task Publishing_a_newer_version_supersedes_the_prior_active_one()
    {
        await _fixture.PublishDatasetVersionAsync("v-a");
        await _fixture.PublishDatasetVersionAsync("v-b");

        var client = _fixture.CreateClient();
        var resp = await client.GetAsync("/api/places/reverse?lat=61.6363&lon=8.3120");

        resp.Headers.ETag!.Tag.Should().Be("\"v-b\"",
            "exactly one version is active at a time; the latest publish wins");
    }
}
