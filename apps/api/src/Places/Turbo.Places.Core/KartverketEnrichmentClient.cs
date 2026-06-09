using System.Globalization;
using System.Text.Json;

namespace Turboapi.Places.Core;

/// <summary>
/// Precomputes per-feature enrichment from Kartverket at ingest time:
/// elevation (Høydedata <c>/punkt</c>) and containing kommune/fylke
/// (Kommuneinfo <c>/punkt</c>). The values are stored on the place row, so the
/// reverse path serves them from our own data with no third-party call at
/// query time. National ingestion (M4) swaps these per-point calls for a DTM
/// raster + an admin-polygon spatial join.
/// </summary>
public sealed class KartverketEnrichmentClient
{
    private const string ElevationEndpoint = "https://ws.geonorge.no/hoydedata/v1/punkt";
    private const string KommuneEndpoint = "https://ws.geonorge.no/kommuneinfo/v1/punkt";

    // place-core's elevation sanity bounds; reject anything outside.
    private const double MinElevationM = -1000;
    private const double MaxElevationM = 9000;

    private readonly HttpClient _http;

    public KartverketEnrichmentClient(HttpClient http) => _http = http;

    public async Task<double?> ElevationAsync(double lat, double lng, CancellationToken ct = default)
    {
        var url = string.Format(CultureInfo.InvariantCulture,
            "{0}?nord={1}&ost={2}&koordsys=4258&geojson=false", ElevationEndpoint, lat, lng);
        using var doc = await GetAsync(url, ct);
        if (doc is null) return null;
        if (!doc.RootElement.TryGetProperty("punkter", out var pts) || pts.GetArrayLength() == 0)
            return null;
        if (!pts[0].TryGetProperty("z", out var z) || z.ValueKind != JsonValueKind.Number)
            return null;
        var value = z.GetDouble();
        return double.IsFinite(value) && value is >= MinElevationM and <= MaxElevationM ? value : null;
    }

    public async Task<(string? Kommune, string? Fylke)> KommuneAsync(
        double lat, double lng, CancellationToken ct = default)
    {
        var url = string.Format(CultureInfo.InvariantCulture,
            "{0}?nord={1}&ost={2}&koordsys=4258", KommuneEndpoint, lat, lng);
        using var doc = await GetAsync(url, ct);
        if (doc is null) return (null, null);
        var root = doc.RootElement;
        var kommune = root.TryGetProperty("kommunenavn", out var k) ? k.GetString() : null;
        var fylke = root.TryGetProperty("fylkesnavn", out var f) ? f.GetString() : null;
        return (kommune, fylke);
    }

    private async Task<JsonDocument?> GetAsync(string url, CancellationToken ct)
    {
        using var resp = await _http.GetAsync(url, ct);
        if (!resp.IsSuccessStatusCode) return null;
        await using var stream = await resp.Content.ReadAsStreamAsync(ct);
        return await JsonDocument.ParseAsync(stream, cancellationToken: ct);
    }
}
