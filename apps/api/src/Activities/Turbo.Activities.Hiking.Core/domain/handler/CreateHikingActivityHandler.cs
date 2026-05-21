using NetTopologySuite.Geometries;
using NetTopologySuite.IO;
using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Activities.domain;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Hiking.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Hiking.domain.handler;

public sealed class CreateHikingActivityHandler
{
    private readonly IGeometryNormalizer _geom;
    private readonly IOutbox<HikingScope> _outbox;
    private readonly IUnitOfWork<HikingScope> _uow;

    public CreateHikingActivityHandler(IGeometryNormalizer geom, IOutbox<HikingScope> outbox, IUnitOfWork<HikingScope> uow)
    {
        _geom = geom; _outbox = outbox; _uow = uow;
    }

    public async Task<Guid> Handle(CreateHikingActivityCommand cmd)
    {
        var reader = new WKTReader();
        var parsed = reader.Read(cmd.RouteWkt);
        if (parsed.SRID != 4326) parsed.SRID = 4326;
        var route = (LineString)_geom.Normalize(parsed, ActivityGeometryKind.LineString);

        var core = ActivityCore.New(cmd.CallerId, cmd.Name, cmd.Description, route);
        _ = HikingActivity.Create(core, cmd.Details);

        var created = new HikingActivityCreated(
            core.Id, core.OwnerId, core.Name, core.Description,
            new WKTWriter().Write(route), cmd.Details);

        var summary = new ActivitySummaryUpserted(
            activityId: core.Id, ownerId: core.OwnerId, kind: "hiking",
            name: core.Name,
            geometry: new ActivityGeometryWkt(ActivityGeometryKind.LineString, new WKTWriter().Write(route)),
            iconKey: "hiking", colorHex: "#2E7D32", version: core.Version);

        var events = new DomainEvent[] { created, summary };
        await _uow.SaveChangesAsync(ct => _outbox.AppendEventsAsync(core.Id, events, ct));
        return core.Id;
    }
}
