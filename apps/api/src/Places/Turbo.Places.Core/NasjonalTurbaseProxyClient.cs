using System.Globalization;
using System.Text.Json;
using Microsoft.Extensions.Options;

namespace Turboapi.Places.Core;

/// <summary>A place/cabin/trip projected from a Nasjonal Turbase document.</summary>
public sealed record NtbPoi(
    string Id,
    string Type,          // "cabin" | "trip" | "place"
    double Lat,
    double Lng,
    string Title,
    string? Summary,
    string? ImageUrl,
    string? UtUrl);

/// <summary>A trip's full detail: route polyline (as [lng,lat] pairs) + metadata.</summary>
public sealed record NtbRoute(
    string Id,
    string Title,
    IReadOnlyList<double[]> Points,
    string? Description,
    double? DistanceMeters,
    string? Grade,
    string? ImageUrl,
    string? UtUrl);

/// <summary>
/// Server-side proxy to the open Nasjonal Turbase API (ut.no / DNT). Injects the
/// secret <c>api_key</c> and normalises the upstream's loose documents into the
/// small DTOs the mobile clients consume — so the NTB-specific quirks (geojson
/// shapes, tag/link extraction) live here, in one place, and every client stays
/// thin. Read-only. Failures degrade to empty/null rather than throwing.
/// </summary>
public sealed class NasjonalTurbaseProxyClient
{
    private readonly HttpClient _http;
    private readonly TurbasenConfig _config;

    public NasjonalTurbaseProxyClient(HttpClient http, IOptions<TurbasenConfig> config)
    {
        _http = http;
        _config = config.Value;
    }

    public bool IsConfigured => !string.IsNullOrWhiteSpace(_config.ApiKey);

    /// <summary>Builds a geo-bounded list query for <paramref name="type"/>
    /// (<c>steder</c> / <c>turer</c>), injecting the api key and a Mongo-style
    /// <c>near</c> centred on the bbox. The exact geo-param spelling is isolated
    /// here so it can be tuned against the live API.</summary>
    public Uri BuildListUri(
        string type, double minLat, double minLon, double maxLat, double maxLon, int limit)
    {
        var centerLat = (minLat + maxLat) / 2;
        var centerLon = (minLon + maxLon) / 2;
        var radius = Haversine(minLat, minLon, maxLat, maxLon) / 2;
        var near = string.Format(
            CultureInfo.InvariantCulture,
            "{{\"$geometry\":{{\"type\":\"Point\",\"coordinates\":[{0},{1}]}},\"$maxDistance\":{2}}}",
            centerLon, centerLat, Math.Round(radius));

        var baseUrl = _config.BaseUrl.TrimEnd('/');
        var query =
            $"api_key={Uri.EscapeDataString(_config.ApiKey)}" +
            $"&limit={limit}" +
            $"&near={Uri.EscapeDataString(near)}";
        return new Uri($"{baseUrl}/{_config.ApiVersion}/{type}?{query}");
    }

    /// <summary>Cabins/places (<c>steder</c>) + trip markers (<c>turer</c>) in
    /// the bbox.</summary>
    public async Task<IReadOnlyList<NtbPoi>> FetchPoisAsync(
        double minLat, double minLon, double maxLat, double maxLon,
        int limitPerType = 80, CancellationToken ct = default)
    {
        if (!IsConfigured) return Array.Empty<NtbPoi>();

        var steder = FetchListAsync("steder", minLat, minLon, maxLat, maxLon, limitPerType, PoiFromSted, ct);
        var turer = FetchListAsync("turer", minLat, minLon, maxLat, maxLon, limitPerType, PoiFromTur, ct);
        await Task.WhenAll(steder, turer);
        return [.. steder.Result, .. turer.Result];
    }

    /// <summary>One trip's route geometry + metadata, or null on any failure.</summary>
    public async Task<NtbRoute?> FetchRouteAsync(string turId, CancellationToken ct = default)
    {
        if (!IsConfigured || string.IsNullOrWhiteSpace(turId)) return null;
        try
        {
            var baseUrl = _config.BaseUrl.TrimEnd('/');
            var uri = new Uri(
                $"{baseUrl}/{_config.ApiVersion}/turer/{Uri.EscapeDataString(turId)}" +
                $"?api_key={Uri.EscapeDataString(_config.ApiKey)}");
            using var doc = await GetJsonAsync(uri, ct);
            return doc is null ? null : RouteFromTur(doc.RootElement);
        }
        catch
        {
            return null;
        }
    }

    private async Task<List<NtbPoi>> FetchListAsync(
        string type, double minLat, double minLon, double maxLat, double maxLon,
        int limit, Func<JsonElement, NtbPoi?> project, CancellationToken ct)
    {
        try
        {
            var uri = BuildListUri(type, minLat, minLon, maxLat, maxLon, limit);
            using var doc = await GetJsonAsync(uri, ct);
            if (doc is null) return [];
            var outList = new List<NtbPoi>();
            foreach (var d in Documents(doc.RootElement))
            {
                var poi = project(d);
                if (poi is not null) outList.Add(poi);
            }
            return outList;
        }
        catch
        {
            return [];
        }
    }

