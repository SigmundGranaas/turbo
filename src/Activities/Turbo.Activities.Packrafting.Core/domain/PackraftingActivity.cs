using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Packrafting.value;

namespace Turboapi.Activities.Packrafting.domain;

public sealed class PackraftingActivity
{
    public ActivityCore Core { get; private set; }
    public PackraftingDetails Details { get; private set; }

    private PackraftingActivity(ActivityCore core, PackraftingDetails details)
    { Core = core; Details = details; }

    public static PackraftingActivity Create(ActivityCore core, PackraftingDetails details)
    {
        if (core.Geometry is not LineString)
            throw new ArgumentException(
                "Packrafting activities are route-based; pass a LineString geometry to ActivityCore.New",
                nameof(core));
        ArgumentNullException.ThrowIfNull(details);
        EnsureValid(details);
        return new PackraftingActivity(core, details);
    }

    public static PackraftingActivity Reconstitute(ActivityCore core, PackraftingDetails details) => new(core, details);

    public PackraftingActivity Rename(string? name, string? description) => new(Core.WithRename(name, description), Details);
    public PackraftingActivity ReplaceRoute(LineString newRoute) => new(Core.WithGeometry(newRoute), Details);
    public PackraftingActivity ReplaceDetails(PackraftingDetails details)
    {
        ArgumentNullException.ThrowIfNull(details);
        EnsureValid(details);
        return new PackraftingActivity(Core.BumpVersion(), details);
    }
    public PackraftingActivity SoftDelete() => new(Core.WithSoftDelete(), Details);

    public LineString Route => (LineString)Core.Geometry;

    private static void EnsureValid(PackraftingDetails d)
    {
        if (d.DistanceMeters < 0 || d.PaddleDistanceMeters < 0 || d.PortageDistanceMeters < 0)
            throw new ArgumentException("distances must be non-negative", nameof(d));
        if (d.PaddleDistanceMeters + d.PortageDistanceMeters > d.DistanceMeters * 11 / 10)
            // Allow 10% over-count for measurement noise; otherwise reject.
            throw new ArgumentException(
                "paddle + portage distances must not significantly exceed total distance", nameof(d));
        if (d.TypicalGrade > d.MaxGrade)
            throw new ArgumentException("typicalGrade must not exceed maxGrade", nameof(d));
    }
}
