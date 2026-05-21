using Microsoft.EntityFrameworkCore;
using NetTopologySuite.IO;
using Turboapi.Activities.BackcountrySki.domain;
using Turboapi.Activities.BackcountrySki.domain.handler;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.domain;

namespace Turboapi.Activities.BackcountrySki.data;

public sealed class EfBackcountrySkiActivityReader : IBackcountrySkiActivityReader
{
    private readonly BackcountrySkiContext _db;

    public EfBackcountrySkiActivityReader(BackcountrySkiContext db) => _db = db;

    public async Task<BackcountrySkiActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default)
    {
        var row = await _db.Activities
            .Include(a => a.AspectMix)
            .Include(a => a.Legs)
            .AsNoTracking()
            .FirstOrDefaultAsync(a => a.Id == activityId && a.DeletedAt == null, cancellationToken);
        if (row is null) return null;

        var writer = new WKTWriter();
        var details = new BackcountrySkiDetails(
            ascentMeters: row.AscentMeters,
            descentMeters: row.DescentMeters,
            distanceMeters: row.DistanceMeters,
            elevationMinMeters: row.ElevationMinMeters,
            elevationMaxMeters: row.ElevationMaxMeters,
            atesRating: (AtesRating)row.AtesRating,
            dominantAspect: row.DominantAspect is { } a ? (Aspect)a : null,
            varsomRegionId: row.VarsomRegionId,
            preferredAvalancheMaxLevel: row.PreferredAvalancheMaxLevel,
            aspectMix: row.AspectMix
                .Select(am => new AspectShare((Aspect)am.Aspect, am.Fraction))
                .ToList(),
            legs: row.Legs
                .OrderBy(l => l.Ordinal)
                .Select(l => new RouteLeg(
                    (LegKind)l.LegKind,
                    l.StartElevationMeters,
                    l.EndElevationMeters,
                    writer.Write(l.Geometry)))
                .ToList());

        var core = ActivityCore.Reconstitute(
            row.Id, row.OwnerId, row.Name, row.Description, row.Route,
            row.CreatedAt, row.UpdatedAt, row.DeletedAt, row.Version);
        return BackcountrySkiActivity.Reconstitute(core, details);
    }
}
