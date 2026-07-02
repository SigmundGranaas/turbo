using System.Text;
using System.Text.Json;

namespace Turboapi.Places.Ingestion;

/// <summary>A Geonorge download area (fylke/kommune/landsdekkende), from the
/// area codelist.</summary>
public sealed record GeonorgeArea(string Type, string Name, string Code);

/// <summary>A projection from the codelist (Geonorge serves UTM33 = 25833).</summary>
public sealed record GeonorgeProjection(string Code, string Name, string Codespace);

/// <summary>A file the order produced, ready to download.</summary>
public sealed record GeonorgeFile(string Name, string DownloadUrl, string Status);

/// <summary>
/// Client for Geonorge's bulk download API. The flow is: resolve a dataset's
/// codelists (area/format/projection), POST an order, then download the
/// resulting file(s) — typically a ZIP of GeoJSON/GML/GPKG. Discovered against
/// the live API (see fixtures/geonorge-order-response.json).
/// </summary>
public sealed class GeonorgeClient
{
    public const string OrderEndpoint = "https://nedlasting.geonorge.no/api/order";

    /// <summary>Kartkatalog metadata endpoint — a small JSON GET (no order, no
    /// download) that carries the dataset's <c>DateUpdated</c> (the data-update
    /// date, distinct from <c>DateMetadataUpdated</c>). Used as a cheap
    /// pre-download freshness marker so an unchanged dataset is never re-ordered
    /// or re-downloaded.</summary>
    public const string MetadataEndpoint = "https://kartkatalog.geonorge.no/api/getdata/";

    private static readonly JsonSerializerOptions CamelCase = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
    };

    private readonly HttpClient _http;

    public GeonorgeClient(HttpClient http) => _http = http;

    /// <summary>The order request body for one dataset / area / format /
    /// projection (camelCase, as the API expects).</summary>
    public static string BuildOrderJson(
        string metadataUuid, GeonorgeArea area, string format, GeonorgeProjection projection)
    {
        var order = new
        {
            email = "",
            orderLines = new[]
            {
                new
                {
                    metadataUuid,
                    areas = new[] { new { area.Code, area.Type, area.Name } },
                    projections = new[] { new { projection.Code, projection.Name, projection.Codespace } },
                    formats = new[] { new { name = format } },
                },
            },
        };
        return JsonSerializer.Serialize(order, CamelCase);
    }

    /// <summary>Extract the ready-to-download files from an order response.</summary>
    public static IReadOnlyList<GeonorgeFile> ParseOrderResponse(string json)
    {
        using var doc = JsonDocument.Parse(json);
        var files = new List<GeonorgeFile>();
        if (!doc.RootElement.TryGetProperty("files", out var arr) || arr.ValueKind != JsonValueKind.Array)
            return files;

        foreach (var f in arr.EnumerateArray())
        {
            var name = f.TryGetProperty("name", out var n) ? n.GetString() : null;
            var url = f.TryGetProperty("downloadUrl", out var u) ? u.GetString() : null;
            if (string.IsNullOrEmpty(name) || string.IsNullOrEmpty(url)) continue;
            var status = f.TryGetProperty("status", out var s) ? s.GetString() ?? "" : "";
            files.Add(new GeonorgeFile(name, url, status));
        }
        return files;
    }

    /// <summary>Parse the dataset's data-update marker (<c>DateUpdated</c>) from a
    /// Kartkatalog metadata JSON body. Falls back to <c>DateMetadataUpdated</c>
    /// only if the data date is absent; returns null when neither is present.</summary>
    public static string? ParseDatasetVersion(string json)
    {
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;
        foreach (var key in new[] { "DateUpdated", "DateMetadataUpdated" })
        {
            if (root.TryGetProperty(key, out var v) && v.ValueKind == JsonValueKind.String)
            {
                var s = v.GetString();
                if (!string.IsNullOrWhiteSpace(s)) return s;
            }
        }
        return null;
    }

    /// <summary>Cheap pre-download freshness marker for a dataset: its
    /// <c>DateUpdated</c> from Kartkatalog metadata (a small JSON GET — no order,
    /// no bulk download). Returns null if the metadata can't be fetched/parsed, so
    /// the caller falls back to ingesting rather than skipping.</summary>
    public async Task<string?> GetDatasetVersionAsync(string metadataUuid, CancellationToken ct = default)
    {
        try
        {
            using var resp = await _http.GetAsync(MetadataEndpoint + metadataUuid, ct);
            if (!resp.IsSuccessStatusCode) return null;
            return ParseDatasetVersion(await resp.Content.ReadAsStringAsync(ct));
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or JsonException)
        {
            return null;
        }
    }

    /// <summary>POST an order and return its ready files.</summary>
    public async Task<IReadOnlyList<GeonorgeFile>> OrderAsync(
        string metadataUuid, GeonorgeArea area, string format, GeonorgeProjection projection,
        CancellationToken ct = default)
    {
        using var content = new StringContent(
            BuildOrderJson(metadataUuid, area, format, projection), Encoding.UTF8, "application/json");
        using var resp = await _http.PostAsync(OrderEndpoint, content, ct);
        resp.EnsureSuccessStatusCode();
        return ParseOrderResponse(await resp.Content.ReadAsStringAsync(ct));
    }

    /// <summary>Stream a file to disk (the order's downloadUrl).</summary>
    public async Task DownloadToAsync(string url, string destinationPath, CancellationToken ct = default)
    {
        using var resp = await _http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct);
        resp.EnsureSuccessStatusCode();
        await using var src = await resp.Content.ReadAsStreamAsync(ct);
        await using var dst = File.Create(destinationPath);
        await src.CopyToAsync(dst, ct);
    }
}
