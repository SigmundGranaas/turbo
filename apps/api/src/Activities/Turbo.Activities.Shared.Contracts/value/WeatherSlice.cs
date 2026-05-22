using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Typed payload of weather conditions for one location/time. Mirrors
/// the subset of met.no's locationforecast/2.0 compact response that the
/// kind-specific advisors actually care about. Cached as JSON bytes.
/// </summary>
public sealed record WeatherSlice
{
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }

    [JsonPropertyName("airTemperatureCelsius")] public float AirTemperatureCelsius { get; init; }
    [JsonPropertyName("airPressureHpa")] public float AirPressureHpa { get; init; }
    [JsonPropertyName("relativeHumidityPct")] public float RelativeHumidityPct { get; init; }
    [JsonPropertyName("cloudCoveragePct")] public float CloudCoveragePct { get; init; }

    [JsonPropertyName("windSpeedMs")] public float WindSpeedMs { get; init; }
    [JsonPropertyName("windGustMs")] public float? WindGustMs { get; init; }
    [JsonPropertyName("windFromDegrees")] public float WindFromDegrees { get; init; }

    [JsonPropertyName("precipitationNext1hMm")] public float? PrecipitationNext1hMm { get; init; }
    [JsonPropertyName("precipitationNext6hMm")] public float? PrecipitationNext6hMm { get; init; }

    /// <summary>met.no symbol_code from next_1_hours.summary when
    /// available — opaque to the server, surfaced to the client for
    /// rendering. Examples: "rain", "partlycloudy_day", "snow".</summary>
    [JsonPropertyName("symbolCode")] public string? SymbolCode { get; init; }

    [JsonConstructor]
    public WeatherSlice(
        DateTimeOffset validAt,
        float airTemperatureCelsius, float airPressureHpa,
        float relativeHumidityPct, float cloudCoveragePct,
        float windSpeedMs, float? windGustMs, float windFromDegrees,
        float? precipitationNext1hMm, float? precipitationNext6hMm,
        string? symbolCode)
    {
        ValidAt = validAt;
        AirTemperatureCelsius = airTemperatureCelsius;
        AirPressureHpa = airPressureHpa;
        RelativeHumidityPct = relativeHumidityPct;
        CloudCoveragePct = cloudCoveragePct;
        WindSpeedMs = windSpeedMs;
        WindGustMs = windGustMs;
        WindFromDegrees = windFromDegrees;
        PrecipitationNext1hMm = precipitationNext1hMm;
        PrecipitationNext6hMm = precipitationNext6hMm;
        SymbolCode = symbolCode;
    }
}

/// <summary>
/// Weather provider interface. Implementations either hit a real API
/// (MetNoWeatherProvider) or synthesize deterministic data
/// (SyntheticWeatherProvider). Composition: per-kind advisors take
/// this as a constructor dependency.
/// </summary>
public interface IWeatherProvider
{
    string Key { get; }
    Task<WeatherSlice> GetAsync(
        double latitude, double longitude,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
