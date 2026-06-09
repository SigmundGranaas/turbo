using System.Globalization;
using System.Text.Json;
using Turboapi.Places;

namespace Turboapi.Places.Core;

/// <summary>
/// Downloads protected-area polygons (national parks, nature reserves,
/// landscape-protected areas) from Miljødirektoratet's Naturbase ArcGIS
/// service for a bounding box, as GeoJSON. The polygons are stored in the
/// areas table so the reverse path resolves "In Jotunheimen nasjonalpark"
/// by containment — no Identify call at query time.
/// </summary>
public sealed class NaturbaseClient
{
    // Layer 0 = current protected areas (polygons). The live app's Identify
    // probe discovers this dynamically; for ingestion the layer id is stable
    // enough to pin, and a wrong pin fails loudly at ingest, not at runtime.
    private const string Endpoint =
        "https://kart.miljodirektoratet.no/arcgis/rest/services/vern/MapServer/0/query";

    private readonly HttpClient _http;

    public NaturbaseClient(HttpClient http) => _http = http;

    /// <summary>Protected areas intersecting the bbox (WGS84 lon/lat).</summary>
    public async Task<IReadOnlyList<Area>> DownloadAreasAsync(
        double minLng, double minLat, double maxLng, double maxLat,
        CancellationToken ct = default)
    {
        var url = string.Format(CultureInfo.InvariantCulture,
            "{0}?where=1%3D1&geometry={1},{2},{3},{4}" +
            "&geometryType=esriGeometryEnvelope&inSR=4326&spatialRel=esriSpatialRelIntersects" +
            "&outFields=navn,verneform,naturvernId&returnGeometry=true&outSR=4326&f=geojson",
            Endpoint, minLng, minLat, maxLng, maxLat);

        using var resp = await _http.GetAsync(url, ct);
        resp.EnsureSuccessStatusCode();
        await using var stream = await resp.Content.ReadAsStreamAsync(ct);
        using var doc = await JsonDocument.ParseAsync(stream, cancellationToken: ct);

        var areas = new List<Area>();
        if (!doc.RootElement.TryGetProperty("features", out var features))
            return areas;

        foreach (var f in features.EnumerateArray())
        {
            if (!f.TryGetProperty("properties", out var props)) continue;
            if (!f.TryGetProperty("geometry", out var geom) ||
                geom.ValueKind != JsonValueKind.Object) continue;

            var name = props.TryGetProperty("navn", out var n) ? n.GetString() : null;
            if (string.IsNullOrWhiteSpace(name)) continue;
            // Naturbase codes (VV00002858) must never surface as a name.
            if (System.Text.RegularExpressions.Regex.IsMatch(name, @"^[A-Z]{1,5}\d+$")) continue;

            var kind = props.TryGetProperty("verneform", out var v) ? v.GetString() : null;
            var id = props.TryGetProperty("naturvernId", out var i)
                ? i.ToString()
                : name;

            areas.Add(new Area(
                Source: "naturbase",
                SourceId: id,
                AreaType: "protected_area",
                Name: name,
                Kind: kind,
                GeoJsonGeometry: geom.GetRawText()));
        }
        return areas;
    }
}
