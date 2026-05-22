using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// NVE hydapi (hydapi.nve.no) river-flow provider. Fetches the last
/// 24h of discharge observations for the supplied station and projects
/// them into the typed <see cref="RiverFlowSlice"/>:
///   * CurrentCumecs = the most recent reading
///   * Trend = simple comparison of the most-recent reading vs the
///     average of the previous 24h (±10% threshold).
///
/// Endpoint:
///   /api/v1/Observations?StationId={code}&amp;Parameter=1001&amp;
///   ReferenceTime={from}/{to}&amp;ResolutionTime=60
/// (1001 = discharge / vannføring m³/s; ResolutionTime in minutes.)
///
/// Wiring: registered only when <c>Nve:ApiKey</c> is supplied;
/// otherwise <see cref="SyntheticRiverFlowProvider"/> is wired in
/// its place. The api key is sent via the <c>X-API-Key</c> header per
/// NVE's contract.
/// </summary>
public sealed class NveRiverFlowProvider : IRiverFlowProvider
{
    public const string HttpClientName = "nve-hydapi";

    public string Key => "nve_river_flow";

    private readonly IHttpClientFactory _http;
    private readonly ILogger<NveRiverFlowProvider> _logger;

    public NveRiverFlowProvider(IHttpClientFactory http, ILogger<NveRiverFlowProvider> logger)
    {
        _http = http;
        _logger = logger;
    }

    public async Task<RiverFlowSlice> GetAsync(
        string nveStationCode, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var client = _http.CreateClient(HttpClientName);
        var to = at.UtcDateTime;
        var from = to.AddDays(-1);
        var url = $"api/v1/Observations?StationId={Uri.EscapeDataString(nveStationCode)}"
                  + $"&Parameter=1001"
                  + $"&ReferenceTime={from:yyyy-MM-ddTHH:mm:ssZ}/{to:yyyy-MM-ddTHH:mm:ssZ}"
                  + $"&ResolutionTime=60";

        NveObservationsResponse? response;
        try
        {
            response = await client.GetFromJsonAsync<NveObservationsResponse>(url, cancellationToken);
        }
        catch (HttpRequestException ex)
        {
            throw new ConditionsProviderException("NVE upstream request failed", ex);
        }
        catch (JsonException ex)
        {
            throw new ConditionsProviderException("NVE returned malformed JSON", ex);
        }
        if (response is null)
            throw new ConditionsProviderException("NVE returned empty body");

        var observations = response.Data?.FirstOrDefault()?.Observations;
        if (observations is null || observations.Count == 0)
            throw new ConditionsProviderException($"NVE returned no observations for station {nveStationCode}");

        // Most recent first; if upstream returns ascending we'll pick the last.
        var sorted = observations.OrderBy(o => o.Time).ToList();
        var latest = sorted[^1];
        var earlier = sorted.Count > 1 ? (float)sorted.Take(sorted.Count - 1).Average(o => o.Value) : latest.Value;
        var ratio = (latest.Value - earlier) / Math.Max(earlier, 0.1f);
        var trend = ratio switch
        {
            > 0.10f => "rising",
            < -0.10f => "falling",
            _ => "stable",
        };

        return new RiverFlowSlice(
            stationCode: nveStationCode,
            validAt: latest.Time,
            currentCumecs: latest.Value,
            trend: trend);
    }
}

internal sealed record NveObservationsResponse
{
    [JsonPropertyName("data")] public List<NveDataEntry>? Data { get; init; }
}

internal sealed record NveDataEntry
{
    [JsonPropertyName("observations")] public List<NveObservation>? Observations { get; init; }
}

internal sealed record NveObservation
{
    [JsonPropertyName("time")] public DateTimeOffset Time { get; init; }
    [JsonPropertyName("value")] public float Value { get; init; }
}

public sealed class NveOptions
{
    /// <summary>X-API-Key value supplied by NVE.</summary>
    public string? ApiKey { get; set; }

    public string BaseUrl { get; set; } = "https://hydapi.nve.no/";
}
