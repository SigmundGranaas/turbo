using NetTopologySuite.IO;
using Turboapi.Activities.BackcountrySki.controller.request;
using Turboapi.Activities.BackcountrySki.data.model;
using Turboapi.Activities.BackcountrySki.value;

namespace Turboapi.Activities.BackcountrySki.controller;

public sealed record CreateBackcountrySkiActivityResponse(Guid Id);

public sealed class BackcountrySkiActivityResponse
{
    public Guid Id { get; set; }
    public Guid OwnerId { get; set; }
    public string Name { get; set; } = string.Empty;
    public string? Description { get; set; }

    /// <summary>WKT LINESTRING in EPSG:4326.</summary>
    public string RouteWkt { get; set; } = string.Empty;

    public BackcountrySkiDetailsDto Details { get; set; } = new();
    public DateTime CreatedAt { get; set; }
    public DateTime UpdatedAt { get; set; }
    public long Version { get; set; }

    public static BackcountrySkiActivityResponse From(BackcountrySkiActivityEntity e)
    {
        var writer = new WKTWriter();
        return new BackcountrySkiActivityResponse
        {
            Id = e.Id,
            OwnerId = e.OwnerId,
            Name = e.Name,
            Description = e.Description,
            RouteWkt = writer.Write(e.Route),
            Details = new BackcountrySkiDetailsDto
            {
                AscentMeters = e.AscentMeters,
                DescentMeters = e.DescentMeters,
                DistanceMeters = e.DistanceMeters,
                ElevationMinMeters = e.ElevationMinMeters,
                ElevationMaxMeters = e.ElevationMaxMeters,
                AtesRating = (AtesRating)e.AtesRating,
                DominantAspect = e.DominantAspect is { } a ? (Aspect)a : null,
                VarsomRegionId = e.VarsomRegionId,
                PreferredAvalancheMaxLevel = e.PreferredAvalancheMaxLevel,
                AspectMix = e.AspectMix
                    .Select(am => new AspectShareDto { Aspect = (Aspect)am.Aspect, Fraction = am.Fraction })
                    .ToList(),
                Legs = e.Legs
                    .OrderBy(l => l.Ordinal)
                    .Select(l => new RouteLegDto
                    {
                        Kind = (LegKind)l.LegKind,
                        StartElevationMeters = l.StartElevationMeters,
                        EndElevationMeters = l.EndElevationMeters,
                        PolylineWkt = writer.Write(l.Geometry),
                    })
                    .ToList(),
            },
            CreatedAt = e.CreatedAt,
            UpdatedAt = e.UpdatedAt,
            Version = e.Version,
        };
    }
}

public sealed record ErrorResponse(string Title, string Detail);
