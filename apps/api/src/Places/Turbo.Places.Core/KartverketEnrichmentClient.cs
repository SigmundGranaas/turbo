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

    public async Task<(string? Nummer, string? Kommune, string? Fylke)> KommuneAsync(
        double lat, double lng, CancellationToken ct = default)
    {
        var url = string.Format(CultureInfo.InvariantCulture,
            "{0}?nord={1}&ost={2}&koordsys=4258", KommuneEndpoint, lat, lng);
        using var doc = await GetAsync(url, ct);
        if (doc is null) return (null, null, null);
        var root = doc.RootElement;
        var nummer = root.TryGetProperty("kommunenummer", out var nr) ? nr.GetString() : null;
        var kommune = root.TryGetProperty("kommunenavn", out var k) ? k.GetString() : null;
        var fylke = root.TryGetProperty("fylkesnavn", out var f) ? f.GetString() : null;
        return (nummer, kommune, fylke);
    }

    /// <summary>The kommune's boundary polygon (Kommuneinfo
    /// <c>/kommuner/{nr}/omrade</c>) as an <see cref="Area"/> for the
    /// containment table. <paramref name="fylke"/> rides along as the area's
    /// Kind so containment answers kommune + fylke from one row.</summary>
    public async Task<Area?> KommuneAreaAsync(
        string kommunenummer, string? fylke, CancellationToken ct = default)
    {
        var url = string.Format(CultureInfo.InvariantCulture,
            "https://ws.geonorge.no/kommuneinfo/v1/kommuner/{0}/omrade", kommunenummer);
        using var doc = await GetAsync(url, ct);
        if (doc is null) return null;
        var root = doc.RootElement;
        if (!root.TryGetProperty("omrade", out var omrade) ||
            omrade.ValueKind != JsonValueKind.Object) return null;
        var name = root.TryGetProperty("kommunenavn", out var k) ? k.GetString() : null;
        if (string.IsNullOrWhiteSpace(name)) return null;

        return new Area(
            Source: "kommuneinfo",
            SourceId: kommunenummer,
            AreaType: "kommune",
            Name: name,
            Kind: fylke,
            GeoJsonGeometry: omrade.GetRawText());
    }

    private async Task<JsonDocument?> GetAsync(string url, CancellationToken ct)
    {
        using var resp = await _http.GetAsync(url, ct);
        if (!resp.IsSuccessStatusCode) return null;
        await using var stream = await resp.Content.ReadAsStreamAsync(ct);
        return await JsonDocument.ParseAsync(stream, cancellationToken: ct);
    }
}
