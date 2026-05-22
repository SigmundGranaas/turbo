using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.domain;
using Turboapi.Activities.Hiking.domain;
using Turboapi.Activities.Hiking.domain.handler;
using Turboapi.Activities.Hiking.value;

namespace Turboapi.Activities.Hiking.data;

public sealed class EfHikingActivityReader : IHikingActivityReader
{
    private readonly HikingContext _db;

    public EfHikingActivityReader(HikingContext db) => _db = db;

    public async Task<HikingActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default)
    {
        var row = await _db.Activities
            .Include(a => a.WaterSources)
            .AsNoTracking()
            .FirstOrDefaultAsync(a => a.Id == activityId && a.DeletedAt == null, cancellationToken);
        if (row is null) return null;

        var details = new HikingDetails(
            distanceMeters: row.DistanceMeters,
            ascentMeters: row.AscentMeters,
            descentMeters: row.DescentMeters,
            elevationMinMeters: row.ElevationMinMeters,
            elevationMaxMeters: row.ElevationMaxMeters,
            difficulty: (HikingDifficulty)row.Difficulty,
            surface: (TrailSurface)row.Surface,
            marking: (TrailMarking)row.Marking,
            estimatedHours: row.EstimatedHours,
            hasWaterSources: row.HasWaterSources,
            hasShelter: row.HasShelter,
            waterSources: row.WaterSources
                .OrderBy(w => w.Ordinal)
                .Select(w => new WaterSource(w.Lat, w.Lon, w.Kind, w.Notes))
                .ToList());

        var core = ActivityCore.Reconstitute(
            row.Id, row.OwnerId, row.Name, row.Description, row.Route,
            row.CreatedAt, row.UpdatedAt, row.DeletedAt, row.Version);
        return HikingActivity.Reconstitute(core, details);
    }
}
