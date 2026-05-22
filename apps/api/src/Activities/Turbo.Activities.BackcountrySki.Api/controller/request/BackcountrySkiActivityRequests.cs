using Turboapi.Activities.BackcountrySki.value;

namespace Turboapi.Activities.BackcountrySki.controller.request;

public sealed class CreateBackcountrySkiActivityRequest
{
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }

    /// <summary>WKT LINESTRING in EPSG:4326.</summary>
    public string RouteWkt { get; set; } = string.Empty;

    public BackcountrySkiDetailsDto Details { get; set; } = new();
}

public sealed class UpdateBackcountrySkiActivityRequest
{
    public string? Name { get; set; }
    public string? Description { get; set; }
    public string? RouteWkt { get; set; }
    public BackcountrySkiDetailsDto? Details { get; set; }
}

public sealed class BackcountrySkiDetailsDto
{
    public int AscentMeters { get; set; }
    public int DescentMeters { get; set; }
    public int DistanceMeters { get; set; }
    public int ElevationMinMeters { get; set; }
    public int ElevationMaxMeters { get; set; }
    public AtesRating AtesRating { get; set; }
    public Aspect? DominantAspect { get; set; }
    public int? VarsomRegionId { get; set; }
    public short? PreferredAvalancheMaxLevel { get; set; }
    public List<AspectShareDto> AspectMix { get; set; } = new();
    public List<RouteLegDto> Legs { get; set; } = new();

    public BackcountrySkiDetails ToValueObject() => new(
        ascentMeters: AscentMeters,
        descentMeters: DescentMeters,
        distanceMeters: DistanceMeters,
        elevationMinMeters: ElevationMinMeters,
        elevationMaxMeters: ElevationMaxMeters,
        atesRating: AtesRating,
        dominantAspect: DominantAspect,
        varsomRegionId: VarsomRegionId,
        preferredAvalancheMaxLevel: PreferredAvalancheMaxLevel,
        aspectMix: AspectMix.Select(a => new AspectShare(a.Aspect, a.Fraction)).ToList(),
        legs: Legs.Select(l => new RouteLeg(l.Kind, l.StartElevationMeters, l.EndElevationMeters, l.PolylineWkt)).ToList());
}

public sealed class AspectShareDto
{
    public Aspect Aspect { get; set; }
    public float Fraction { get; set; }
}

public sealed class RouteLegDto
{
    public LegKind Kind { get; set; }
    public int StartElevationMeters { get; set; }
    public int EndElevationMeters { get; set; }
    public string PolylineWkt { get; set; } = string.Empty;
}
