using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Typed river-flow snapshot for one NVE station, modelled on the
/// hydapi.nve.no observation schema. <c>Trend</c> is "rising",
/// "stable", or "falling" — providers decide the threshold
/// (typically ±10% over 24h).
/// </summary>
public sealed record RiverFlowSlice
{
    [JsonPropertyName("stationCode")] public string StationCode { get; init; }
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("currentCumecs")] public float CurrentCumecs { get; init; }
    [JsonPropertyName("trend")] public string Trend { get; init; }

    [JsonConstructor]
    public RiverFlowSlice(string stationCode, DateTimeOffset validAt, float currentCumecs, string trend)
    {
        StationCode = stationCode ?? throw new ArgumentNullException(nameof(stationCode));
        ValidAt = validAt;
        CurrentCumecs = currentCumecs;
        Trend = trend ?? throw new ArgumentNullException(nameof(trend));
    }
}

/// <summary>
/// River-flow source. Packrafting's advisor consumes this; other kinds
/// may compose it later (e.g. fly fishing). Implementations live in
/// Shared.Infrastructure.
/// </summary>
public interface IRiverFlowProvider
{
    string Key { get; }

    Task<RiverFlowSlice> GetAsync(
        string nveStationCode,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
