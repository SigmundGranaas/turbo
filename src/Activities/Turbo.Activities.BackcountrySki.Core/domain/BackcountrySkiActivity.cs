using NetTopologySuite.Geometries;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.domain;

namespace Turboapi.Activities.BackcountrySki.domain;

/// <summary>
/// Backcountry ski kind aggregate. Composes <see cref="ActivityCore"/>
/// (identity, ownership, name, geometry, version) and adds typed
/// per-kind state — ascent/descent/distance, ATES rating, dominant
/// aspect, optional Varsom region for fast avalanche lookups, plus
/// owned aspect-mix and leg collections that become their own tables.
/// </summary>
public sealed class BackcountrySkiActivity
{
    public ActivityCore Core { get; private set; }
    public BackcountrySkiDetails Details { get; private set; }

    private BackcountrySkiActivity(ActivityCore core, BackcountrySkiDetails details)
    {
        Core = core;
        Details = details;
    }

    public static BackcountrySkiActivity Create(ActivityCore core, BackcountrySkiDetails details)
    {
        if (core.Geometry is not LineString)
            throw new ArgumentException(
                "Backcountry ski activities are route-based; pass a LineString geometry to ActivityCore.New",
                nameof(core));
        ArgumentNullException.ThrowIfNull(details);
        EnsureElevations(details);
        return new BackcountrySkiActivity(core, details);
    }

    public static BackcountrySkiActivity Reconstitute(ActivityCore core, BackcountrySkiDetails details)
        => new(core, details);

    public BackcountrySkiActivity Rename(string? name, string? description)
        => new(Core.WithRename(name, description), Details);

    public BackcountrySkiActivity ReplaceRoute(LineString newRoute)
        => new(Core.WithGeometry(newRoute), Details);

    public BackcountrySkiActivity ReplaceDetails(BackcountrySkiDetails details)
    {
        ArgumentNullException.ThrowIfNull(details);
        EnsureElevations(details);
        return new BackcountrySkiActivity(Core.BumpVersion(), details);
    }

    public BackcountrySkiActivity SoftDelete() => new(Core.WithSoftDelete(), Details);

    public LineString Route => (LineString)Core.Geometry;

    private static void EnsureElevations(BackcountrySkiDetails d)
    {
        if (d.ElevationMaxMeters < d.ElevationMinMeters)
            throw new ArgumentException(
                "elevationMaxMeters must be >= elevationMinMeters",
                nameof(d));
        if (d.AscentMeters < 0 || d.DescentMeters < 0 || d.DistanceMeters < 0)
            throw new ArgumentException(
                "ascent, descent, and distance must be non-negative",
                nameof(d));
        if (d.AspectMix is { Count: > 0 })
        {
            var total = d.AspectMix.Sum(a => a.Fraction);
            if (total < 0.99 || total > 1.01)
                throw new ArgumentException(
                    $"aspectMix fractions must sum to 1.0 (got {total:F3})",
                    nameof(d));
        }
    }
}
