using System.Text.Json;
using Turboapi.Places;

namespace Turboapi.Places.Ingestion;

/// <summary>Where the canonical fields live in a source area GeoJSON's feature
/// properties (admin enheter, Naturbase). The source CRS is UTM33 (25833) —
/// the reader reprojects to WGS84.</summary>
public sealed record GeoJsonAreaSpec(
    string Source, string AreaType, string IdProperty, string NameProperty, string? KindProperty = null);

/// <summary>
/// Reads Geonorge polygon GeoJSON (admin enheter / Naturbase) into canonical
/// <see cref="Area"/>s, handling the real-world shapes: a UTF-8 BOM, an
/// EPSG:25833 CRS, and geometries wrapped in a GeometryCollection. Vertices are
/// reprojected UTM33→WGS84 and re-emitted as a WGS84 MultiPolygon (which
/// PostGIS ingests via <c>ST_GeomFromGeoJSON</c>).
/// </summary>
public sealed class GeoJsonAreaReader
{
    public IEnumerable<Area> ReadAreas(string path, GeoJsonAreaSpec spec)
    {
        // File.ReadAllText strips the UTF-8 BOM, so JsonDocument is happy.
        var json = File.ReadAllText(path);
        using var doc = JsonDocument.Parse(json);
        if (!doc.RootElement.TryGetProperty("features", out var features) ||
            features.ValueKind != JsonValueKind.Array)
        {
            yield break;
        }

        foreach (var feature in features.EnumerateArray())
        {
            if (!feature.TryGetProperty("properties", out var props)) continue;
            var name = Str(props, spec.NameProperty);
            if (string.IsNullOrWhiteSpace(name)) continue;
            var id = Str(props, spec.IdProperty);
            if (string.IsNullOrWhiteSpace(id)) continue;
            if (!feature.TryGetProperty("geometry", out var geometry)) continue;

            var polygons = new List<List<List<(double X, double Y)>>>();
            CollectPolygons(geometry, polygons);
            if (polygons.Count == 0) continue;

            var kind = spec.KindProperty is null ? null : Str(props, spec.KindProperty);
            yield return new Area(spec.Source, id!, spec.AreaType, name!.Trim(), kind, ToWgs84MultiPolygon(polygons));
        }
    }

    /// <summary>Collect every polygon (as rings of source-CRS vertices) from a
    /// Polygon / MultiPolygon / GeometryCollection.</summary>
    private static void CollectPolygons(JsonElement geometry, List<List<List<(double, double)>>> sink)
    {
        var type = geometry.TryGetProperty("type", out var t) ? t.GetString() : null;
        switch (type)
        {
            case "Polygon":
                sink.Add(ReadRings(geometry.GetProperty("coordinates")));
                break;
            case "MultiPolygon":
                foreach (var poly in geometry.GetProperty("coordinates").EnumerateArray())
                    sink.Add(ReadRings(poly));
                break;
            case "GeometryCollection":
                foreach (var g in geometry.GetProperty("geometries").EnumerateArray())
                    CollectPolygons(g, sink);
                break;
        }
    }

    private static List<List<(double, double)>> ReadRings(JsonElement polygonCoords)
    {
        var rings = new List<List<(double, double)>>();
        foreach (var ring in polygonCoords.EnumerateArray())
        {
            var pts = new List<(double, double)>();
            foreach (var pt in ring.EnumerateArray())
                pts.Add((pt[0].GetDouble(), pt[1].GetDouble())); // (easting, northing)
            rings.Add(pts);
        }
        return rings;
    }

    private static string ToWgs84MultiPolygon(List<List<List<(double X, double Y)>>> polygons)
    {
        var ms = new MemoryStream();
        using (var w = new Utf8JsonWriter(ms))
        {
            w.WriteStartObject();
            w.WriteString("type", "MultiPolygon");
            w.WritePropertyName("coordinates");
            w.WriteStartArray();
            foreach (var polygon in polygons)
            {
                w.WriteStartArray();
                foreach (var ring in polygon)
                {
                    w.WriteStartArray();
                    foreach (var (easting, northing) in ring)
                    {
                        var (lat, lng) = Utm33.ToWgs84(easting, northing);
                        w.WriteStartArray();
                        w.WriteNumberValue(lng);
                        w.WriteNumberValue(lat);
                        w.WriteEndArray();
                    }
                    w.WriteEndArray();
                }
                w.WriteEndArray();
            }
            w.WriteEndArray();
            w.WriteEndObject();
        }
        return System.Text.Encoding.UTF8.GetString(ms.ToArray());
    }

    private static string? Str(JsonElement props, string name) =>
        props.TryGetProperty(name, out var v)
            ? v.ValueKind == JsonValueKind.String ? v.GetString() : v.ToString()
            : null;
}
