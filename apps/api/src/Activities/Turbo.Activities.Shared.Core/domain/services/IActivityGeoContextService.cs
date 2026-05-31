using NetTopologySuite.Geometries;

namespace Turboapi.Activities.domain.services;

/// <summary>
/// Computes — and caches against the geometry hash — every attribute of an
/// activity that is a pure function of its geometry: elevation profile,
/// slope and aspect histograms, the Varsom/Mareano region it sits in, the
/// nearest NVE hydrometric stations, treeline crossings, watershed id.
/// Each kind's create/update handler calls
/// <see cref="ComputeAndStoreAsync"/> after geometry normalization;
/// orchestrators read the persisted context cheaply at analysis time.
/// </summary>
public interface IActivityGeoContextService
{
    /// <summary>Compute (or recompute when geometry has changed) the geo
    /// context for an activity and persist it.</summary>
    Task<ActivityGeoContext> ComputeAndStoreAsync(
        Guid activityId,
        Geometry geometry,
        CancellationToken cancellationToken);

    /// <summary>Read the persisted geo context for an activity, or
    /// <c>null</c> if it has not been computed yet.</summary>
    Task<ActivityGeoContext?> GetAsync(Guid activityId, CancellationToken cancellationToken);

    /// <summary>Compute a context against an arbitrary geometry without
    /// persisting it. Used by the create-flow's "derived preview" step
    /// before the activity itself exists.</summary>
    Task<ActivityGeoContext> ComputeTransientAsync(Geometry geometry, CancellationToken cancellationToken);
}

/// <summary>
/// Deterministic, geometry-derived attributes for one activity. Stored as
/// a versioned jsonb blob keyed on a hash of the geometry — recompute is
/// triggered only when the geometry changes. Replaces the user-entered
/// mirror fields on per-kind details (ascent, aspect, Varsom region, …).
/// </summary>
public sealed record ActivityGeoContext
{
    public Guid ActivityId { get; init; }
    public int Version { get; init; }

    public double ElevationMinM { get; init; }
    public double ElevationMaxM { get; init; }
    public double AscentM { get; init; }
    public double DescentM { get; init; }
    public double LengthM { get; init; }

    /// <summary>8-bin aspect histogram (N, NE, E, SE, S, SW, W, NW).
    /// Fractions sum to 1 over segments steep enough to have a
    /// meaningful aspect (slope &gt; ~5°). Empty list for point
    /// geometries.</summary>
    public IReadOnlyList<AspectShare> AspectMix { get; init; }

    /// <summary>5°-bin slope histogram (0–5°, 5–10°, …, 50°+).
    /// Fractions sum to 1.</summary>
    public IReadOnlyList<SlopeBin> SlopeHistogram { get; init; }

    public int? VarsomRegionId { get; init; }
    public string? VarsomRegionName { get; init; }

    public int? MareanoCellId { get; init; }

    /// <summary>NVE REGINE watershed id. Lets cross-activity signals share
    /// upstream-precipitation context (e.g. river-mouth freediving reads
    /// upstream packrafting observations on the same watershed).</summary>
    public string? WatershedHrefId { get; init; }

    /// <summary>Top-3 nearest NVE hydrometric stations, ordered by
    /// distance. Empty when no station within a reasonable radius
    /// (geometry too far from monitored water).</summary>
    public IReadOnlyList<NearestStation> NveStations { get; init; }

    public int? TreelineCrossings { get; init; }
    public double? AboveTreelineFractionM { get; init; }

    public bool TouchesCoastline { get; init; }
    public double? DistanceToCoastM { get; init; }

    public DateTime ComputedAt { get; init; }

    public ActivityGeoContext(
        Guid activityId, int version,
        double elevationMinM, double elevationMaxM, double ascentM, double descentM, double lengthM,
        IReadOnlyList<AspectShare> aspectMix, IReadOnlyList<SlopeBin> slopeHistogram,
        int? varsomRegionId, string? varsomRegionName,
        int? mareanoCellId, string? watershedHrefId,
        IReadOnlyList<NearestStation> nveStations,
        int? treelineCrossings, double? aboveTreelineFractionM,
        bool touchesCoastline, double? distanceToCoastM,
        DateTime computedAt)
    {
        ActivityId = activityId;
        Version = version;
        ElevationMinM = elevationMinM;
        ElevationMaxM = elevationMaxM;
        AscentM = ascentM;
        DescentM = descentM;
        LengthM = lengthM;
        AspectMix = aspectMix ?? Array.Empty<AspectShare>();
        SlopeHistogram = slopeHistogram ?? Array.Empty<SlopeBin>();
        VarsomRegionId = varsomRegionId;
        VarsomRegionName = varsomRegionName;
        MareanoCellId = mareanoCellId;
        WatershedHrefId = watershedHrefId;
        NveStations = nveStations ?? Array.Empty<NearestStation>();
        TreelineCrossings = treelineCrossings;
        AboveTreelineFractionM = aboveTreelineFractionM;
        TouchesCoastline = touchesCoastline;
        DistanceToCoastM = distanceToCoastM;
        ComputedAt = computedAt;
    }
}

public sealed record AspectShare(Aspect Aspect, double Fraction);

public sealed record SlopeBin(int MinDegrees, int MaxDegrees, double Fraction);

public sealed record NearestStation(string Code, string Name, double DistanceM);

public enum Aspect
{
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}
