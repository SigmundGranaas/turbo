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

    /// <summary>
    /// Fetch the full available forecast timeseries for a point in a single
    /// upstream call (met.no returns hourly for ~2 days then 6-hourly out to
    /// ~10 days in one response). Callers slice this into now / hourly / daily
    /// views — much cheaper than calling <see cref="GetAsync"/> once per sample.
    ///
    /// The default samples <see cref="GetAsync"/> at a fixed cadence so cheap /
    /// synthetic providers work without bespoke code; real upstream providers
    /// (and the cache decorator) override it to fetch / cache the series once.
    /// </summary>
    async Task<IReadOnlyList<WeatherSlice>> GetForecastAsync(
        double latitude, double longitude,
        CancellationToken cancellationToken)
    {
        var now = DateTimeOffset.UtcNow;
        var midnight = new DateTimeOffset(now.UtcDateTime.Date, TimeSpan.Zero);
        var slices = new List<WeatherSlice>();
        for (var h = 0; h <= 24; h += 3)
            slices.Add(await GetAsync(latitude, longitude, now.AddHours(h), cancellationToken));
        for (var d = 1; d <= 7; d++)
            slices.Add(await GetAsync(latitude, longitude, midnight.AddDays(d).AddHours(12), cancellationToken));
        return slices;
    }
}
