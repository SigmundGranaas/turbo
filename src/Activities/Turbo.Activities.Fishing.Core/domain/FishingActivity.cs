using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.domain;

/// <summary>
/// Fishing kind aggregate root. Composes <see cref="ActivityCore"/> for
/// identity/ownership/naming/geometry/versioning; carries fishing-specific
/// fields as typed first-class properties. No JSON catch-all; the
/// fishing.activities table mirrors each field as its own column (or
/// owned-collection table for target species + depth samples).
/// </summary>
public sealed class FishingActivity
{
    public ActivityCore Core { get; private set; }
    public FishingDetails Details { get; private set; }

    private FishingActivity(ActivityCore core, FishingDetails details)
    {
        Core = core;
        Details = details;
    }

    public static FishingActivity Create(ActivityCore core, FishingDetails details)
    {
        if (core.Geometry is not Point)
            throw new ArgumentException(
                "Fishing activities are point-based; pass a Point geometry to ActivityCore.New",
                nameof(core));
        ArgumentNullException.ThrowIfNull(details);
        return new FishingActivity(core, details);
    }

    public static FishingActivity Reconstitute(ActivityCore core, FishingDetails details)
        => new(core, details);

    public FishingActivity Rename(string? name, string? description)
        => new(Core.WithRename(name, description), Details);

    public FishingActivity Relocate(Point newGeometry)
        => new(Core.WithGeometry(newGeometry), Details);

    public FishingActivity ReplaceDetails(FishingDetails details)
    {
        ArgumentNullException.ThrowIfNull(details);
        return new FishingActivity(Core.BumpVersion(), details);
    }

    public FishingActivity SoftDelete()
        => new(Core.WithSoftDelete(), Details);

    public Point Position => (Point)Core.Geometry;
}
