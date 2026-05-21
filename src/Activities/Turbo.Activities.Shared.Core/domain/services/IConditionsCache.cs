using NetTopologySuite.Geometries;
using Turboapi.Activities.value;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// Server-side conditions cache. Each entry is a typed payload (serialized
/// to bytes) keyed by (provider, grid cell, time bucket). Providers snap
/// geometry to a grid cell (~0.01° ≈ 1km in lat) and time to a per-
/// provider bucket (hourly for weather, 6h for tides, 24h for avalanche)
/// before hitting the cache — so two requests for nearby points within
/// the same window share an upstream call.
/// </summary>
public interface IConditionsCache
{
    Task<CachedConditionsSlice?> TryGetAsync(
        string providerKey,
        string gridCell,
        DateTimeOffset timeBucket,
        CancellationToken cancellationToken);

    Task PutAsync(
        string providerKey,
        string gridCell,
        DateTimeOffset timeBucket,
        ReadOnlyMemory<byte> payload,
        DateTimeOffset fetchedAt,
        DateTimeOffset expiresAt,
        CancellationToken cancellationToken);
}

public sealed record CachedConditionsSlice(
    string ProviderKey,
    string GridCell,
    DateTimeOffset TimeBucket,
    ReadOnlyMemory<byte> Payload,
    DateTimeOffset FetchedAt,
    DateTimeOffset ExpiresAt);

/// <summary>
/// Helpers for snapping geometry and time to cache cells. Lives in
/// Core (not Infrastructure) so providers can use it without taking an
/// EF dependency.
/// </summary>
public static class ConditionsCacheKey
{
    /// <summary>Snap a geometry's centroid to a 0.01°-resolution grid
    /// string like "60.12_5.32". Two points within ~1km of each other
    /// in latitude / ~600m–1km in longitude (depending on latitude)
    /// share the same cell.</summary>
    public static string GridCellFor(Geometry geometry)
    {
        ArgumentNullException.ThrowIfNull(geometry);
        var c = geometry.Centroid.Coordinate;
        var lat = Math.Round(c.Y, 2, MidpointRounding.ToEven);
        var lon = Math.Round(c.X, 2, MidpointRounding.ToEven);
        return $"{lat:F2}_{lon:F2}";
    }

    /// <summary>Snap a timestamp to the start of its hour (UTC).</summary>
    public static DateTimeOffset HourBucket(DateTimeOffset at) =>
        new DateTimeOffset(at.Year, at.Month, at.Day, at.Hour, 0, 0, TimeSpan.Zero);

    /// <summary>Snap a timestamp to a 6h bucket (UTC).</summary>
    public static DateTimeOffset SixHourBucket(DateTimeOffset at)
    {
        var slot = at.Hour - (at.Hour % 6);
        return new DateTimeOffset(at.Year, at.Month, at.Day, slot, 0, 0, TimeSpan.Zero);
    }

    /// <summary>Snap a timestamp to the start of its day (UTC).</summary>
    public static DateTimeOffset DayBucket(DateTimeOffset at) =>
        new DateTimeOffset(at.Year, at.Month, at.Day, 0, 0, 0, TimeSpan.Zero);
}
