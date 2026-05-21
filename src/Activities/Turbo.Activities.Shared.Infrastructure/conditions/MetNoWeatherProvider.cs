using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Extensions.Logging;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// met.no Locationforecast 2.0 (compact) provider. Hits
/// api.met.no/weatherapi/locationforecast/2.0/compact, which requires
/// a unique User-Agent string identifying the caller — wire one through
/// <see cref="MetNoOptions.UserAgent"/> in configuration.
///
/// Response shape is documented at
/// https://api.met.no/weatherapi/locationforecast/2.0/documentation.
/// We pick the timeseries entry nearest the requested instant and
/// project it into <see cref="WeatherSlice"/>. The cache layer in front
/// (see <c>FishingConditionsAdvisor</c>) keeps repeat lookups for
/// nearby points / same hour from re-hitting the upstream.
/// </summary>
public sealed class MetNoWeatherProvider : IWeatherProvider
{
    public string Key => "met_no_weather";

    public const string HttpClientName = "met-no";

    private readonly IHttpClientFactory _http;
    private readonly ILogger<MetNoWeatherProvider> _logger;

    public MetNoWeatherProvider(IHttpClientFactory http, ILogger<MetNoWeatherProvider> logger)
    {
        _http = http;
        _logger = logger;
    }

    public async Task<WeatherSlice> GetAsync(
        double latitude, double longitude,
        DateTimeOffset at,
        CancellationToken cancellationToken)
    {
        var client = _http.CreateClient(HttpClientName);
        // met.no requires lat/lon clamped to 4 decimals max.
        var url = $"weatherapi/locationforecast/2.0/compact?lat={Math.Round(latitude, 4)}&lon={Math.Round(longitude, 4)}";

        MetNoForecast? response;
        try
        {
            response = await client.GetFromJsonAsync<MetNoForecast>(url, cancellationToken);
        }
        catch (HttpRequestException ex)
        {
            throw new ConditionsProviderException("met.no upstream request failed", ex);
        }
        catch (JsonException ex)
        {
            throw new ConditionsProviderException("met.no returned malformed JSON", ex);
        }
        if (response is null)
            throw new ConditionsProviderException("met.no returned empty body");

        var nearest = NearestTimeseries(response, at)
                      ?? throw new ConditionsProviderException(
                          $"met.no returned no timeseries entries usable for instant {at:O}");

        var instant = nearest.Data.Instant.Details;
        var next1h = nearest.Data.Next1Hours;
        var next6h = nearest.Data.Next6Hours;

        return new WeatherSlice(
            validAt: nearest.Time,
            airTemperatureCelsius: instant.AirTemperature,
            airPressureHpa: instant.AirPressureAtSeaLevel,
            relativeHumidityPct: instant.RelativeHumidity,
            cloudCoveragePct: instant.CloudAreaFraction,
            windSpeedMs: instant.WindSpeed,
            windGustMs: instant.WindSpeedOfGust,
            windFromDegrees: instant.WindFromDirection,
            precipitationNext1hMm: next1h?.Details?.PrecipitationAmount,
            precipitationNext6hMm: next6h?.Details?.PrecipitationAmount,
            symbolCode: next1h?.Summary?.SymbolCode ?? next6h?.Summary?.SymbolCode);
    }

    private static MetNoTimeseriesEntry? NearestTimeseries(MetNoForecast response, DateTimeOffset at)
    {
        var entries = response.Properties.Timeseries;
        if (entries is null || entries.Count == 0) return null;

        MetNoTimeseriesEntry? best = null;
        var bestDelta = TimeSpan.MaxValue;
        foreach (var e in entries)
        {
            var delta = (e.Time - at).Duration();
            if (delta < bestDelta) { best = e; bestDelta = delta; }
        }
        return best;
    }
}

public sealed class MetNoOptions
{
    /// <summary>
    /// User-Agent string required by met.no's TOS. Must identify the
    /// application + contact email so met.no can reach out about
    /// abusive usage. Example: <c>TurboActivities/0.1 ops@example.com</c>.
    /// </summary>
    public string? UserAgent { get; set; }

    /// <summary>Base URL override; defaults to api.met.no.</summary>
    public string BaseUrl { get; set; } = "https://api.met.no/";
}

// === met.no compact response shape ===

internal sealed record MetNoForecast
{
    [JsonPropertyName("properties")] public MetNoProperties Properties { get; init; } = default!;
}

internal sealed record MetNoProperties
{
    [JsonPropertyName("timeseries")] public List<MetNoTimeseriesEntry> Timeseries { get; init; } = new();
}

internal sealed record MetNoTimeseriesEntry
{
    [JsonPropertyName("time")] public DateTimeOffset Time { get; init; }
    [JsonPropertyName("data")] public MetNoTimeData Data { get; init; } = default!;
}

internal sealed record MetNoTimeData
{
    [JsonPropertyName("instant")] public MetNoInstant Instant { get; init; } = default!;
    [JsonPropertyName("next_1_hours")] public MetNoHorizon? Next1Hours { get; init; }
    [JsonPropertyName("next_6_hours")] public MetNoHorizon? Next6Hours { get; init; }
}

internal sealed record MetNoInstant
{
    [JsonPropertyName("details")] public MetNoInstantDetails Details { get; init; } = default!;
}

internal sealed record MetNoInstantDetails
{
    [JsonPropertyName("air_temperature")] public float AirTemperature { get; init; }
    [JsonPropertyName("air_pressure_at_sea_level")] public float AirPressureAtSeaLevel { get; init; }
    [JsonPropertyName("relative_humidity")] public float RelativeHumidity { get; init; }
    [JsonPropertyName("cloud_area_fraction")] public float CloudAreaFraction { get; init; }
    [JsonPropertyName("wind_speed")] public float WindSpeed { get; init; }
    [JsonPropertyName("wind_speed_of_gust")] public float? WindSpeedOfGust { get; init; }
    [JsonPropertyName("wind_from_direction")] public float WindFromDirection { get; init; }
}

internal sealed record MetNoHorizon
{
    [JsonPropertyName("summary")] public MetNoSummary? Summary { get; init; }
    [JsonPropertyName("details")] public MetNoHorizonDetails? Details { get; init; }
}

internal sealed record MetNoSummary
{
    [JsonPropertyName("symbol_code")] public string? SymbolCode { get; init; }
}

internal sealed record MetNoHorizonDetails
{
    [JsonPropertyName("precipitation_amount")] public float? PrecipitationAmount { get; init; }
}