    private async Task<JsonDocument?> GetJsonAsync(Uri uri, CancellationToken ct)
    {
        using var req = new HttpRequestMessage(HttpMethod.Get, uri);
        req.Headers.UserAgent.ParseAdd("turbo-api/1.0 (+https://github.com/sigmundgranaas/turbo)");
        req.Headers.Accept.ParseAdd("application/json");
        using var resp = await _http.SendAsync(req, ct);
        if (!resp.IsSuccessStatusCode) return null;
        await using var stream = await resp.Content.ReadAsStreamAsync(ct);
        return await JsonDocument.ParseAsync(stream, cancellationToken: ct);
    }

    // --- document → DTO projections (public + static for unit tests) ---

    public static IEnumerable<JsonElement> Documents(JsonElement root)
    {
        JsonElement list;
        if (root.ValueKind == JsonValueKind.Array)
        {
            list = root;
        }
        else if (root.ValueKind == JsonValueKind.Object &&
                 (TryProp(root, "documents", out list) ||
                  TryProp(root, "data", out list) ||
                  TryProp(root, "results", out list)) &&
                 list.ValueKind == JsonValueKind.Array)
        {
            // list assigned
        }
        else
        {
            yield break;
        }

        foreach (var e in list.EnumerateArray())
            if (e.ValueKind == JsonValueKind.Object)
                yield return e;
    }

    public static NtbPoi? PoiFromSted(JsonElement doc)
    {
        var pos = NtbGeo.Point(Prop(doc, "geojson"));
        if (pos is null) return null;
        var isCabin = Tags(doc).Any(t => string.Equals(t, "Hytte", StringComparison.OrdinalIgnoreCase));
        var id = Id(doc);
        return new NtbPoi(
            id, isCabin ? "cabin" : "place", pos.Value.lat, pos.Value.lng,
            Title(doc), Summary(doc), FirstImage(doc), UtUrl(doc, isCabin ? "hytte" : "sted", id));
    }

    public static NtbPoi? PoiFromTur(JsonElement doc)
    {
        var pos = NtbGeo.Point(Prop(doc, "geojson"));
        if (pos is null) return null;
        var id = Id(doc);
        return new NtbPoi(
            id, "trip", pos.Value.lat, pos.Value.lng,
            Title(doc), Summary(doc), FirstImage(doc), UtUrl(doc, "turforslag", id));
    }

    public static NtbRoute RouteFromTur(JsonElement doc)
    {
        var id = Id(doc);
        var points = NtbGeo.Line(Prop(doc, "geojson"))
            .Select(p => new[] { p.lng, p.lat }).ToArray();
        return new NtbRoute(
            id, Title(doc), points, Summary(doc),
            DistanceMeters(doc), Str(Prop(doc, "gradering")), FirstImage(doc),
            UtUrl(doc, "turforslag", id));
    }

    // --- field helpers ---

    private static JsonElement? Prop(JsonElement obj, string name) =>
        obj.ValueKind == JsonValueKind.Object && obj.TryGetProperty(name, out var v) ? v : null;

    private static bool TryProp(JsonElement obj, string name, out JsonElement value)
    {
        if (obj.TryGetProperty(name, out value)) return true;
        value = default;
        return false;
    }

    private static string Id(JsonElement doc) =>
        Str(Prop(doc, "_id")) ?? Str(Prop(doc, "id")) ?? "";

    private static string Title(JsonElement doc)
    {
        var navn = Str(Prop(doc, "navn"));
        return string.IsNullOrEmpty(navn) ? Id(doc) : navn;
    }

    private static string? Summary(JsonElement doc)
    {
        foreach (var key in new[] { "beskrivelse", "innledning", "ingress" })
        {
            var v = Str(Prop(doc, key));
            if (!string.IsNullOrEmpty(v)) return v;
        }
        return null;
    }

    private static string? Str(JsonElement? el)
    {
        if (el is null) return null;
        var e = el.Value;
        return e.ValueKind switch
        {
            JsonValueKind.String => string.IsNullOrWhiteSpace(e.GetString()) ? null : e.GetString(),
            JsonValueKind.Number => e.ToString(),
            _ => null,
        };
    }

    private static IEnumerable<string> Tags(JsonElement doc)
    {
        var tags = Prop(doc, "tags");
        if (tags is { ValueKind: JsonValueKind.Array })
            foreach (var t in tags.Value.EnumerateArray())
                if (t.ValueKind == JsonValueKind.String)
                    yield return t.GetString()!;
    }

    private static double? DistanceMeters(JsonElement doc)
    {
        var d = Prop(doc, "distanse");
        if (d is null) return null;
        return d.Value.ValueKind switch
        {
            JsonValueKind.Number => d.Value.GetDouble(),
            JsonValueKind.String => double.TryParse(
                d.Value.GetString(), NumberStyles.Float, CultureInfo.InvariantCulture, out var v)
                ? v
                : null,
            _ => null,
        };
    }

