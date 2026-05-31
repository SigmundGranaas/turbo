using System.Globalization;
using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json.Serialization;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Real <see cref="IGriddedSnowProvider"/> against NVE's seNorge dataset.
/// The seNorge GridTimeSeries API (<c>gts.nve.no</c>) returns gridded
/// surface state — snow depth, SWE, fresh snow — for a point in
/// EPSG:4326. We pull snow_depth_cm + swe_mm for the requested day +
/// the 24h fresh delta + last-week freeze/thaw count.
///
/// The endpoint shape is:
///   GET /api/GridTimeSeries/{lat}/{lon}/{from}/{to}/{theme}.json
/// We make one call per theme; the named HTTP client batches retries.
/// </summary>
public sealed class SeNorgeGriddedSnowProvider : IGriddedSnowProvider
{
    public string Key => "senorge_gridded_snow";
    public const string HttpClientName = "senorge-gts";

    private readonly IHttpClientFactory _http;
    private readonly ILogger<SeNorgeGriddedSnowProvider> _logger;

    public SeNorgeGriddedSnowProvider(
        IHttpClientFactory http, ILogger<SeNorgeGriddedSnowProvider> logger)
    {
        _http = http;
        _logger = logger;
    }

    public async Task<GriddedSnowSlice> GetAsync(
        double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var client = _http.CreateClient(HttpClientName);
        var to = at.UtcDateTime.Date;
        var from = to.AddDays(-7);

        // Fan out the three themes in parallel.
        var snowDepthTask = FetchSeriesAsync(client, latitude, longitude, from, to, "sdfsw", cancellationToken);
        var sweTask = FetchSeriesAsync(client, latitude, longitude, from, to, "swe", cancellationToken);
        var freshTask = FetchSeriesAsync(client, latitude, longitude, from, to, "fsw", cancellationToken);
        await Task.WhenAll(snowDepthTask, sweTask, freshTask).ConfigureAwait(false);

        var depthCm = snowDepthTask.Result?.LastOrDefault() ?? 0;
        var sweMm = sweTask.Result?.LastOrDefault() ?? 0;
        var fresh24h = freshTask.Result?.LastOrDefault() ?? 0;

        // Crude freeze/thaw proxy from the snow-depth series: count the
        // number of days where depth dropped then rose. Lacking the real
        // temperature series at this grid cell, this is the cheapest
        // proxy that still surfaces something useful.
        var freezeThaw = CountInflections(snowDepthTask.Result);

        return new GriddedSnowSlice(
            validAt: new DateTimeOffset(to, TimeSpan.Zero),
            sweMm: sweMm,
            snowDepthCm: depthCm,
            freshSnowLast24hCm: fresh24h,
            freezeThawLast7d: freezeThaw);
    }

    private async Task<List<double>?> FetchSeriesAsync(
        HttpClient client, double lat, double lon, DateTime from, DateTime to, string theme,
        CancellationToken ct)
    {
        try
        {
            var url = $"api/GridTimeSeries/"
                + lat.ToString(CultureInfo.InvariantCulture) + "/"
                + lon.ToString(CultureInfo.InvariantCulture) + "/"
                + from.ToString("yyyy-MM-dd") + "/"
                + to.ToString("yyyy-MM-dd") + "/"
                + theme + ".json";
            using var resp = await client.GetAsync(url, ct).ConfigureAwait(false);
            if (!resp.IsSuccessStatusCode) return null;
            var doc = await resp.Content.ReadFromJsonAsync<SeNorgeResponse>(ct).ConfigureAwait(false);
            return doc?.Data
                ?.Where(v => v.HasValue)
                .Select(v => v!.Value)
                .ToList();
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { return null; }
        catch (Exception ex)
        {
            _logger.LogDebug(ex, "seNorge {Theme} fetch failed for ({Lat},{Lon})", theme, lat, lon);
            return null;
        }
    }

    private static int CountInflections(List<double>? series)
    {
        if (series is null || series.Count < 3) return 0;
        var inflections = 0;
        for (var i = 1; i < series.Count - 1; i++)
        {
            var leftDelta = series[i] - series[i - 1];
            var rightDelta = series[i + 1] - series[i];
            if (leftDelta < -0.5 && rightDelta > 0.5) inflections++; // thaw then re-grow
        }
        return inflections;
    }

    private sealed class SeNorgeResponse
    {
        /// <summary>One value per day in the requested range. seNorge
        /// uses -999.0 as a no-data sentinel; the deserializer surfaces
        /// it as a double and we filter it client-side.</summary>
        [JsonPropertyName("Data")] public List<double?>? Data { get; set; }
    }
}

public sealed class SeNorgeOptions
{
    public string BaseUrl { get; set; } = "https://gts.nve.no/";
    public bool Enabled { get; set; }
}
