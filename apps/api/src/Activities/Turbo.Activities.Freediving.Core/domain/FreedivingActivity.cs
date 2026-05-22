using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Freediving.value;

namespace Turboapi.Activities.Freediving.domain;

public sealed class FreedivingActivity
{
    public ActivityCore Core { get; private set; }
    public FreedivingDetails Details { get; private set; }

    private FreedivingActivity(ActivityCore core, FreedivingDetails details)
    { Core = core; Details = details; }

    public static FreedivingActivity Create(ActivityCore core, FreedivingDetails details)
    {
        if (core.Geometry is not Point)
            throw new ArgumentException(
                "Freediving activities are spot-based; pass a Point geometry to ActivityCore.New",
                nameof(core));
        ArgumentNullException.ThrowIfNull(details);
        EnsureValid(details);
        return new FreedivingActivity(core, details);
    }

    public static FreedivingActivity Reconstitute(ActivityCore core, FreedivingDetails details) => new(core, details);

    public FreedivingActivity Rename(string? name, string? description) => new(Core.WithRename(name, description), Details);
    public FreedivingActivity Relocate(Point newPosition) => new(Core.WithGeometry(newPosition), Details);
    public FreedivingActivity ReplaceDetails(FreedivingDetails details)
    {
        ArgumentNullException.ThrowIfNull(details);
        EnsureValid(details);
        return new FreedivingActivity(Core.BumpVersion(), details);
    }
    public FreedivingActivity SoftDelete() => new(Core.WithSoftDelete(), Details);

    public Point Position => (Point)Core.Geometry;

    private static void EnsureValid(FreedivingDetails d)
    {
        if (d.MaxDepthMeters < 0) throw new ArgumentException("maxDepthMeters must be non-negative", nameof(d));
        if (d.MaxDepthMeters > 200) throw new ArgumentException("maxDepthMeters > 200m is implausible for a freediving spot", nameof(d));
        if (d.TypicalVisibilityMeters is { } v && v < 0) throw new ArgumentException("typicalVisibilityMeters must be non-negative", nameof(d));
    }
}
