using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Coastal water turbidity at a point — the direct upstream the
/// freediving visibility model wants. In production this comes from
/// recent Sentinel-2 turbidity products (or a derived Vannmiljø proxy);
/// in dev / staging it's deterministically synthesized.
///
/// <see cref="TurbidityNtu"/> is in nephelometric turbidity units (NTU).
/// Common Norwegian fjord ranges: 0–2 clear, 2–8 moderate, &gt;8 silty.
/// <see cref="CloudCoveragePct"/> is the satellite cloud cover at the
/// pixel at the time the image was captured — low clouds = high
/// confidence; heavy clouds = the value is interpolated and the
/// orchestrator should weight it lower.
/// </summary>
public sealed record TurbiditySlice
{
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("turbidityNtu")] public double TurbidityNtu { get; init; }
    [JsonPropertyName("cloudCoveragePct")] public double CloudCoveragePct { get; init; }
    [JsonPropertyName("ageHours")] public int AgeHours { get; init; }

    [JsonConstructor]
    public TurbiditySlice(
        DateTimeOffset validAt,
        double turbidityNtu,
        double cloudCoveragePct,
        int ageHours)
    {
        ValidAt = validAt;
        TurbidityNtu = turbidityNtu;
        CloudCoveragePct = cloudCoveragePct;
        AgeHours = ageHours;
    }
}

/// <summary>
/// Coastal turbidity source. Implementations live in
/// Shared.Infrastructure. Cached aggressively — turbidity changes over
/// hours-to-days, not minutes.
/// </summary>
public interface ITurbidityProvider
{
    string Key { get; }

    Task<TurbiditySlice> GetAsync(
        double latitude, double longitude,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
