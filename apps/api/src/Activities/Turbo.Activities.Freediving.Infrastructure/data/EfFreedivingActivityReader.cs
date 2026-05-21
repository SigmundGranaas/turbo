using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.domain;
using Turboapi.Activities.Freediving.domain;
using Turboapi.Activities.Freediving.domain.handler;
using Turboapi.Activities.Freediving.value;

namespace Turboapi.Activities.Freediving.data;

public sealed class EfFreedivingActivityReader : IFreedivingActivityReader
{
    private readonly FreedivingContext _db;
    public EfFreedivingActivityReader(FreedivingContext db) => _db = db;

    public async Task<FreedivingActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default)
    {
        var row = await _db.Activities.Include(a => a.TargetSpecies).AsNoTracking()
            .FirstOrDefaultAsync(a => a.Id == activityId && a.DeletedAt == null, cancellationToken);
        if (row is null) return null;

        var details = new FreedivingDetails(
            waterBody: (WaterBody)row.WaterBody,
            bottomType: (BottomType)row.BottomType,
            maxDepthMeters: row.MaxDepthMeters,
            typicalVisibilityMeters: row.TypicalVisibilityMeters,
            harpoonAllowed: row.HarpoonAllowed,
            shoreEntry: row.ShoreEntry,
            accessNotes: row.AccessNotes,
            targetSpecies: row.TargetSpecies
                .Select(t => new TargetSpecies(t.SpeciesCode, t.Notes))
                .ToList());

        var core = ActivityCore.Reconstitute(
            row.Id, row.OwnerId, row.Name, row.Description, row.Geometry,
            row.CreatedAt, row.UpdatedAt, row.DeletedAt, row.Version);
        return FreedivingActivity.Reconstitute(core, details);
    }
}
