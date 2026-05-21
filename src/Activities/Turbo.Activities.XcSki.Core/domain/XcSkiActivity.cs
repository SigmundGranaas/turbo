using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.domain;

public sealed class XcSkiActivity
{
    public ActivityCore Core { get; private set; }
    public XcSkiDetails Details { get; private set; }

    private XcSkiActivity(ActivityCore core, XcSkiDetails details)
    {
        Core = core;
        Details = details;
    }

    public static XcSkiActivity Create(ActivityCore core, XcSkiDetails details)
    {
        if (core.Geometry is not LineString)
            throw new ArgumentException(
                "XC ski activities are trail-based; pass a LineString geometry to ActivityCore.New",
                nameof(core));
        ArgumentNullException.ThrowIfNull(details);
        EnsureValid(details);
        return new XcSkiActivity(core, details);
    }

    public static XcSkiActivity Reconstitute(ActivityCore core, XcSkiDetails details) => new(core, details);

    public XcSkiActivity Rename(string? name, string? description) => new(Core.WithRename(name, description), Details);
    public XcSkiActivity ReplaceRoute(LineString newRoute) => new(Core.WithGeometry(newRoute), Details);
    public XcSkiActivity ReplaceDetails(XcSkiDetails details)
    {
        ArgumentNullException.ThrowIfNull(details);
        EnsureValid(details);
        return new XcSkiActivity(Core.BumpVersion(), details);
    }
    public XcSkiActivity SoftDelete() => new(Core.WithSoftDelete(), Details);

    public LineString Route => (LineString)Core.Geometry;

    private static void EnsureValid(XcSkiDetails d)
    {
        if (d.DistanceMeters < 0 || d.AscentMeters < 0 || d.DescentMeters < 0)
            throw new ArgumentException("distance, ascent, descent must be non-negative", nameof(d));
    }
}