    private static string? FirstImage(JsonElement doc)
    {
        var bilder = Prop(doc, "bilder");
        if (bilder is not { ValueKind: JsonValueKind.Array }) return null;
        foreach (var b in bilder.Value.EnumerateArray())
        {
            if (b.ValueKind == JsonValueKind.String && b.GetString()?.StartsWith("http") == true)
                return b.GetString();
            if (b.ValueKind == JsonValueKind.Object)
                foreach (var key in new[] { "url", "original", "src", "href" })
                    if (b.TryGetProperty(key, out var u) && u.ValueKind == JsonValueKind.String &&
                        u.GetString()?.StartsWith("http") == true)
                        return u.GetString();
        }
        return null;
    }

    private static string? UtUrl(JsonElement doc, string path, string id)
    {
        var lenker = Prop(doc, "lenker");
        if (lenker is { ValueKind: JsonValueKind.Array })
            foreach (var l in lenker.Value.EnumerateArray())
                if (l.ValueKind == JsonValueKind.Object &&
                    l.TryGetProperty("url", out var u) && u.ValueKind == JsonValueKind.String &&
                    u.GetString()?.Contains("ut.no") == true)
                    return u.GetString();
        return string.IsNullOrEmpty(id) ? null : $"https://ut.no/{path}/{id}";
    }

    // --- geometry ---

    private static double Haversine(double lat1, double lon1, double lat2, double lon2)
    {
        const double r = 6371000.0;
        double Rad(double d) => d * Math.PI / 180.0;
        var dLat = Rad(lat2 - lat1);
        var dLon = Rad(lon2 - lon1);
        var a = Math.Sin(dLat / 2) * Math.Sin(dLat / 2) +
                Math.Cos(Rad(lat1)) * Math.Cos(Rad(lat2)) *
                Math.Sin(dLon / 2) * Math.Sin(dLon / 2);
        return r * 2 * Math.Atan2(Math.Sqrt(a), Math.Sqrt(1 - a));
    }
}

/// <summary>Parses the GeoJSON carried in a Nasjonal Turbase <c>geojson</c>
/// field (bare geometry or wrapped Feature; coords are [lon, lat]). Tolerant —
/// returns null/empty for anything malformed.</summary>
internal static class NtbGeo
{
    public static (double lat, double lng)? Point(JsonElement? geojson)
    {
        var geom = Geometry(geojson);
        if (geom is null) return null;
        var type = geom.Value.TryGetProperty("type", out var t) ? t.GetString() : null;
        if (type == "Point")
            return Coord(geom.Value.TryGetProperty("coordinates", out var c) ? c : default);
        var line = Line(geojson);
        return line.Count == 0 ? null : line[0];
    }

    public static IReadOnlyList<(double lat, double lng)> Line(JsonElement? geojson)
    {
        var geom = Geometry(geojson);
        if (geom is null) return Array.Empty<(double, double)>();
        var type = geom.Value.TryGetProperty("type", out var t) ? t.GetString() : null;
        if (!geom.Value.TryGetProperty("coordinates", out var coords) ||
            coords.ValueKind != JsonValueKind.Array)
            return Array.Empty<(double, double)>();

        var outList = new List<(double lat, double lng)>();
        if (type == "LineString")
        {
            foreach (var c in coords.EnumerateArray())
            {
                var p = Coord(c);
                if (p is not null) outList.Add(p.Value);
            }
        }
        else if (type == "MultiLineString")
        {
            foreach (var part in coords.EnumerateArray())
            {
                if (part.ValueKind != JsonValueKind.Array) continue;
                foreach (var c in part.EnumerateArray())
                {
                    var p = Coord(c);
                    if (p is not null) outList.Add(p.Value);
                }
            }
        }
        return outList;
    }

    private static JsonElement? Geometry(JsonElement? geojson)
    {
        if (geojson is not { ValueKind: JsonValueKind.Object } el) return null;
        var type = el.TryGetProperty("type", out var t) ? t.GetString() : null;
        if (type == "Feature")
            return Geometry(el.TryGetProperty("geometry", out var g) ? g : (JsonElement?)null);
        if (type == "GeometryCollection")
        {
            if (el.TryGetProperty("geometries", out var gs) && gs.ValueKind == JsonValueKind.Array)
                foreach (var g in gs.EnumerateArray())
                    return Geometry(g);
            return null;
        }
        return el;
    }

    private static (double lat, double lng)? Coord(JsonElement c)
    {
        if (c.ValueKind != JsonValueKind.Array) return null;
        var arr = c.EnumerateArray().ToArray();
        if (arr.Length < 2 ||
            arr[0].ValueKind != JsonValueKind.Number ||
            arr[1].ValueKind != JsonValueKind.Number)
            return null;
        return (arr[1].GetDouble(), arr[0].GetDouble());
    }
}
