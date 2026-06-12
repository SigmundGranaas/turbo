using System.Globalization;
using System.Runtime.CompilerServices;
using System.Text.Json;
using Turboapi.Places;

namespace Turboapi.Places.Core;

/// <summary>
/// Batch-downloads a sample area of real place names from Kartverket's
/// Stedsnavn REST API (<c>/stedsnavn/v1/punkt</c>), paging the disc around a
/// centre point. This is the M1 "use real data" path; national bulk ingestion
/// (Geonorge GPKG) comes at M4.
/// </summary>
public sealed class KartverketStedsnavnClient
{
    private const string Endpoint = "https://ws.geonorge.no/stedsnavn/v1/punkt";
    private const int PageSize = 100;

    /// <summary>The <c>/punkt</c> endpoint rejects radii above 5 km (HTTP 400).
    /// Larger sample areas must be tiled across several centres.</summary>
    public const int MaxRadiusM = 5000;

    private readonly HttpClient _http;

    public KartverketStedsnavnClient(HttpClient http) => _http = http;

    /// <summary>
    /// Streams every primary-name feature within <paramref name="radiusM"/>
    /// metres of (<paramref name="lat"/>, <paramref name="lng"/>).
    /// </summary>
    public async IAsyncEnumerable<Place> DownloadAreaAsync(
        double lat, double lng, int radiusM,
        [EnumeratorCancellation] CancellationToken ct = default)
    {
        radiusM = Math.Min(radiusM, MaxRadiusM);
        var total = int.MaxValue;
        var seen = 0;
        for (var side = 1; seen < total; side++)
        {
            var url = string.Format(CultureInfo.InvariantCulture,
                "{0}?nord={1}&ost={2}&koordsys=4258&radius={3}&treffPerSide={4}&side={5}&navnestatus=hovednavn",
                Endpoint, lat, lng, radiusM, PageSize, side);

            using var stream = await _http.GetStreamAsync(url, ct);
            using var doc = await JsonDocument.ParseAsync(stream, cancellationToken: ct);
            var root = doc.RootElement;

            if (root.TryGetProperty("metadata", out var meta) &&
                meta.TryGetProperty("totaltAntallTreff", out var t))
            {
                total = t.GetInt32();
            }

            if (!root.TryGetProperty("navn", out var navn) || navn.GetArrayLength() == 0)
            {
                yield break;
            }

            foreach (var item in navn.EnumerateArray())
            {
                seen++;
                var place = Map(item);
                if (place is not null) yield return place;
            }
        }
    }

    private static Place? Map(JsonElement item)
    {
        var name = PrimaryName(item);
        if (name is null) return null;

        if (!item.TryGetProperty("representasjonspunkt", out var pt)) return null;
        if (!pt.TryGetProperty("nord", out var nord) || !pt.TryGetProperty("øst", out var ost))
            return null;

        var kind = item.TryGetProperty("navneobjekttype", out var k) ? k.GetString() ?? "" : "";
        var status = item.TryGetProperty("stedstatus", out var s) ? s.GetString() ?? "aktiv" : "aktiv";
        var sourceId = item.TryGetProperty("stedsnummer", out var sn)
            ? sn.GetRawText()
            : Guid.NewGuid().ToString("n");

        return new Place(
            Source: "ssr",
            SourceId: sourceId,
            FeatureType: kind,
            PrimaryName: name,
            Lat: nord.GetDouble(),
            Lng: ost.GetDouble(),
            Status: status);
    }

    /// <summary>Primary spelling from the <c>stedsnavn[]</c> array, preferring
    /// the <c>hovednavn</c> entry; rejects empty / "Ukjent".</summary>
    private static string? PrimaryName(JsonElement item)
    {
        if (!item.TryGetProperty("stedsnavn", out var names) || names.ValueKind != JsonValueKind.Array)
            return null;

        string? fallback = null;
        foreach (var n in names.EnumerateArray())
        {
            if (!n.TryGetProperty("skrivemåte", out var w)) continue;
            var spelling = w.GetString()?.Trim();
            if (string.IsNullOrEmpty(spelling)) continue;
            if (spelling.Equals("Ukjent", StringComparison.OrdinalIgnoreCase) ||
                spelling.Equals("Unknown", StringComparison.OrdinalIgnoreCase)) continue;

            var status = n.TryGetProperty("navnestatus", out var ns) ? ns.GetString() : null;
            if (status == "hovednavn") return spelling;
            fallback ??= spelling;
        }
        return fallback;
    }
}
