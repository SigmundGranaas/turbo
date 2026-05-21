using Microsoft.EntityFrameworkCore;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Fishing.data.model;
using Turboapi.Activities.Fishing.domain;
using Turboapi.Activities.Fishing.domain.handler;
using Turboapi.Activities.Fishing.value;

namespace Turboapi.Activities.Fishing.data;

/// <summary>
/// Infrastructure-side implementation of <see cref="IFishingActivityReader"/>.
/// Lives next to the EF context so Core can stay framework-agnostic.
/// </summary>
public sealed class EfFishingActivityReader : IFishingActivityReader
{
    private readonly FishingContext _db;

    public EfFishingActivityReader(FishingContext db) => _db = db;

    public async Task<FishingActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default)
    {
        var row = await _db.Activities
            .Include(a => a.TargetSpecies)
            .Include(a => a.DepthSamples)
            .AsNoTracking()
            .FirstOrDefaultAsync(a => a.Id == activityId && a.DeletedAt == null, cancellationToken);
        if (row is null) return null;

        var details = new FishingDetails(
            waterKind: (WaterKind)row.WaterKind,
            shoreOrBoat: (ShoreOrBoat)row.ShoreOrBoat,
            accessNotes: row.AccessNotes,
            targetSpecies: row.TargetSpecies
                .Select(t => new TargetSpecies(t.SpeciesCode, t.Notes))
                .ToList(),
            knownDepths: row.DepthSamples
                .OrderBy(d => d.Ordinal)
                .Select(d => new DepthSample(d.Lat, d.Lon, d.DepthMeters))
                .ToList(),
            preferred: row.PreferredPressureMinHpa is null && row.PreferredPressureMaxHpa is null && row.PreferredWindMaxMs is null
                ? null
                : new PreferredConditions(row.PreferredPressureMinHpa, row.PreferredPressureMaxHpa, row.PreferredWindMaxMs));

        var core = ActivityCore.Reconstitute(
            row.Id, row.OwnerId, row.Name, row.Description, row.Geometry,
            row.CreatedAt, row.UpdatedAt, row.DeletedAt, row.Version);
        return FishingActivity.Reconstitute(core, details);
    }
}
