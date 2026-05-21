using System.Text.Json.Serialization;

namespace Turboapi.Activities.BackcountrySki.value;

/// <summary>
/// Typed payload of backcountry-ski-specific fields. Every field is
/// typed; nothing JSONB-shaped. Owned-collection-style fields
/// (Aspects, Legs) become their own tables on the read side.
/// </summary>
public sealed record BackcountrySkiDetails
{
    [JsonPropertyName("ascentMeters")] public int AscentMeters { get; init; }
    [JsonPropertyName("descentMeters")] public int DescentMeters { get; init; }
    [JsonPropertyName("distanceMeters")] public int DistanceMeters { get; init; }
    [JsonPropertyName("elevationMinMeters")] public int ElevationMinMeters { get; init; }
    [JsonPropertyName("elevationMaxMeters")] public int ElevationMaxMeters { get; init; }

    [JsonPropertyName("atesRating")] public AtesRating AtesRating { get; init; }
    [JsonPropertyName("dominantAspect")] public Aspect? DominantAspect { get; init; }
    [JsonPropertyName("varsomRegionId")] public int? VarsomRegionId { get; init; }
    [JsonPropertyName("preferredAvalancheMaxLevel")] public short? PreferredAvalancheMaxLevel { get; init; }

    [JsonPropertyName("aspectMix")] public IReadOnlyList<AspectShare> AspectMix { get; init; } = Array.Empty<AspectShare>();
    [JsonPropertyName("legs")] public IReadOnlyList<RouteLeg> Legs { get; init; } = Array.Empty<RouteLeg>();

    [JsonConstructor]
    public BackcountrySkiDetails(
        int ascentMeters,
        int descentMeters,
        int distanceMeters,
        int elevationMinMeters,
        int elevationMaxMeters,
        AtesRating atesRating,
        Aspect? dominantAspect,
        int? varsomRegionId,
        short? preferredAvalancheMaxLevel,
        IReadOnlyList<AspectShare>? aspectMix,
        IReadOnlyList<RouteLeg>? legs)
    {
        AscentMeters = ascentMeters;
        DescentMeters = descentMeters;
        DistanceMeters = distanceMeters;
        ElevationMinMeters = elevationMinMeters;
        ElevationMaxMeters = elevationMaxMeters;
        AtesRating = atesRating;
        DominantAspect = dominantAspect;
        VarsomRegionId = varsomRegionId;
        PreferredAvalancheMaxLevel = preferredAvalancheMaxLevel;
        AspectMix = aspectMix ?? Array.Empty<AspectShare>();
        Legs = legs ?? Array.Empty<RouteLeg>();
    }
}

/// <summary>Fraction of the route that faces a given aspect (0..1).</summary>
public sealed record AspectShare
{
    [JsonPropertyName("aspect")] public Aspect Aspect { get; init; }
    [JsonPropertyName("fraction")] public float Fraction { get; init; }

    [JsonConstructor]
    public AspectShare(Aspect aspect, float fraction)
    {
        Aspect = aspect;
        Fraction = fraction;
    }
}

/// <summary>One segment of the route — typically an ascent, descent, or
/// traverse leg with its own start/end elevation. Geometry of the leg is
/// a substring of the parent route's LineString.</summary>
public sealed record RouteLeg
{
    [JsonPropertyName("kind")] public LegKind Kind { get; init; }
    [JsonPropertyName("startElevationMeters")] public int StartElevationMeters { get; init; }
    [JsonPropertyName("endElevationMeters")] public int EndElevationMeters { get; init; }
    [JsonPropertyName("polylineWkt")] public string PolylineWkt { get; init; }

    [JsonConstructor]
    public RouteLeg(LegKind kind, int startElevationMeters, int endElevationMeters, string polylineWkt)
    {
        Kind = kind;
        StartElevationMeters = startElevationMeters;
        EndElevationMeters = endElevationMeters;
        PolylineWkt = polylineWkt ?? throw new ArgumentNullException(nameof(polylineWkt));
    }
}
