using Microsoft.EntityFrameworkCore;
using NetTopologySuite.IO;
using Turboapi.Activities.domain;
using Turboapi.Activities.Packrafting.domain;
using Turboapi.Activities.Packrafting.domain.handler;
using Turboapi.Activities.Packrafting.value;

namespace Turboapi.Activities.Packrafting.data;

public sealed class EfPackraftingActivityReader : IPackraftingActivityReader
{
    private readonly PackraftingContext _db;
    public EfPackraftingActivityReader(PackraftingContext db) => _db = db;

    public async Task<PackraftingActivity?> GetByIdAsync(Guid activityId, CancellationToken cancellationToken = default)
    {
        var row = await _db.Activities.Include(a => a.Segments).AsNoTracking()
            .FirstOrDefaultAsync(a => a.Id == activityId && a.DeletedAt == null, cancellationToken);
        if (row is null) return null;

        var writer = new WKTWriter();
        var details = new PackraftingDetails(
            distanceMeters: row.DistanceMeters,
            paddleDistanceMeters: row.PaddleDistanceMeters,
            portageDistanceMeters: row.PortageDistanceMeters,
            maxGrade: (WaterGrade)row.MaxGrade,
            typicalGrade: (WaterGrade)row.TypicalGrade,
            putInLat: row.PutInLat, putInLon: row.PutInLon,
            takeOutLat: row.TakeOutLat, takeOutLon: row.TakeOutLon,
            nveStationCode: row.NveStationCode,
            minFlowCumecs: row.MinFlowCumecs,
            maxFlowCumecs: row.MaxFlowCumecs,
            segments: row.Segments.OrderBy(s => s.Ordinal)
                .Select(s => new RouteSegment(
                    (SegmentKind)s.Kind,
                    s.Grade is { } g ? (WaterGrade)g : null,
                    s.DistanceMeters,
                    writer.Write(s.Geometry),
                    s.Notes))
                .ToList());

        var core = ActivityCore.Reconstitute(
            row.Id, row.OwnerId, row.Name, row.Description, row.Route,
            row.CreatedAt, row.UpdatedAt, row.DeletedAt, row.Version);
        return PackraftingActivity.Reconstitute(core, details);
    }
}
