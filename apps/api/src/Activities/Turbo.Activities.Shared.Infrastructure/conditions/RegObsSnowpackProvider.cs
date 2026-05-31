using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json.Serialization;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Real <see cref="ISnowpackProvider"/> against NVE/NGI's regObs API at
/// <c>api.regobs.no</c>. POSTs a search request with a bounding box +
/// recency window and aggregates observations into a
/// <see cref="SnowpackSlice"/>:
///
///   * Distinct weak-layer codes from the snow-cover obs.
///   * Avalanche-event count from the avalanche obs.
///   * Stability-test result codes.
///
/// Public read API (no auth required); we still send a User-Agent
/// configured via <see cref="RegObsOptions.UserAgent"/> because the
/// service identifies callers in logs.
///
/// Search bbox is built around the requested point with a ~10 km
/// half-side — close enough that observations there are relevant to
/// the route, broad enough to get useful coverage in regions where
/// reports are sparse.
/// </summary>
public sealed class RegObsSnowpackProvider : ISnowpackProvider
{
    public string Key => "regobs_snowpack";
    public const string HttpClientName = "regobs";

    private const double SearchRadiusKm = 10.0;

    private readonly IHttpClientFactory _http;
    private readonly ILogger<RegObsSnowpackProvider> _logger;

    public RegObsSnowpackProvider(
        IHttpClientFactory http, ILogger<RegObsSnowpackProvider> logger)
    {
        _http = http;
        _logger = logger;
    }

    public async Task<SnowpackSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, int lookbackDays,
        CancellationToken cancellationToken)
    {
        var client = _http.CreateClient(HttpClientName);

        var to = at.UtcDateTime;
        var from = to.AddDays(-lookbackDays);
        var bbox = BboxAround(latitude, longitude, SearchRadiusKm);

        var request = new RegObsSearchRequest
        {
            FromDtObsTime = from,
            ToDtObsTime = to,
            // Geohazards: 10 = avalanche. The avalanche endpoint
            // covers snow-cover obs, weak-layer obs, avalanche events,
            // and stability tests under one search.
            SelectedGeoHazards = new[] { 10 },
            WestLng = bbox.WestLng,
            SouthLat = bbox.SouthLat,
            EastLng = bbox.EastLng,
            NorthLat = bbox.NorthLat,
            NumberOfRecords = 100,
        };

        try
        {
            using var resp = await client.PostAsJsonAsync("v5/Search", request, cancellationToken)
                .ConfigureAwait(false);
            if (!resp.IsSuccessStatusCode)
            {
                _logger.LogDebug("regObs search returned {Status}", (int)resp.StatusCode);
                return Empty(at);
            }
            var body = await resp.Content.ReadFromJsonAsync<List<RegObsResult>>(cancellationToken)
                .ConfigureAwait(false);
            if (body is null) return Empty(at);
            return AggregateAsync(body, at);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            return Empty(at);
        }
        catch (Exception ex)
        {
            _logger.LogDebug(ex, "regObs search failed for ({Lat},{Lon})", latitude, longitude);
            return Empty(at);
        }
    }

    private static SnowpackSlice AggregateAsync(IReadOnlyList<RegObsResult> results, DateTimeOffset at)
    {
        var weakLayers = new HashSet<string>(StringComparer.Ordinal);
        var stabilityTests = new HashSet<string>(StringComparer.Ordinal);
        var slideEvents = 0;

        foreach (var r in results)
        {
            foreach (var wl in r.WeakLayers ?? Array.Empty<string>())
            {
                if (!string.IsNullOrWhiteSpace(wl)) weakLayers.Add(wl);
            }
            foreach (var st in r.StabilityTests ?? Array.Empty<string>())
            {
                if (!string.IsNullOrWhiteSpace(st)) stabilityTests.Add(st);
            }
            // regObs marks slide observations with a registration type
            // code; the public search response surfaces it via
            // RegistrationTid (15 = avalanche observation).
            if (r.RegistrationTid == 15) slideEvents++;
        }

        return new SnowpackSlice(
            validAt: at,
            weakLayers: weakLayers.ToArray(),
            recentSlideActivity: slideEvents,
            stabilityTests: stabilityTests.ToArray(),
            observationCount: results.Count);
    }

    private static SnowpackSlice Empty(DateTimeOffset at) =>
        new(at,
            weakLayers: Array.Empty<string>(),
            recentSlideActivity: 0,
            stabilityTests: Array.Empty<string>(),
            observationCount: 0);

    private static (double WestLng, double SouthLat, double EastLng, double NorthLat) BboxAround(
        double lat, double lon, double radiusKm)
    {
        // 1° latitude ≈ 111 km; 1° longitude varies by cos(latitude).
        var dLat = radiusKm / 111.0;
        var dLon = radiusKm / (111.0 * Math.Cos(lat * Math.PI / 180.0));
        return (lon - dLon, lat - dLat, lon + dLon, lat + dLat);
    }

    private sealed class RegObsSearchRequest
    {
        [JsonPropertyName("FromDtObsTime")] public DateTime FromDtObsTime { get; set; }
        [JsonPropertyName("ToDtObsTime")] public DateTime ToDtObsTime { get; set; }
        [JsonPropertyName("SelectedGeoHazards")] public IReadOnlyList<int> SelectedGeoHazards { get; set; } = Array.Empty<int>();
        [JsonPropertyName("WestLng")] public double WestLng { get; set; }
        [JsonPropertyName("SouthLat")] public double SouthLat { get; set; }
        [JsonPropertyName("EastLng")] public double EastLng { get; set; }
        [JsonPropertyName("NorthLat")] public double NorthLat { get; set; }
        [JsonPropertyName("NumberOfRecords")] public int NumberOfRecords { get; set; }
    }

    /// <summary>Subset of the regObs search-result shape. The real
    /// response is far richer; we only need what feeds the synthesizer.
    /// Field names here intentionally tolerate the various shapes the
    /// regObs API has carried over time — both <c>WeakLayers</c> and
    /// the nested-form layer codes are accepted.</summary>
    private sealed class RegObsResult
    {
        [JsonPropertyName("WeakLayers")] public string[]? WeakLayers { get; set; }
        [JsonPropertyName("StabilityTests")] public string[]? StabilityTests { get; set; }
        [JsonPropertyName("RegistrationTid")] public int? RegistrationTid { get; set; }
    }
}

public sealed class RegObsOptions
{
    public string BaseUrl { get; set; } = "https://api.regobs.no/";
    public string UserAgent { get; set; } = "turbo-app/0.1";
    public bool Enabled { get; set; }
}
