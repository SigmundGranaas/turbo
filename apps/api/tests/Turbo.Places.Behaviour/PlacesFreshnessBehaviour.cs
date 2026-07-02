using FluentAssertions;
using Turboapi.Places.Ingestion;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// Freshness gate: the ingest must skip the ~3 GB Geonorge order + download when
/// SSR is unchanged. That decision compares the upstream <c>DateUpdated</c>
/// (parsed from a cheap metadata GET) against the active dataset's stored
/// <c>source_version</c>. These cover the two halves: parsing the marker, and the
/// store round-trip that persists/reads it.
/// </summary>
public class PlacesFreshnessParsing
{
    [Fact]
    public void Parses_DateUpdated_preferring_the_data_date_over_metadata_date()
    {
        var json = """
            { "Title": "Stedsnavn",
              "DateUpdated": "2026-06-27T00:00:00",
              "DateMetadataUpdated": "2026-07-02T00:00:00" }
            """;
        GeonorgeClient.ParseDatasetVersion(json).Should().Be("2026-06-27T00:00:00");
    }

    [Fact]
    public void Falls_back_to_metadata_date_when_data_date_absent()
    {
        var json = """{ "DateMetadataUpdated": "2026-07-02T00:00:00" }""";
        GeonorgeClient.ParseDatasetVersion(json).Should().Be("2026-07-02T00:00:00");
    }

    [Fact]
    public void Returns_null_when_no_date_is_present()
    {
        GeonorgeClient.ParseDatasetVersion("""{ "Title": "Stedsnavn" }""").Should().BeNull();
    }
}

/// <summary>Store round-trip for the <c>source_version</c> provenance column the
/// freshness gate reads.</summary>
public class PlacesSourceVersionRoundTrip : IClassFixture<PlacesDbFixture>
{
    private readonly PlacesDbFixture _fixture;

    public PlacesSourceVersionRoundTrip(PlacesDbFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task Active_source_version_is_persisted_and_read_back()
    {
        var store = _fixture.Store;

        // No publication yet → no source version to compare against.
        (await store.GetActiveSourceVersionAsync()).Should().BeNull();

        await store.PublishDatasetVersionAsync("bulk-ssr-1", sourceVersion: "2026-06-27T00:00:00");
        (await store.GetActiveSourceVersionAsync()).Should().Be(
            "2026-06-27T00:00:00", "the ingest reads this to skip an unchanged re-ingest");

        // A newer publication supersedes and updates the marker.
        await store.PublishDatasetVersionAsync("bulk-ssr-2", sourceVersion: "2026-07-10T00:00:00");
        (await store.GetActiveSourceVersionAsync()).Should().Be("2026-07-10T00:00:00");
        (await store.GetActiveDatasetVersionAsync()).Should().Be("bulk-ssr-2");
    }
}
