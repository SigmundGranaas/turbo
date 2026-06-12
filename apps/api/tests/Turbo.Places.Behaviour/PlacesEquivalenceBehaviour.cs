using System.Globalization;
using System.Net;
using System.Text.Json;
using FluentAssertions;
using Turboapi.Places.Core;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// P3d — the keystone consistency gate: a bundle built by the server, opened by
/// the place-core embedded engine, must reverse-geocode identically to the live
/// <c>/api/places/reverse</c>. Both run the same rank() over the same data, so
/// offline == online; this test proves the data layer (PostGIS slice → SQLite
/// R*Tree + polygon rings) doesn't distort that.
/// </summary>
public class PlacesEquivalenceBehaviour : IClassFixture<PlacesHostFixture>
{
    private readonly PlacesHostFixture _fixture;

    public PlacesEquivalenceBehaviour(PlacesHostFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task Embedded_bundle_reverse_equals_server_reverse_over_the_region()
    {
        var client = _fixture.CreateClient();

        // A bbox over the Jotunheimen fixture data (summit + park + kommune);
        // excludes the Tromsø rows.
        var resp = await client.GetAsync("/api/places/bundle?bbox=8.0,61.4,8.6,61.8");
        resp.StatusCode.Should().Be(HttpStatusCode.OK);
        resp.Headers.ETag!.Tag.Should().Be("\"test-fixture\"");

        var path = Path.Combine(Path.GetTempPath(), $"eq-bundle-{Guid.NewGuid():n}.sqlite");
        await File.WriteAllBytesAsync(path, await resp.Content.ReadAsByteArrayAsync());

        var handle = PlaceCore.BundleOpen(path);
        handle.Should().NotBe(IntPtr.Zero, "the server-built bundle must open");
        try
        {
            // Summit (on a peak), wilderness (park containment), and a bare
            // point in the kommune (kommune fallback) — all three cascade paths.
            (double Lat, double Lng)[] points =
            [
                (61.6363, 8.3120),
                (PlacesHostFixture.WildLat, PlacesHostFixture.WildLng),
                (61.60, 8.50),
            ];

            foreach (var (lat, lng) in points)
            {
                var server = await ServerReverseAsync(client, lat, lng);
                var bundle = BundleReverse(handle, lat, lng);

                bundle.Should().Be(server,
                    $"offline must equal online at ({lat.ToString(CultureInfo.InvariantCulture)}, " +
                    $"{lng.ToString(CultureInfo.InvariantCulture)})");
            }
        }
        finally
        {
            PlaceCore.BundleFree(handle);
            File.Delete(path);
        }
    }

    /// <summary>The identity fields of a reverse result, source-agnostic.</summary>
    private record Description(string? Title, string? Qualifier, string? Secondary, string? Kommune, string? Fylke);

    private static async Task<Description?> ServerReverseAsync(HttpClient client, double lat, double lng)
    {
        var url = string.Create(CultureInfo.InvariantCulture, $"/api/places/reverse?lat={lat}&lon={lng}");
        var resp = await client.GetAsync(url);
        if (resp.StatusCode == HttpStatusCode.NotFound) return null;
        resp.EnsureSuccessStatusCode();
        var d = JsonSerializer.Deserialize<JsonElement>(await resp.Content.ReadAsStringAsync());
        return new Description(
            Str(d, "title"), Str(d, "qualifier"), Str(d, "secondary"), Str(d, "kommune"), Str(d, "fylke"));
    }

    private static Description? BundleReverse(IntPtr handle, double lat, double lng)
    {
        var json = PlaceCore.BundleReverseJson(handle, lat, lng);
        if (json == "null") return null;
        var d = JsonSerializer.Deserialize<JsonElement>(json);
        return new Description(
            Str(d, "title"), Str(d, "qualifier"), Str(d, "secondary"), Str(d, "kommune"), Str(d, "fylke"));
    }

    private static string? Str(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
}
