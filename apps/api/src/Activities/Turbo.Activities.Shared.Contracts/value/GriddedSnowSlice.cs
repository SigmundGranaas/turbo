using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Gridded snow state at a location/day from NVE's seNorge model.
/// Drives "is there snow to ski on, how fresh, how stable" decisions for
/// both ski kinds.
/// </summary>
public sealed record GriddedSnowSlice
{
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }

    /// <summary>Snow-water equivalent (mm).</summary>
    [JsonPropertyName("sweMm")] public double SweMm { get; init; }

    /// <summary>Snow depth (cm).</summary>
    [JsonPropertyName("snowDepthCm")] public double SnowDepthCm { get; init; }

    /// <summary>Fresh snow accumulated in the last 24h (cm).</summary>
    [JsonPropertyName("freshSnowLast24hCm")] public double FreshSnowLast24hCm { get; init; }

    /// <summary>Count of freeze/thaw cycles in the last 7 days. Predicts
    /// breakable crust / icy track conditions.</summary>
    [JsonPropertyName("freezeThawLast7d")] public int FreezeThawLast7d { get; init; }

    [JsonConstructor]
    public GriddedSnowSlice(
        DateTimeOffset validAt,
        double sweMm,
        double snowDepthCm,
        double freshSnowLast24hCm,
        int freezeThawLast7d)
    {
        ValidAt = validAt;
        SweMm = sweMm;
        SnowDepthCm = snowDepthCm;
        FreshSnowLast24hCm = freshSnowLast24hCm;
        FreezeThawLast7d = freezeThawLast7d;
    }
}

/// <summary>
/// seNorge-backed source. 1 km grid; clients should round lat/lon before
/// calling so a single grid cell is reused across nearby spots.
/// </summary>
public interface IGriddedSnowProvider
{
    string Key { get; }

    Task<GriddedSnowSlice> GetAsync(
        double latitude, double longitude,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
