using Microsoft.EntityFrameworkCore;
using Turboapi.Activities.domain;
using Turboapi.Activities.XcSki.domain;
using Turboapi.Activities.XcSki.domain.handler;
using Turboapi.Activities.XcSki.value;

namespace Turboapi.Activities.XcSki.data;

public sealed class EfXcSkiActivityReader : IXcSkiActivityReader
{
    private readonly XcSkiContext _db;
    public EfXcSkiActivityReader(XcSkiContext db) => _db = db;

    public async Task<XcSkiActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default)
    {
        var row = await _db.Activities.AsNoTracking()
            .FirstOrDefaultAsync(a => a.Id == activityId && a.DeletedAt == null, cancellationToken);
        if (row is null) return null;

        var details = new XcSkiDetails(
            row.DistanceMeters, row.AscentMeters, row.DescentMeters,
            (XcSkiTechnique)row.Technique, (GroomingStatus)row.GroomingStatus,
            row.IsLit, row.RequiresSeasonPass, row.GroomingFeedKey);

        var core = ActivityCore.Reconstitute(
            row.Id, row.OwnerId, row.Name, row.Description, row.Route,
            row.CreatedAt, row.UpdatedAt, row.DeletedAt, row.Version);
        return XcSkiActivity.Reconstitute(core, details);
    }
}
