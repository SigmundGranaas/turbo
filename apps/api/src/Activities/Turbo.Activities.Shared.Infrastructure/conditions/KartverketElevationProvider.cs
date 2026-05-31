using System.Globalization;
using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Real <see cref="IElevationProvider"/> against Kartverket's public
/// elevation point API at https://ws.geonorge.no/hoydedata/v1/punkt.
/// One GET per sample point: <c>?nord=&ost=&koordsys=4258&geojson=false</c>.
/// Kartverket's DTM has ~10 m resolution at most of Norway; pixel
/// values come back in meters.
///
/// Per-attempt timeout + resilience handler are configured on the
/// named HTTP client by <c>ActivitiesSharedModule</c>. We cap concurrent
/// in-flight requests with a SemaphoreSlim so a 200-vertex route
/// doesn't fan out 200 simultaneous calls to a free public API.
///
/// Geographic coordinates are sent in EPSG:4258 (ETRS89). For the
/// geographic accuracy we care about (~10 m DEM resolution), WGS84
/// lat/lon coordinates are within ~1 m of ETRS89 lat/lon, so we pass
/// them through unchanged.
/// </summary>
public sealed class KartverketElevationProvider : IElevationProvider
{
    public string Key => "kartverket_dem";
    public const string HttpClientName = "kartverket-dem";

    /// <summary>Max parallel point lookups. Kartverket's API doesn't
    /// publish a rate limit but it's a free public service — bound
    /// ourselves at a polite 6 in flight per request.</summary>
    private const int MaxConcurrentRequests = 6;

    private readonly IHttpClientFactory _http;
    private readonly ILogger<KartverketElevationProvider> _logger;

    public KartverketElevationProvider(
        IHttpClientFactory http, ILogger<KartverketElevationProvider> logger)
    {
        _http = http;
        _logger = logger;
    }

    public async Task<ElevationSlice> GetAsync(
        IReadOnlyList<(double Latitude, double Longitude)> path,
        double sampleSpacingM,
        CancellationToken cancellationToken)
    {
        if (path.Count == 0)
            return new ElevationSlice(DateTimeOffset.UtcNow, Array.Empty<ElevationSample>());

        var client = _http.CreateClient(HttpClientName);
        var sem = new SemaphoreSlim(MaxConcurrentRequests);

        // Walk vertices, accumulating cumulative distance as we go.
        var distances = new double[path.Count];
        for (var i = 1; i < path.Count; i++)
        {
            distances[i] = distances[i - 1] + HaversineMeters(path[i - 1], path[i]);
        }

        var samples = new ElevationSample?[path.Count];
        var tasks = new List<Task>(path.Count);
        for (var i = 0; i < path.Count; i++)
        {
            var idx = i;
            var (lat, lon) = path[i];
            tasks.Add(Task.Run(async () =>
            {
                await sem.WaitAsync(cancellationToken).ConfigureAwait(false);
                try
                {
                    var elev = await FetchOneAsync(client, lat, lon, cancellationToken).ConfigureAwait(false);
                    if (elev is not null)
                    {
                        samples[idx] = new ElevationSample(distances[idx], elev.Value);
                    }
                }
                finally
                {
                    sem.Release();
                }
            }, cancellationToken));
        }

        await Task.WhenAll(tasks).ConfigureAwait(false);

        // Drop nulls (failed fetches) and return remaining points in
        // distance order. Orchestrators degrade gracefully on sparse
        // profiles — they'd rather have 70% of a route than none.
        var ordered = samples
            .Where(s => s is not null)
            .Cast<ElevationSample>()
            .OrderBy(s => s.DistanceM)
            .ToArray();
        return new ElevationSlice(DateTimeOffset.UtcNow, ordered);
    }

    private async Task<double?> FetchOneAsync(
        HttpClient client, double lat, double lon, CancellationToken ct)
    {
        try
        {
            // Note query parameter casing — Kartverket's API uses
            // Norwegian names. koordsys=4258 (ETRS89) is the closest
            // public option to WGS84 for the geographic ranges we use.
            var url = "v1/punkt"
                + $"?nord={lat.ToString(CultureInfo.InvariantCulture)}"
                + $"&ost={lon.ToString(CultureInfo.InvariantCulture)}"
                + "&koordsys=4258"
                + "&geojson=false";
            using var resp = await client.GetAsync(url, ct).ConfigureAwait(false);
            if (!resp.IsSuccessStatusCode)
            {
                _logger.LogDebug("Kartverket DEM call returned {Status} for ({Lat},{Lon})",
                    (int)resp.StatusCode, lat, lon);
                return null;
            }
            var doc = await resp.Content.ReadFromJsonAsync<KartverketPointResponse>(ct).ConfigureAwait(false);
            if (doc?.Punkter is null || doc.Punkter.Count == 0) return null;
            var first = doc.Punkter[0];
            if (first.Z is null) return null;
            return first.Z.Value;
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested)
        {
            return null;
        }
        catch (Exception ex)
        {
            _logger.LogDebug(ex, "Kartverket DEM call failed for ({Lat},{Lon})", lat, lon);
            return null;
        }
    }

    private static double HaversineMeters((double Lat, double Lon) a, (double Lat, double Lon) b)
    {
        const double earthRadiusM = 6_371_000;
        var lat1 = a.Lat * Math.PI / 180.0;
        var lat2 = b.Lat * Math.PI / 180.0;
        var dLat = lat2 - lat1;
        var dLon = (b.Lon - a.Lon) * Math.PI / 180.0;
        var h = Math.Sin(dLat / 2) * Math.Sin(dLat / 2)
                + Math.Cos(lat1) * Math.Cos(lat2) * Math.Sin(dLon / 2) * Math.Sin(dLon / 2);
        return 2 * earthRadiusM * Math.Atan2(Math.Sqrt(h), Math.Sqrt(1 - h));
    }

    /// <summary>Wire shape of the Kartverket point-elevation response.
    /// Only the fields we use are listed; the API returns a few more
    /// (datum, koordsys, etc.) we ignore.</summary>
    private sealed class KartverketPointResponse
    {
        [JsonPropertyName("punkter")] public List<Point>? Punkter { get; set; }

        public sealed class Point
        {
            [JsonPropertyName("z")] public double? Z { get; set; }
        }
    }
}

public sealed class KartverketOptions
{
    public string BaseUrl { get; set; } = "https://ws.geonorge.no/hoydedata/";
    public bool Enabled { get; set; }
}
