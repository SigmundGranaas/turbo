using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic weather generator used when no real provider is
/// configured. Produces plausible values for the given time and place so
/// the conditions UI has something to render in dev / test / staging
/// without hitting upstream APIs. Production deployments register
/// <see cref="MetNoWeatherProvider"/> instead by supplying
/// <c>MetNo:UserAgent</c> in configuration.
///
/// Determinism: same (lat, lon, hour) returns the same slice — the
/// generator hashes the inputs and shapes the output around climatic
/// rules of thumb (cooler with higher latitude, slight diurnal cycle,
/// pressure noise around 1013 hPa, occasional precipitation).
/// </summary>
public sealed class SyntheticWeatherProvider : IWeatherProvider
{
    public string Key => "synthetic_weather";

    public Task<WeatherSlice> GetAsync(
        double latitude, double longitude,
        DateTimeOffset at,
        CancellationToken cancellationToken)
    {
        var bucket = new DateTimeOffset(at.Year, at.Month, at.Day, at.Hour, 0, 0, TimeSpan.Zero);
        // Snap to grid using integer math so two points in the same cell
        // hash identically (avoids floating-point bit-representation
        // mismatches on round()). Subsequent climate calculations use
        // the snapped values too — otherwise two grid-mates produce the
        // same RNG sequence but different latitudinal-cosine factors.
        var latInt = (int)Math.Round(latitude * 100, MidpointRounding.ToEven);
        var lonInt = (int)Math.Round(longitude * 100, MidpointRounding.ToEven);
        var snappedLat = latInt / 100.0;
        var seed = HashCode.Combine(latInt, lonInt, bucket.ToUnixTimeSeconds());
        var rng = new Random(seed);

        // Latitudinal base + diurnal swing + small random noise.
        var latFactor = Math.Cos(snappedLat * Math.PI / 180.0);
        var diurnal = Math.Sin((at.Hour - 6) * Math.PI / 12.0);
        var baseTempC = (float)(latFactor * 18.0 + diurnal * 5.0 + (rng.NextDouble() - 0.5) * 3.0);

        var pressureHpa = (float)(1013.0 + (rng.NextDouble() - 0.5) * 20.0);
        var humidityPct = (float)(60.0 + (rng.NextDouble() - 0.5) * 30.0);
        var cloudPct = (float)(rng.NextDouble() * 100.0);
        var windMs = (float)(rng.NextDouble() * 8.0);
        var precip1h = rng.NextDouble() < 0.25 ? (float)(rng.NextDouble() * 3.0) : (float?)null;

        return Task.FromResult(new WeatherSlice(
            validAt: bucket,
            airTemperatureCelsius: baseTempC,
            airPressureHpa: pressureHpa,
            relativeHumidityPct: Math.Clamp(humidityPct, 0, 100),
            cloudCoveragePct: cloudPct,
            windSpeedMs: windMs,
            windGustMs: windMs * 1.6f,
            windFromDegrees: (float)(rng.NextDouble() * 360.0),
            precipitationNext1hMm: precip1h,
            precipitationNext6hMm: precip1h is null ? null : precip1h * 4,
            symbolCode: cloudPct > 80 ? "cloudy" : cloudPct > 40 ? "partlycloudy_day" : "clearsky_day"));
    }
}
