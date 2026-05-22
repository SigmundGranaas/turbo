using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Hiking.value;

namespace Turboapi.Activities.Hiking.domain;

/// <summary>
/// Hiking kind aggregate. Same composition pattern as the other kinds —
/// holds an <see cref="ActivityCore"/> value object plus its own typed
/// <see cref="HikingDetails"/> payload. LineString geometry only.
/// </summary>
public sealed class HikingActivity
{
    public ActivityCore Core { get; private set; }
    public HikingDetails Details { get; private set; }

    private HikingActivity(ActivityCore core, HikingDetails details)
    {
        Core = core;
        Details = details;
    }

    public static HikingActivity Create(ActivityCore core, HikingDetails details)
    {
        if (core.Geometry is not LineString)
            throw new ArgumentException(
                "Hiking activities are route-based; pass a LineString geometry to ActivityCore.New",
                nameof(core));
        ArgumentNullException.ThrowIfNull(details);
        EnsureValid(details);
        return new HikingActivity(core, details);
    }

    public static HikingActivity Reconstitute(ActivityCore core, HikingDetails details) => new(core, details);

    public HikingActivity Rename(string? name, string? description) => new(Core.WithRename(name, description), Details);
    public HikingActivity ReplaceRoute(LineString newRoute) => new(Core.WithGeometry(newRoute), Details);
    public HikingActivity ReplaceDetails(HikingDetails details)
    {
        ArgumentNullException.ThrowIfNull(details);
        EnsureValid(details);
        return new HikingActivity(Core.BumpVersion(), details);
    }
    public HikingActivity SoftDelete() => new(Core.WithSoftDelete(), Details);

    public LineString Route => (LineString)Core.Geometry;

    private static void EnsureValid(HikingDetails d)
    {
        if (d.DistanceMeters < 0 || d.AscentMeters < 0 || d.DescentMeters < 0)
            throw new ArgumentException("distance, ascent, descent must be non-negative", nameof(d));
        if (d.ElevationMaxMeters < d.ElevationMinMeters)
            throw new ArgumentException("elevationMaxMeters must be >= elevationMinMeters", nameof(d));
        if (d.EstimatedHours is { } h && h < 0)
            throw new ArgumentException("estimatedHours must be non-negative", nameof(d));
    }
}
